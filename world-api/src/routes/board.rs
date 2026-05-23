use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AgentId;
use crate::error::{AppError, AppResult};
use crate::models::board::{BoardPost, BoardStateWithIds, PublicBoardState};
use crate::state::AppState;
use crate::ws_events::WorldEventEnvelope;

const BOARD_OBJECT_ID: &str = "notice_board_main";

#[derive(Debug, Deserialize)]
pub struct CreateBoardPostRequest {
    pub text: String,
}

#[derive(Debug, Deserialize, serde::Serialize, Default)]
struct NoticeBoardState {
    posts: Vec<BoardPost>,
}

pub async fn get_public_board(State(state): State<AppState>) -> AppResult<Json<PublicBoardState>> {
    let (location_id, board_state) = load_board_state(state.pool()).await?;

    Ok(Json(PublicBoardState {
        location_id,
        posts: board_state.posts.into_iter().map(|p| p.text).collect(),
    }))
}

pub async fn get_board_posts(State(state): State<AppState>) -> AppResult<Json<BoardStateWithIds>> {
    let (location_id, board_state) = load_board_state(state.pool()).await?;

    Ok(Json(BoardStateWithIds {
        location_id,
        posts: board_state.posts,
    }))
}

pub async fn create_board_post(
    State(state): State<AppState>,
    AgentId(actor_id): AgentId,
    Json(payload): Json<CreateBoardPostRequest>,
) -> AppResult<Json<BoardPost>> {
    let text = payload.text.trim();

    if text.is_empty() {
        return Err(AppError::BadRequest(
            "post text cannot be empty".to_string(),
        ));
    }

    let mut tx = state.pool().begin().await?;
    let (location_id, mut board_state) = load_board_state_for_update(&mut tx).await?;

    let post = BoardPost {
        id: Uuid::new_v4().to_string(),
        text: text.to_string(),
        created_at: Utc::now().to_rfc3339(),
    };

    board_state.posts.push(post.clone());
    persist_board_state(&mut tx, &board_state).await?;

    // Insert event with importance/visibility for routing
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at, importance, visibility)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7, $8)
        "#,
    )
    .bind("board.post.created")
    .bind(&actor_id)
    .bind(&location_id)
    .bind(format!("A new notice board post was created: {}", post.text))
    .bind(
        serde_json::json!({
            "post_id": post.id,
            "text": post.text,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .bind(3i16) // notable
    .bind("public")
    .execute(&mut *tx)
    .await?;

    // Route through the central event router
    let _routing_result = crate::routes::events::route_event(
        &mut tx,
        &state,
        crate::models::event::RouteEventInput {
            event_type: "board.post.created".to_string(),
            actor_id: Some(actor_id.clone()),
            location_id: Some(location_id.clone()),
            importance: 3,
            visibility: "public".to_string(),
            description: format!("A new notice board post appeared at {}: {}", location_id, post.text),
            metadata: serde_json::json!({
                "post_id": post.id,
                "text": post.text,
            }),
            target_agent_ids: vec![],
        },
    )
    .await?;

    let _ = state.event_tx().send(WorldEventEnvelope::new(
        "board.posted",
        vec![],
        Some(location_id.clone()),
        serde_json::json!({
            "post_id": post.id,
            "text": post.text,
            "created_at": post.created_at,
            "actor_id": actor_id,
        }),
    ));

    tx.commit().await?;

    Ok(Json(post))
}

pub async fn delete_board_post(
    State(state): State<AppState>,
    AgentId(actor_id): AgentId,
    Path(post_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let mut tx = state.pool().begin().await?;
    let (location_id, mut board_state) = load_board_state_for_update(&mut tx).await?;

    let initial_len = board_state.posts.len();
    board_state.posts.retain(|post| post.id != post_id);

    if board_state.posts.len() == initial_len {
        return Err(AppError::NotFound);
    }

    persist_board_state(&mut tx, &board_state).await?;

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at, importance, visibility)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7, $8)
        "#,
    )
    .bind("board.post.deleted")
    .bind(&actor_id)
    .bind(&location_id)
    .bind("A notice board post was deleted")
    .bind(serde_json::json!({ "post_id": post_id }).to_string())
    .bind(Utc::now())
    .bind(1i16) // trivial
    .bind("actor")
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(serde_json::json!({"deleted_post_id": post_id})))
}

pub async fn clear_board(
    State(state): State<AppState>,
    AgentId(actor_id): AgentId,
) -> AppResult<Json<serde_json::Value>> {
    let mut tx = state.pool().begin().await?;
    let (location_id, mut board_state) = load_board_state_for_update(&mut tx).await?;
    let removed_count = board_state.posts.len();
    board_state.posts.clear();

    persist_board_state(&mut tx, &board_state).await?;

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at, importance, visibility)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7, $8)
        "#,
    )
    .bind("board.cleared")
    .bind(&actor_id)
    .bind(&location_id)
    .bind("All notice board posts were removed")
    .bind(serde_json::json!({ "removed_count": removed_count }).to_string())
    .bind(Utc::now())
    .bind(1i16) // trivial
    .bind("actor")
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(
        serde_json::json!({"cleared": true, "removed_count": removed_count}),
    ))
}

async fn load_board_state(
    pool: &sqlx::Pool<sqlx::Postgres>,
) -> AppResult<(String, NoticeBoardState)> {
    let (location_id, state): (String, serde_json::Value) = sqlx::query_as(
        r#"
        SELECT location_id, state
        FROM world_objects
        WHERE id = $1
        "#,
    )
    .bind(BOARD_OBJECT_ID)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let board_state = serde_json::from_value::<NoticeBoardState>(state).unwrap_or_default();

    Ok((location_id, board_state))
}

async fn load_board_state_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> AppResult<(String, NoticeBoardState)> {
    let (location_id, state): (String, serde_json::Value) = sqlx::query_as(
        r#"
        SELECT location_id, state
        FROM world_objects
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(BOARD_OBJECT_ID)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let board_state = serde_json::from_value::<NoticeBoardState>(state).unwrap_or_default();

    Ok((location_id, board_state))
}

async fn persist_board_state(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    board_state: &NoticeBoardState,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE world_objects
        SET state = $1::jsonb
        WHERE id = $2
        "#,
    )
    .bind(serde_json::to_string(board_state).unwrap_or_else(|_| "{}".to_string()))
    .bind(BOARD_OBJECT_ID)
    .execute(&mut **tx)
    .await?;

    Ok(())
}
