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

/// Default sleep recovery rates per minute, used when a sleeping agent has no
/// occupied bed metadata. Concrete beds can override these through JSON state.
const STANDARD_SLEEP_RECOVERY_PER_MIN: f64 = 2.0;
const STANDARD_STAMINA_RECOVERY_PER_MIN: f64 = 0.5;

const HOME_SLEEP_RECOVERY_PER_MIN: f64 = 3.0;
const HOME_STAMINA_RECOVERY_PER_MIN: f64 = 1.2;

const DORM_SLEEP_RECOVERY_PER_MIN: f64 = 2.4;
const DORM_STAMINA_RECOVERY_PER_MIN: f64 = 0.9;

const MOTEL_SLEEP_RECOVERY_PER_MIN: f64 = 2.0;
const MOTEL_STAMINA_RECOVERY_PER_MIN: f64 = 0.7;

const CAMPGROUND_SLEEP_RECOVERY_PER_MIN: f64 = 1.0;
const CAMPGROUND_STAMINA_RECOVERY_PER_MIN: f64 = 0.4;

/// Stamina cost for moving to an adjacent location
pub const MOVE_STAMINA_COST: i16 = 5;

#[derive(Debug, Clone)]
pub struct SleepRecoveryPlan {
    pub tier: String,
    pub label: String,
    pub sleep_recovery_per_min: f64,
    pub stamina_recovery_per_min: f64,
    pub cost_cents: i64,
}

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

    // If sleeping, apply housing-aware recovery instead of decay.
    if agent.state == "sleeping" {
        let plan = sleep_recovery_plan_for_current_bed_tx(tx, &agent).await?;
        return apply_sleep_recovery_inner(tx, agent, &plan).await;
    }

    apply_decay_inner(tx, agent).await
}

/// Apply vitals decay + sleep recovery for a sleeping agent.
/// Food/water/stamina still decay while sleeping; sleep_level recovers.
/// Uses FOR UPDATE to prevent concurrent race conditions.
#[allow(dead_code)]
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

    let plan = sleep_recovery_plan_for_current_bed_tx(tx, &agent).await?;
    apply_sleep_recovery_inner(tx, agent, &plan).await
}

pub async fn apply_sleep_recovery_with_plan_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
    plan: &SleepRecoveryPlan,
) -> AppResult<Agent> {
    let agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT * FROM agents WHERE id = $1 FOR UPDATE
        "#,
    )
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await?;

    if agent.state != "sleeping" {
        return apply_decay_inner(tx, agent).await;
    }

    apply_sleep_recovery_inner(tx, agent, plan).await
}

pub async fn sleep_recovery_plan_for_bed_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
    location_id: Option<&str>,
    bed_state: &serde_json::Value,
) -> AppResult<SleepRecoveryPlan> {
    let inferred_tier = match bed_state
        .get("housing_tier")
        .and_then(|value| value.as_str())
    {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => infer_housing_tier_for_location_tx(tx, agent_id, location_id).await?,
    };

    let mut plan = plan_for_tier(&inferred_tier);

    if let Some(value) = bed_state
        .get("sleep_recovery_per_min")
        .and_then(|value| value.as_f64())
    {
        plan.sleep_recovery_per_min = value;
    }
    if let Some(value) = bed_state
        .get("stamina_recovery_per_min")
        .and_then(|value| value.as_f64())
    {
        plan.stamina_recovery_per_min = value;
    }
    if let Some(value) = bed_state
        .get("nightly_price_cents")
        .and_then(|value| value.as_i64())
    {
        plan.cost_cents = value.max(0);
    }

    Ok(plan)
}

