use axum::{Json, extract::State};
use chrono::{DateTime, Timelike, Utc};
use serde::Deserialize;

use crate::error::AppResult;
use crate::models::world::WorldTimeResponse;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateTimeRequest {
    pub time_scale: Option<f64>,
    pub paused: Option<bool>,
    pub epoch_start: Option<String>,
}

/// Compute the current simulated time based on epoch_start + time_scale.
/// If no sim clock is configured, falls back to real wall-clock time.
/// Returns (sim_time, time_scale, paused, epoch_start).
pub async fn compute_sim_time(pool: &sqlx::PgPool) -> (DateTime<Utc>, f64, bool, Option<DateTime<Utc>>) {
    let config = sqlx::query_as::<_, (String,)>(
        r#"SELECT value::text FROM simulation_state WHERE key = 'time'"#,
    )
    .fetch_optional(pool)
    .await;

    let now_wall = Utc::now();

    match config {
        Ok(Some((raw_json,))) => {
            let parsed: serde_json::Value = serde_json::from_str(&raw_json).unwrap_or_default();
            let time_scale = parsed.get("time_scale").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let paused = parsed.get("paused").and_then(|v| v.as_bool()).unwrap_or(false);

            let epoch = parsed.get("epoch_start")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            if paused {
                let frozen_time = epoch.unwrap_or(now_wall);
                return (frozen_time, time_scale, true, epoch);
            }

            match epoch {
                Some(epoch_dt) => {
                    let wall_elapsed = (now_wall - epoch_dt).num_seconds() as f64;
                    let sim_elapsed = wall_elapsed * time_scale;
                    let sim_time = epoch_dt + chrono::Duration::seconds(sim_elapsed as i64);
                    (sim_time, time_scale, false, Some(epoch_dt))
                }
                None => (now_wall, time_scale, false, None),
            }
        }
        _ => (now_wall, 1.0, false, None),
    }
}

/// Determine time_of_day label from hour
pub fn time_of_day_from_hour(hour: u32) -> &'static str {
    match hour {
        5..=10 => "morning",
        11..=16 => "afternoon",
        17..=20 => "evening",
        _ => "night",
    }
}

/// Check if any shop is currently open at the given sim hour
pub async fn any_shop_open(pool: &sqlx::PgPool, hour: u32) -> bool {
    sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM shops WHERE is_active = TRUE AND opens_at <= $1 AND closes_at > $1)"#,
    )
    .bind(hour as i16)
    .fetch_one(pool)
    .await
    .unwrap_or(true) // default to open if query fails
}

/// Compute day number from sim time (day 1 = first day from epoch_start)
pub fn day_number_from_sim_time(sim_time: &DateTime<Utc>, epoch_start: Option<&DateTime<Utc>>) -> i64 {
    match epoch_start {
        Some(epoch) => {
            let sim_elapsed_hours = (*sim_time - *epoch).num_seconds() / 3600;
            (sim_elapsed_hours / 24) + 1
        }
        None => 1,
    }
}

pub async fn get_world_time(State(state): State<AppState>) -> AppResult<Json<WorldTimeResponse>> {
    let (sim_time, _time_scale, paused, epoch) = compute_sim_time(state.pool()).await;

    // Also check simulation_paused from world state
    let world_paused = sqlx::query_scalar::<_, bool>(
        r#"SELECT COALESCE((value->>'simulation_paused')::boolean, false) FROM simulation_state WHERE key = 'world' LIMIT 1"#,
    )
    .fetch_optional(state.pool())
    .await?
    .unwrap_or(false);

    let is_paused = paused || world_paused;
    let hour = sim_time.hour();
    let time_of_day = time_of_day_from_hour(hour).to_string();
    let day_number = day_number_from_sim_time(&sim_time, epoch.as_ref());
    let shops_open = any_shop_open(state.pool(), hour).await;

    Ok(Json(WorldTimeResponse {
        timestamp: sim_time.to_rfc3339(),
        hour,
        time_of_day,
        day_number,
        shops_open,
        simulation_paused: is_paused,
    }))
}

pub async fn update_world_time(
    State(state): State<AppState>,
    Json(payload): Json<UpdateTimeRequest>,
) -> AppResult<Json<WorldTimeResponse>> {
    // Read current config
    let current = sqlx::query_as::<_, (String,)>(
        r#"SELECT value::text FROM simulation_state WHERE key = 'time'"#,
    )
    .fetch_optional(state.pool())
    .await;

    let mut config: serde_json::Value = match current {
        Ok(Some((raw_json,))) => serde_json::from_str(&raw_json).unwrap_or_default(),
        _ => serde_json::json!({"time_scale": 1.0, "paused": false}),
    };

    if let Some(ts) = payload.time_scale {
        config["time_scale"] = serde_json::json!(ts);
    }
    if let Some(p) = payload.paused {
        config["paused"] = serde_json::json!(p);
    }
    if let Some(ep) = payload.epoch_start {
        config["epoch_start"] = serde_json::json!(ep);
    }

    // Ensure epoch_start exists
    if config.get("epoch_start").is_none() {
        config["epoch_start"] = serde_json::json!(Utc::now().to_rfc3339());
    }

    sqlx::query(
        r#"INSERT INTO simulation_state (id, key, value) VALUES ('time_config', 'time', $1::jsonb)
        ON CONFLICT (id) DO UPDATE SET key = EXCLUDED.key, value = EXCLUDED.value, updated_at = NOW()"#,
    )
    .bind(config.to_string())
    .execute(state.pool())
    .await?;

    // Return updated time
    get_world_time(State(state)).await
}
