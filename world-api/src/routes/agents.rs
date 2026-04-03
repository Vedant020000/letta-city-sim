use axum::{
    Json,
    extract::{Path, State},
};

use crate::error::AppError;
use crate::error::AppResult;
use crate::models::agent::Agent;
use crate::state::AppState;

pub async fn list_agents(State(state): State<AppState>) -> AppResult<Json<Vec<Agent>>> {
    let agents = sqlx::query_as::<_, Agent>(
        r#"
        SELECT
            id,
            name,
            occupation,
            current_location_id,
            state,
            current_activity,
            is_npc,
            is_active,
            state_updated_at
        FROM agents
        ORDER BY name
        "#,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(agents))
}

pub async fn get_agent_by_id(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> AppResult<Json<Agent>> {
    let agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT
            id,
            name,
            occupation,
            current_location_id,
            state,
            current_activity,
            is_npc,
            is_active,
            state_updated_at
        FROM agents
        WHERE id = $1
        "#,
    )
    .bind(agent_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(agent))
}
