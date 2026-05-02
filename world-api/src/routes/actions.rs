use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::auth::AgentId;
use crate::error::{AppError, AppResult};
use crate::models::agent::Agent;
use crate::models::common::ApiResponse;
use crate::routes::agents::{
    UpdateAgentLocationRequest, perform_agent_activity_update, perform_agent_activity_update_in_tx,
    perform_agent_location_update,
};
use crate::routes::board::{CreateBoardPostRequest, create_board_post};
use crate::routes::sleep::start_sleep;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SetActivityActionRequest {
    pub activity: String,
}

#[derive(Debug, Deserialize)]
pub struct CookFoodActionRequest {
    pub recipe_id: String,
    pub quantity: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct CookFoodActionResponse {
    pub agent: Agent,
    pub recipe_id: String,
    pub quantity: i32,
    pub placeholder: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ToolManifestResponse {
    pub agent_id: String,
    pub location_id: String,
    pub location_name: String,
    pub context: ToolManifestContext,
    pub tools: Vec<WorldToolDefinition>,
}

#[derive(Debug, Serialize)]
pub struct ToolManifestContext {
    pub nearby_location_ids: Vec<String>,
    pub object_ids: Vec<String>,
    pub object_action_tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct WorldToolDefinition {
    pub name: String,
    pub description: String,
    pub endpoint: String,
    pub method: String,
    pub parameters: Value,
}

pub async fn action_set_activity(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<SetActivityActionRequest>,
) -> AppResult<Json<ApiResponse<Agent>>> {
    let activity = payload.activity.trim();
    if activity.is_empty() {
        return Err(AppError::BadRequest(
            "activity cannot be empty".to_string(),
        ));
    }

    let updated = perform_agent_activity_update(&state, &agent_id, activity).await?;
    Ok(Json(ApiResponse::from(updated)))
}

pub async fn action_move_to(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<UpdateAgentLocationRequest>,
) -> AppResult<Json<ApiResponse<Agent>>> {
    let response = perform_agent_location_update(&state, &agent_id, &payload.location_id).await?;
    Ok(Json(response))
}

pub async fn action_board_post(
    state: State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<CreateBoardPostRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let Json(post) = create_board_post(state, AgentId(agent_id), Json(payload)).await?;
    Ok(Json(json!({
        "ok": true,
        "post": post,
    })))
}

pub async fn action_sleep(
    state: State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<Agent>>> {
    start_sleep(state, AgentId(agent_id)).await
}

pub async fn action_cook_food(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<CookFoodActionRequest>,
) -> AppResult<Json<ApiResponse<CookFoodActionResponse>>> {
    let recipe_id = payload.recipe_id.trim();
    if recipe_id.is_empty() {
        return Err(AppError::BadRequest(
            "recipe_id cannot be empty".to_string(),
        ));
    }

    let quantity = payload.quantity.unwrap_or(1);
    if quantity <= 0 {
        return Err(AppError::BadRequest(
            "quantity must be greater than 0".to_string(),
        ));
    }

    let mut tx = state.pool().begin().await?;

    let updated_agent = perform_agent_activity_update_in_tx(
        &mut tx,
        &agent_id,
        &format!("Cooking {}", recipe_id),
    )
    .await?;

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.cook.started")
    .bind(&updated_agent.id)
    .bind(&updated_agent.current_location_id)
    .bind(format!(
        "Agent {} started cooking {}",
        updated_agent.id, recipe_id
    ))
    .bind(
        json!({
            "recipe_id": recipe_id,
            "quantity": quantity,
            "placeholder": true,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(CookFoodActionResponse {
        agent: updated_agent,
        recipe_id: recipe_id.to_string(),
        quantity,
        placeholder: true,
        message: "Cook food is currently a server-owned placeholder action that marks the agent as cooking."
            .to_string(),
    })))
}

pub async fn get_tool_manifest(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> AppResult<Json<ApiResponse<ToolManifestResponse>>> {
    let agent_row = sqlx::query_as::<_, (String, String, String)>(
        r#"
        SELECT a.id, a.current_location_id, l.name
        FROM agents a
        JOIN locations l ON l.id = a.current_location_id
        WHERE a.id = $1 OR a.letta_agent_id = $1
        LIMIT 1
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    let nearby_location_ids = sqlx::query_scalar::<_, String>(
        r#"
        SELECT to_id
        FROM location_adjacency
        WHERE from_id = $1
        ORDER BY to_id
        "#,
    )
    .bind(&agent_row.1)
    .fetch_all(state.pool())
    .await?;

    let objects = sqlx::query_as::<_, (String, String, Vec<String>)>(
        r#"
        SELECT id, name, actions
        FROM world_objects
        WHERE location_id = $1
        ORDER BY id
        "#,
    )
    .bind(&agent_row.1)
    .fetch_all(state.pool())
    .await?;

    let mut object_ids = Vec::new();
    let mut action_tags = std::collections::BTreeSet::new();
    let mut has_sleep = false;
    let mut has_board = false;
    let mut has_cook = false;

    for (id, name, actions) in &objects {
        object_ids.push(id.clone());
        for action in actions {
            action_tags.insert(action.clone());
        }

        let lower_name = name.to_lowercase();
        if actions.iter().any(|action| action == "sleep") {
            has_sleep = true;
        }
        if lower_name.contains("board") || id.contains("board") {
            has_board = true;
        }
        if lower_name.contains("stove")
            || lower_name.contains("kitchen")
            || lower_name.contains("cafe")
            || actions.iter().any(|action| action == "cook")
        {
            has_cook = true;
        }
    }

    let mut tools = vec![tool_set_activity(), tool_move_to()];
    if has_sleep {
        tools.push(tool_sleep());
    }
    if has_board {
        tools.push(tool_board_post());
    }
    if has_cook || agent_row.2.to_lowercase().contains("cafe") {
        tools.push(tool_cook_food());
    }

    Ok(Json(ApiResponse::from(ToolManifestResponse {
        agent_id: agent_row.0,
        location_id: agent_row.1,
        location_name: agent_row.2,
        context: ToolManifestContext {
            nearby_location_ids,
            object_ids,
            object_action_tags: action_tags.into_iter().collect(),
        },
        tools,
    })))
}

fn tool_set_activity() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "set_activity".to_string(),
        description: "Set the agent's current activity in the world state.".to_string(),
        endpoint: "/actions/set_activity".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "activity": {
                    "type": "string",
                    "description": "The activity label to set for the agent."
                }
            },
            "required": ["activity"]
        }),
    }
}

fn tool_move_to() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "move_to".to_string(),
        description: "Move the agent to a destination location.".to_string(),
        endpoint: "/actions/move_to".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "location_id": {
                    "type": "string",
                    "description": "Destination location id."
                }
            },
            "required": ["location_id"]
        }),
    }
}

fn tool_sleep() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "sleep".to_string(),
        description: "Go to sleep if the current location has a valid bed.".to_string(),
        endpoint: "/actions/sleep".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

fn tool_board_post() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "board_post".to_string(),
        description: "Create a notice board post at the current location if a board is available.".to_string(),
        endpoint: "/actions/board_post".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Notice board post text."
                }
            },
            "required": ["text"]
        }),
    }
}

fn tool_cook_food() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "cook_food".to_string(),
        description: "Cook food at the current location using a server-owned placeholder cooking action.".to_string(),
        endpoint: "/actions/cook_food".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "recipe_id": {
                    "type": "string",
                    "description": "Recipe identifier to cook."
                },
                "quantity": {
                    "type": "integer",
                    "description": "How many to cook.",
                    "minimum": 1
                }
            },
            "required": ["recipe_id"]
        }),
    }
}
