use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use chrono::Utc;

use crate::error::{AppError, AppResult};
use crate::models::object::{UpdateWorldObjectRequest, WorldObject};
use crate::state::AppState;

pub async fn list_objects_by_location(
    State(state): State<AppState>,
    Path(location_id): Path<String>,
) -> AppResult<Json<Vec<WorldObject>>> {
    let exists = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM locations
        WHERE id = $1
        "#,
    )
    .bind(&location_id)
    .fetch_optional(state.pool())
    .await?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    let objects = sqlx::query_as::<_, WorldObject>(
        r#"
        SELECT id, name, location_id, state, actions
        FROM world_objects
        WHERE location_id = $1
        ORDER BY name
        "#,
    )
    .bind(&location_id)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(objects))
}

pub async fn update_object_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(object_id): Path<String>,
    Json(payload): Json<UpdateWorldObjectRequest>,
) -> AppResult<Json<WorldObject>> {
    let actor_id = headers
        .get("x-agent-id")
        .ok_or_else(|| AppError::BadRequest("missing x-agent-id header".to_string()))?
        .to_str()
        .map_err(|_| AppError::BadRequest("invalid x-agent-id header".to_string()))?
        .trim()
        .to_string();

    if actor_id.is_empty() {
        return Err(AppError::BadRequest(
            "x-agent-id header cannot be empty".to_string(),
        ));
    }

    let mut tx = state.pool().begin().await?;

    let updated_object = sqlx::query_as::<_, WorldObject>(
        r#"
        UPDATE world_objects
        SET state = $1::jsonb
        WHERE id = $2
        RETURNING id, name, location_id, state, actions
        "#,
    )
    .bind(payload.state.to_string())
    .bind(&object_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let description = format!("Object {} state updated", updated_object.id);

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("object.updated")
    .bind(&actor_id)
    .bind(&updated_object.location_id)
    .bind(description)
    .bind(
        serde_json::json!({
            "object_id": updated_object.id,
            "new_state": updated_object.state,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(updated_object))
}
