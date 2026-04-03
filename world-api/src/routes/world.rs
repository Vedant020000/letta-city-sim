use axum::{Json, extract::State};
use chrono::{Local, Timelike, Utc};

use crate::error::AppResult;
use crate::models::world::WorldTimeResponse;
use crate::state::AppState;

pub async fn get_world_time(State(state): State<AppState>) -> AppResult<Json<WorldTimeResponse>> {
    let paused = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT COALESCE((value->>'simulation_paused')::boolean, false)
        FROM simulation_state
        WHERE key = 'world'
        LIMIT 1
        "#,
    )
    .fetch_optional(state.pool())
    .await?
    .unwrap_or(false);

    let now_utc = Utc::now();
    let now_local = Local::now();
    let hour = now_local.hour();

    let time_of_day = match hour {
        5..=10 => "morning",
        11..=16 => "afternoon",
        17..=20 => "evening",
        _ => "night",
    }
    .to_string();

    Ok(Json(WorldTimeResponse {
        timestamp: now_utc.to_rfc3339(),
        time_of_day,
        simulation_paused: paused,
    }))
}
