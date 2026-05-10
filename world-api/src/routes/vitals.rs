use chrono::Utc;
use sqlx::{Postgres, Transaction};

use crate::error::AppResult;
use crate::models::agent::Agent;

/// Decay rates per minute (as f64 for precision, stored as i16 after rounding)
const FOOD_DECAY_PER_MIN: f64 = 0.5;
const WATER_DECAY_PER_MIN: f64 = 0.7;
const STAMINA_DECAY_PER_MIN: f64 = 0.3;
const SLEEP_DECAY_PER_MIN: f64 = 0.2;
const HYGIENE_DECAY_PER_MIN: f64 = 0.2;
const APPEARANCE_DECAY_PER_MIN: f64 = 0.15;

/// Sleep recovery rate per minute
const SLEEP_RECOVERY_PER_MIN: f64 = 2.0;

/// Stamina cost for moving to an adjacent location
pub const MOVE_STAMINA_COST: i16 = 5;

/// Apply time-based vitals changes for an agent.
/// Automatically selects decay or recovery based on agent state.
/// Uses FOR UPDATE to prevent concurrent race conditions.
/// Returns the updated agent after applying changes.
pub async fn apply_vitals_decay_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
) -> AppResult<Agent> {
    let agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT * FROM agents WHERE id = $1 FOR UPDATE
        "#,
    )
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await?;

    // If sleeping, apply recovery instead of decay
    if agent.state == "sleeping" {
        return apply_sleep_recovery_inner(tx, agent).await;
    }

    apply_decay_inner(tx, agent).await
}

/// Apply vitals decay + sleep recovery for a sleeping agent.
/// Food/water/stamina still decay while sleeping; sleep_level recovers.
/// Uses FOR UPDATE to prevent concurrent race conditions.
pub async fn apply_sleep_recovery_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
) -> AppResult<Agent> {
    let agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT * FROM agents WHERE id = $1 FOR UPDATE
        "#,
    )
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await?;

    // If not sleeping, apply regular decay instead
    if agent.state != "sleeping" {
        return apply_decay_inner(tx, agent).await;
    }

    apply_sleep_recovery_inner(tx, agent).await
}

/// Inner: apply awake decay. Takes an already-fetched agent (row is locked).
async fn apply_decay_inner(
    tx: &mut Transaction<'_, Postgres>,
    agent: Agent,
) -> AppResult<Agent> {
    let now = Utc::now();
    let elapsed = (now - agent.last_vitals_update).num_seconds().max(0) as f64 / 60.0;

    // No decay if less than a minute has passed (avoid floating point noise)
    if elapsed < 1.0 {
        return Ok(agent);
    }

    let new_food = ((agent.food_level as f64) - FOOD_DECAY_PER_MIN * elapsed).round().clamp(0.0, 100.0) as i16;
    let new_water = ((agent.water_level as f64) - WATER_DECAY_PER_MIN * elapsed).round().clamp(0.0, 100.0) as i16;
    let new_stamina = ((agent.stamina_level as f64) - STAMINA_DECAY_PER_MIN * elapsed).round().clamp(0.0, 100.0) as i16;
    let new_sleep = ((agent.sleep_level as f64) - SLEEP_DECAY_PER_MIN * elapsed).round().clamp(0.0, 100.0) as i16;
    let new_hygiene = ((agent.hygiene_level as f64) - HYGIENE_DECAY_PER_MIN * elapsed).round().clamp(0.0, 100.0) as i16;

    // Appearance decays faster when hygiene is low, slower when hygiene is high
    let appearance_modifier = if agent.hygiene_level < 20 { 2.0 } else if agent.hygiene_level > 80 { 0.5 } else { 1.0 };
    let new_appearance = ((agent.appearance_level as f64) - APPEARANCE_DECAY_PER_MIN * appearance_modifier * elapsed).round().clamp(0.0, 100.0) as i16;

    let updated = sqlx::query_as::<_, Agent>(
        r#"
        UPDATE agents
        SET food_level = $1,
            water_level = $2,
            stamina_level = $3,
            sleep_level = $4,
            hygiene_level = $5,
            appearance_level = $6,
            last_vitals_update = $7,
            updated_at = $7
        WHERE id = $8
        RETURNING *
        "#,
    )
    .bind(new_food)
    .bind(new_water)
    .bind(new_stamina)
    .bind(new_sleep)
    .bind(new_hygiene)
    .bind(new_appearance)
    .bind(now)
    .bind(&agent.id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(updated)
}

/// Inner: apply sleep recovery. Takes an already-fetched agent (row is locked).
async fn apply_sleep_recovery_inner(
    tx: &mut Transaction<'_, Postgres>,
    agent: Agent,
) -> AppResult<Agent> {
    let now = Utc::now();
    let elapsed = (now - agent.last_vitals_update).num_seconds().max(0) as f64 / 60.0;

    if elapsed < 1.0 {
        return Ok(agent);
    }

    // Food/water/stamina still decay while sleeping
    let new_food = ((agent.food_level as f64) - FOOD_DECAY_PER_MIN * elapsed).round().clamp(0.0, 100.0) as i16;
    let new_water = ((agent.water_level as f64) - WATER_DECAY_PER_MIN * elapsed).round().clamp(0.0, 100.0) as i16;
    let new_stamina = ((agent.stamina_level as f64) - STAMINA_DECAY_PER_MIN * elapsed).round().clamp(0.0, 100.0) as i16;
    // Sleep level recovers while sleeping
    let new_sleep = ((agent.sleep_level as f64) + SLEEP_RECOVERY_PER_MIN * elapsed).round().clamp(0.0, 100.0) as i16;
    // Hygiene decays slower while sleeping
    let new_hygiene = ((agent.hygiene_level as f64) - HYGIENE_DECAY_PER_MIN * 0.5 * elapsed).round().clamp(0.0, 100.0) as i16;
    // Appearance decays slower while sleeping
    let appearance_modifier = if agent.hygiene_level < 20 { 2.0 } else if agent.hygiene_level > 80 { 0.5 } else { 1.0 };
    let new_appearance = ((agent.appearance_level as f64) - APPEARANCE_DECAY_PER_MIN * appearance_modifier * 0.5 * elapsed).round().clamp(0.0, 100.0) as i16;

    let updated = sqlx::query_as::<_, Agent>(
        r#"
        UPDATE agents
        SET food_level = $1,
            water_level = $2,
            stamina_level = $3,
            sleep_level = $4,
            hygiene_level = $5,
            appearance_level = $6,
            last_vitals_update = $7,
            updated_at = $7
        WHERE id = $8
        RETURNING *
        "#,
    )
    .bind(new_food)
    .bind(new_water)
    .bind(new_stamina)
    .bind(new_sleep)
    .bind(new_hygiene)
    .bind(new_appearance)
    .bind(now)
    .bind(&agent.id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(updated)
}
