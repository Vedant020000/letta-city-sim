use axum::{
    Json,
    extract::{Query, State},
};
use chrono::Utc;

use crate::auth::SimKey;
use crate::error::{AppError, AppResult};
use crate::models::event::{CreateEventRequest, EventsQuery, SimEvent};
use crate::state::AppState;

pub async fn list_events(
    State(state): State<AppState>,
    Query(query): Query<EventsQuery>,
) -> AppResult<Json<Vec<SimEvent>>> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);

    let events = sqlx::query_as::<_, SimEvent>(
        r#"
        SELECT id, occurred_at, type, actor_id, location_id, description, metadata
        FROM events
        WHERE ($1::timestamptz IS NULL OR occurred_at >= $1)
          AND ($2::text IS NULL OR location_id = $2)
          AND ($3::text IS NULL OR actor_id = $3)
          AND ($4::text IS NULL OR type = $4)
        ORDER BY occurred_at DESC
        LIMIT $5
        "#,
    )
    .bind(query.since)
    .bind(query.location_id)
    .bind(query.actor_id)
    .bind(query.r#type)
    .bind(limit)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(events))
}

pub async fn create_event(
    State(state): State<AppState>,
    _sim_key: SimKey,
    Json(payload): Json<CreateEventRequest>,
) -> AppResult<Json<SimEvent>> {
    if payload.r#type.trim().is_empty() {
        return Err(AppError::BadRequest(
            "event type cannot be empty".to_string(),
        ));
    }

    if payload.description.trim().is_empty() {
        return Err(AppError::BadRequest(
            "event description cannot be empty".to_string(),
        ));
    }

    let created = sqlx::query_as::<_, SimEvent>(
        r#"
        INSERT INTO events (occurred_at, type, actor_id, location_id, description, metadata)
        VALUES ($1, $2, $3, $4, $5, $6::jsonb)
        RETURNING id, occurred_at, type, actor_id, location_id, description, metadata
        "#,
    )
    .bind(Utc::now())
    .bind(payload.r#type)
    .bind(payload.actor_id)
    .bind(payload.location_id)
    .bind(payload.description)
    .bind(
        payload
            .metadata
            .unwrap_or_else(|| serde_json::json!({}))
            .to_string(),
    )
    .fetch_one(state.pool())
    .await?;

    Ok(Json(created))
}
