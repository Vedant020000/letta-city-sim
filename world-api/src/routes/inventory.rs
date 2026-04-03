use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct TransferItemRequest {
    pub to_agent_id: String,
    pub item_id: String,
}

pub async fn transfer_item_between_agents(
    State(state): State<AppState>,
    Path(from_agent_id): Path<String>,
    Json(payload): Json<TransferItemRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if from_agent_id == payload.to_agent_id {
        return Err(AppError::BadRequest(
            "source and destination agents must be different".to_string(),
        ));
    }

    let mut tx = state.pool().begin().await?;

    let from_location = sqlx::query_scalar::<_, String>(
        r#"
        SELECT current_location_id
        FROM agents
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(&from_agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let to_location = sqlx::query_scalar::<_, String>(
        r#"
        SELECT current_location_id
        FROM agents
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(&payload.to_agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let is_adjacent = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM location_adjacency
            WHERE (from_id = $1 AND to_id = $2)
               OR (from_id = $2 AND to_id = $1)
        )
        "#,
    )
    .bind(&from_location)
    .bind(&to_location)
    .fetch_one(&mut *tx)
    .await?;

    if !is_adjacent {
        return Err(AppError::BadRequest(
            "agents must be directly adjacent to transfer items".to_string(),
        ));
    }

    let updated_item = sqlx::query_as::<_, (String, String, String)>(
        r#"
        UPDATE inventory_items
        SET held_by = $1,
            location_id = NULL
        WHERE id = $2
          AND held_by = $3
        RETURNING id, name, held_by
        "#,
    )
    .bind(&payload.to_agent_id)
    .bind(&payload.item_id)
    .bind(&from_agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest("item not found or not owned by source agent".to_string())
    })?;

    let description = format!(
        "Agent {} transferred item {} to agent {}",
        from_agent_id, updated_item.0, payload.to_agent_id
    );

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("item.transferred")
    .bind(&from_agent_id)
    .bind(&from_location)
    .bind(description)
    .bind(
        serde_json::json!({
            "to_agent_id": payload.to_agent_id,
            "item_id": updated_item.0,
            "item_name": updated_item.1,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(serde_json::json!({
        "item_id": updated_item.0,
        "item_name": updated_item.1,
        "from_agent_id": from_agent_id,
        "to_agent_id": payload.to_agent_id,
    })))
}