/// Inner: apply awake decay. Takes an already-fetched agent (row is locked).
async fn apply_decay_inner(tx: &mut Transaction<'_, Postgres>, agent: Agent) -> AppResult<Agent> {
    let now = Utc::now();
    let elapsed = (now - agent.last_vitals_update).num_seconds().max(0) as f64 / 60.0;

    // No decay if less than a minute has passed (avoid floating point noise)
    if elapsed < 1.0 {
        return Ok(agent);
    }

    let new_food = ((agent.food_level as f64) - FOOD_DECAY_PER_MIN * elapsed)
        .round()
        .clamp(0.0, 100.0) as i16;
    let new_water = ((agent.water_level as f64) - WATER_DECAY_PER_MIN * elapsed)
        .round()
        .clamp(0.0, 100.0) as i16;
    let new_stamina = ((agent.stamina_level as f64) - STAMINA_DECAY_PER_MIN * elapsed)
        .round()
        .clamp(0.0, 100.0) as i16;
    let new_sleep = ((agent.sleep_level as f64) - SLEEP_DECAY_PER_MIN * elapsed)
        .round()
        .clamp(0.0, 100.0) as i16;
    let new_hygiene = ((agent.hygiene_level as f64) - HYGIENE_DECAY_PER_MIN * elapsed)
        .round()
        .clamp(0.0, 100.0) as i16;

    // Appearance decays faster when hygiene is low, slower when hygiene is high
    let appearance_modifier = if agent.hygiene_level < 20 {
        2.0
    } else if agent.hygiene_level > 80 {
        0.5
    } else {
        1.0
    };
    let new_appearance = ((agent.appearance_level as f64)
        - APPEARANCE_DECAY_PER_MIN * appearance_modifier * elapsed)
        .round()
        .clamp(0.0, 100.0) as i16;

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
    plan: &SleepRecoveryPlan,
) -> AppResult<Agent> {
    let now = Utc::now();
    let elapsed = (now - agent.last_vitals_update).num_seconds().max(0) as f64 / 60.0;

    if elapsed < 1.0 {
        return Ok(agent);
    }

    // Food and water still decay while sleeping.
    let new_food = ((agent.food_level as f64) - FOOD_DECAY_PER_MIN * elapsed)
        .round()
        .clamp(0.0, 100.0) as i16;
    let new_water = ((agent.water_level as f64) - WATER_DECAY_PER_MIN * elapsed)
        .round()
        .clamp(0.0, 100.0) as i16;
    let stamina_delta = (plan.stamina_recovery_per_min - STAMINA_DECAY_PER_MIN) * elapsed;
    let new_stamina = ((agent.stamina_level as f64) + stamina_delta)
        .round()
        .clamp(0.0, 100.0) as i16;
    let new_sleep = ((agent.sleep_level as f64) + plan.sleep_recovery_per_min * elapsed)
        .round()
        .clamp(0.0, 100.0) as i16;
    // Hygiene decays slower while sleeping
    let new_hygiene = ((agent.hygiene_level as f64) - HYGIENE_DECAY_PER_MIN * 0.5 * elapsed)
        .round()
        .clamp(0.0, 100.0) as i16;
    // Appearance decays slower while sleeping
    let appearance_modifier = if agent.hygiene_level < 20 {
        2.0
    } else if agent.hygiene_level > 80 {
        0.5
    } else {
        1.0
    };
    let new_appearance = ((agent.appearance_level as f64)
        - APPEARANCE_DECAY_PER_MIN * appearance_modifier * 0.5 * elapsed)
        .round()
        .clamp(0.0, 100.0) as i16;

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

async fn sleep_recovery_plan_for_current_bed_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent: &Agent,
) -> AppResult<SleepRecoveryPlan> {
    let occupied_bed = sqlx::query_as::<_, (Option<String>, serde_json::Value)>(
        r#"
        SELECT location_id, state
        FROM world_objects
        WHERE state->>'occupied_by' = $1
        ORDER BY id
        LIMIT 1
        "#,
    )
    .bind(&agent.id)
    .fetch_optional(&mut **tx)
    .await?;

    match occupied_bed {
        Some((location_id, bed_state)) => {
            sleep_recovery_plan_for_bed_tx(tx, &agent.id, location_id.as_deref(), &bed_state).await
        }
        None => {
            sleep_recovery_plan_for_bed_tx(
                tx,
                &agent.id,
                Some(&agent.current_location_id),
                &serde_json::Value::Null,
            )
            .await
        }
    }
}

