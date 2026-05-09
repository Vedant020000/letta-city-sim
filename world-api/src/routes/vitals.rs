use chrono::Utc;
use sqlx::{Postgres, Transaction};

use crate::error::AppResult;
use crate::models::agent::Agent;

/// Decay rates per minute (as f64 for precision, stored as i16 after clamping)
const FOOD_DECAY_PER_MIN: f64 = 0.5;
const WATER_DECAY_PER_MIN: f64 = 0.7;
const STAMINA_DECAY_PER_MIN: f64 = 0.3;
const SLEEP_DECAY_PER_MIN: f64 = 0.2;

/// Sleep recovery rate per minute
const SLEEP_RECOVERY_PER_MIN: f64 = 2.0;

/// Stamina cost for moving to an adjacent location
pub const MOVE_STAMINA_COST: i16 = 5;

/// Apply time-based vitals decay for an awake agent.
/// Must be called inside a transaction where the agent row is already locked (FOR UPDATE).
/// Returns the updated agent after applying decay.
pub async fn apply_vitals_decay_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
) -> AppResult<Agent> {
    let agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT * FROM agents WHERE id = $1
        "#,
    )
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await?;

    let now = Utc::now();
    let elapsed = (now - agent.last_vitals_update).num_seconds().max(0) as f64 / 60.0;

    // No decay if less than a minute has passed (avoid floating point noise)
    if elapsed < 1.0 {
        return Ok(agent);
    }

    let new_food = ((agent.food_level as f64) - FOOD_DECAY_PER_MIN * elapsed).clamp(0.0, 100.0) as i16;
    let new_water = ((agent.water_level as f64) - WATER_DECAY_PER_MIN * elapsed).clamp(0.0, 100.0) as i16;
    let new_stamina = ((agent.stamina_level as f64) - STAMINA_DECAY_PER_MIN * elapsed).clamp(0.0, 100.0) as i16;
    let new_sleep = ((agent.sleep_level as f64) - SLEEP_DECAY_PER_MIN * elapsed).clamp(0.0, 100.0) as i16;

    let updated = sqlx::query_as::<_, Agent>(
        r#"
        UPDATE agents
        SET food_level = $1,
            water_level = $2,
            stamina_level = $3,
            sleep_level = $4,
            last_vitals_update = $5,
            updated_at = $5
        WHERE id = $6
        RETURNING *
        "#,
    )
    .bind(new_food)
    .bind(new_water)
    .bind(new_stamina)
    .bind(new_sleep)
    .bind(now)
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(updated)
}

/// Apply vitals decay + sleep recovery for a sleeping agent.
/// Food/water/stamina still decay while sleeping; sleep_level recovers.
/// Must be called inside a transaction where the agent row is already locked.
pub async fn apply_sleep_recovery_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
) -> AppResult<Agent> {
    let agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT * FROM agents WHERE id = $1
        "#,
    )
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await?;

    let now = Utc::now();
    let elapsed = (now - agent.last_vitals_update).num_seconds().max(0) as f64 / 60.0;

    if elapsed < 1.0 {
        return Ok(agent);
    }

    // Food/water/stamina still decay while sleeping
    let new_food = ((agent.food_level as f64) - FOOD_DECAY_PER_MIN * elapsed).clamp(0.0, 100.0) as i16;
    let new_water = ((agent.water_level as f64) - WATER_DECAY_PER_MIN * elapsed).clamp(0.0, 100.0) as i16;
    let new_stamina = ((agent.stamina_level as f64) - STAMINA_DECAY_PER_MIN * elapsed).clamp(0.0, 100.0) as i16;
    // Sleep level recovers while sleeping
    let new_sleep = ((agent.sleep_level as f64) + SLEEP_RECOVERY_PER_MIN * elapsed).clamp(0.0, 100.0) as i16;

    let updated = sqlx::query_as::<_, Agent>(
        r#"
        UPDATE agents
        SET food_level = $1,
            water_level = $2,
            stamina_level = $3,
            sleep_level = $4,
            last_vitals_update = $5,
            updated_at = $5
        WHERE id = $6
        RETURNING *
        "#,
    )
    .bind(new_food)
    .bind(new_water)
    .bind(new_stamina)
    .bind(new_sleep)
    .bind(now)
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(updated)
}
