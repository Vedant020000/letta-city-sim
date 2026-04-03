use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::models::inventory::InventoryItem;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AddInventoryItemRequest {
    pub item_id: String,
}

#[derive(Debug, Deserialize)]
pub struct RemoveInventoryItemRequest {
    pub item_id: String,
}

#[derive(Debug, Deserialize)]
pub struct TransferItemRequest {
    pub to_agent_id: String,
    pub item_id: String,
}

pub async fn get_agent_inventory(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> AppResult<Json<Vec<InventoryItem>>> {
    let exists = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM agents
        WHERE id = $1
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(state.pool())
    .await?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    let items = sqlx::query_as::<_, InventoryItem>(
        r#"
        SELECT id, name, held_by, location_id, state
        FROM inventory_items
        WHERE held_by = $1
        ORDER BY name
        "#,
    )
    .bind(&agent_id)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(items))
}

pub async fn add_item_to_agent_inventory(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(payload): Json<AddInventoryItemRequest>,
) -> AppResult<Json<InventoryItem>> {
    let mut tx = state.pool().begin().await?;

    let agent_location = sqlx::query_scalar::<_, String>(
        r#"
        SELECT current_location_id
        FROM agents
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let updated_item = sqlx::query_as::<_, InventoryItem>(
        r#"
        UPDATE inventory_items
        SET held_by = $1,
            location_id = NULL
        WHERE id = $2
          AND location_id = $3
        RETURNING id, name, held_by, location_id, state
        "#,
    )
    .bind(&agent_id)
    .bind(&payload.item_id)
    .bind(&agent_location)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest("item is not available at agent's current location".to_string())
    })?;

    let description = format!("Agent {} picked up item {}", agent_id, updated_item.id);

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("item.picked_up")
    .bind(&agent_id)
    .bind(&agent_location)
    .bind(description)
    .bind(
        serde_json::json!({
            "item_id": updated_item.id,
            "item_name": updated_item.name,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(updated_item))
}

pub async fn remove_item_from_agent_inventory(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(payload): Json<RemoveInventoryItemRequest>,
) -> AppResult<Json<InventoryItem>> {
    let mut tx = state.pool().begin().await?;

    let agent_location = sqlx::query_scalar::<_, String>(
        r#"
        SELECT current_location_id
        FROM agents
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let updated_item = sqlx::query_as::<_, InventoryItem>(
        r#"
        UPDATE inventory_items
        SET held_by = NULL,
            location_id = $1
        WHERE id = $2
          AND held_by = $3
        RETURNING id, name, held_by, location_id, state
        "#,
    )
    .bind(&agent_location)
    .bind(&payload.item_id)
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::BadRequest("item is not owned by this agent".to_string()))?;

    let description = format!("Agent {} dropped item {}", agent_id, updated_item.id);

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("item.dropped")
    .bind(&agent_id)
    .bind(&agent_location)
    .bind(description)
    .bind(
        serde_json::json!({
            "item_id": updated_item.id,
            "item_name": updated_item.name,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(updated_item))
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