async fn infer_housing_tier_for_location_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
    location_id: Option<&str>,
) -> AppResult<String> {
    let Some(location_id) = location_id else {
        return Ok("standard".to_string());
    };

    if location_id == "smallville_campground" {
        return Ok("campground".to_string());
    }
    if location_id == "smallville_motel_room" || location_id == "smallville_motel_lobby" {
        return Ok("motel".to_string());
    }

    let location_role = sqlx::query_as::<_, (String, Option<i32>)>(
        r#"
        SELECT lr.role, l.capacity
        FROM location_roles lr
        JOIN locations l ON l.id = lr.location_id
        WHERE lr.agent_id = $1
          AND lr.location_id = $2
          AND lr.role IN ('resident', 'owner', 'tenant')
        ORDER BY CASE lr.role
            WHEN 'owner' THEN 1
            WHEN 'resident' THEN 2
            WHEN 'tenant' THEN 3
            ELSE 4
        END
        LIMIT 1
        "#,
    )
    .bind(agent_id)
    .bind(location_id)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(match location_role {
        Some((_role, Some(_capacity))) => "dorm".to_string(),
        Some((_role, None)) => "home".to_string(),
        None => "standard".to_string(),
    })
}

fn plan_for_tier(tier: &str) -> SleepRecoveryPlan {
    match tier {
        "home" | "owned_home" => SleepRecoveryPlan {
            tier: "home".to_string(),
            label: "owned home".to_string(),
            sleep_recovery_per_min: HOME_SLEEP_RECOVERY_PER_MIN,
            stamina_recovery_per_min: HOME_STAMINA_RECOVERY_PER_MIN,
            cost_cents: 0,
        },
        "dorm" => SleepRecoveryPlan {
            tier: "dorm".to_string(),
            label: "dorm bed".to_string(),
            sleep_recovery_per_min: DORM_SLEEP_RECOVERY_PER_MIN,
            stamina_recovery_per_min: DORM_STAMINA_RECOVERY_PER_MIN,
            cost_cents: 0,
        },
        "motel" => SleepRecoveryPlan {
            tier: "motel".to_string(),
            label: "motel room".to_string(),
            sleep_recovery_per_min: MOTEL_SLEEP_RECOVERY_PER_MIN,
            stamina_recovery_per_min: MOTEL_STAMINA_RECOVERY_PER_MIN,
            cost_cents: 2_500,
        },
        "campground" => SleepRecoveryPlan {
            tier: "campground".to_string(),
            label: "campground".to_string(),
            sleep_recovery_per_min: CAMPGROUND_SLEEP_RECOVERY_PER_MIN,
            stamina_recovery_per_min: CAMPGROUND_STAMINA_RECOVERY_PER_MIN,
            cost_cents: 0,
        },
        _ => SleepRecoveryPlan {
            tier: "standard".to_string(),
            label: "standard bed".to_string(),
            sleep_recovery_per_min: STANDARD_SLEEP_RECOVERY_PER_MIN,
            stamina_recovery_per_min: STANDARD_STAMINA_RECOVERY_PER_MIN,
            cost_cents: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn housing_tiers_have_distinct_sleep_and_stamina_recovery() {
        let home = plan_for_tier("home");
        let motel = plan_for_tier("motel");
        let campground = plan_for_tier("campground");

        assert!(home.sleep_recovery_per_min > motel.sleep_recovery_per_min);
        assert!(motel.sleep_recovery_per_min > campground.sleep_recovery_per_min);

        assert!(home.stamina_recovery_per_min > motel.stamina_recovery_per_min);
        assert!(motel.stamina_recovery_per_min > campground.stamina_recovery_per_min);

        assert_eq!(home.cost_cents, 0);
        assert!(motel.cost_cents > 0);
        assert_eq!(campground.cost_cents, 0);
    }
}
