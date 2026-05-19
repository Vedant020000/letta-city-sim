use axum::{
    Json,
    extract::{Path, State},
};

use crate::auth::AgentId;
use crate::error::{AppError, AppResult};
use crate::models::common::ApiResponse;
use crate::models::location::{
    AdjacentLocation, AgentLocationRole, AgentLocationsResponse, Location, LocationDetailResponse,
    LocationRole,
};
use crate::state::AppState;

use serde_json::json;

pub async fn list_locations(State(state): State<AppState>) -> AppResult<Json<Vec<Location>>> {
    let locations = sqlx::query_as::<_, Location>(
        r#"
        SELECT id, name, description, map_x, map_y, kind, capacity
        FROM locations
        ORDER BY name
        "#,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(locations))
}

pub async fn get_location_by_id(
    State(state): State<AppState>,
    Path(location_id): Path<String>,
) -> AppResult<Json<LocationDetailResponse>> {
    let location = sqlx::query_as::<_, Location>(
        r#"
        SELECT id, name, description, map_x, map_y, kind, capacity
        FROM locations
        WHERE id = $1
        "#,
    )
    .bind(&location_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    let nearby = sqlx::query_as::<_, AdjacentLocation>(
        r#"
        SELECT l.id, l.name, l.description, l.map_x, l.map_y, la.travel_secs
        FROM location_adjacency la
        JOIN locations l ON l.id = la.to_id
        WHERE la.from_id = $1
        ORDER BY la.travel_secs ASC, l.name ASC
        "#,
    )
    .bind(&location_id)
    .fetch_all(state.pool())
    .await?;

    let roles = sqlx::query_as::<_, LocationRole>(
        r#"
        SELECT lr.location_id, lr.agent_id, lr.role, a.name as agent_name, lr.created_at
        FROM location_roles lr
        JOIN agents a ON a.id = lr.agent_id
        WHERE lr.location_id = $1
        ORDER BY lr.role, a.name
        "#,
    )
    .bind(&location_id)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(LocationDetailResponse { location, nearby, roles }))
}

pub async fn get_nearby_locations(
    State(state): State<AppState>,
    Path(location_id): Path<String>,
) -> AppResult<Json<Vec<AdjacentLocation>>> {
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

    let nearby = sqlx::query_as::<_, AdjacentLocation>(
        r#"
        SELECT l.id, l.name, l.description, l.map_x, l.map_y, la.travel_secs
        FROM location_adjacency la
        JOIN locations l ON l.id = la.to_id
        WHERE la.from_id = $1
        ORDER BY la.travel_secs ASC, l.name ASC
        "#,
    )
    .bind(&location_id)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(nearby))
}

pub async fn get_agent_locations(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> AppResult<Json<AgentLocationsResponse>> {
    let agent = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT id, home_location_id FROM agents WHERE id = $1"
    )
    .bind(&agent_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    let roles = sqlx::query_as::<_, AgentLocationRole>(
        r#"
        SELECT lr.location_id, l.name as location_name, l.kind as location_kind, lr.role, lr.created_at
        FROM location_roles lr
        JOIN locations l ON l.id = lr.location_id
        WHERE lr.agent_id = $1
        ORDER BY lr.role, l.name
        "#,
    )
    .bind(&agent_id)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(AgentLocationsResponse {
        agent_id: agent.0,
        home_location_id: agent.1,
        roles,
    }))
}

pub async fn action_check_location_roles(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let location_id: String = sqlx::query_scalar("SELECT current_location_id FROM agents WHERE id = $1")
        .bind(&agent_id)
        .fetch_one(state.pool())
        .await?;

    let roles = sqlx::query_as::<_, LocationRole>(
        r#"
        SELECT lr.location_id, lr.agent_id, lr.role, a.name as agent_name, lr.created_at
        FROM location_roles lr
        JOIN agents a ON a.id = lr.agent_id
        WHERE lr.location_id = $1
        ORDER BY lr.role, a.name
        "#,
    )
    .bind(&location_id)
    .fetch_all(state.pool())
    .await?;

    let location_name: String = sqlx::query_scalar("SELECT name FROM locations WHERE id = $1")
        .bind(&location_id)
        .fetch_one(state.pool())
        .await?;

    Ok(Json(ApiResponse::from(json!({
        "location_id": location_id,
        "location_name": location_name,
        "roles": roles,
    }))))
}
