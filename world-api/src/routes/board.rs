use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::board::{BoardPost, BoardStateWithIds, PublicBoardState};
use crate::state::AppState;

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
    headers: HeaderMap,
    Json(payload): Json<CreateBoardPostRequest>,
) -> AppResult<Json<BoardPost>> {
    let actor_id = parse_actor_id(&headers)?;
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

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("board.post.created")
    .bind(&actor_id)
    .bind(&location_id)
    .bind("A new notice board post was created")
    .bind(
        serde_json::json!({
            "post_id": post.id,
            "text": post.text,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(post))
}

pub async fn delete_board_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(post_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let actor_id = parse_actor_id(&headers)?;

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
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("board.post.deleted")
    .bind(&actor_id)
    .bind(&location_id)
    .bind("A notice board post was deleted")
    .bind(serde_json::json!({ "post_id": post_id }).to_string())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(serde_json::json!({"deleted_post_id": post_id})))
}

pub async fn clear_board(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    let actor_id = parse_actor_id(&headers)?;

    let mut tx = state.pool().begin().await?;
    let (location_id, mut board_state) = load_board_state_for_update(&mut tx).await?;
    let removed_count = board_state.posts.len();
    board_state.posts.clear();

    persist_board_state(&mut tx, &board_state).await?;

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("board.cleared")
    .bind(&actor_id)
    .bind(&location_id)
    .bind("All notice board posts were removed")
    .bind(serde_json::json!({ "removed_count": removed_count }).to_string())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(
        serde_json::json!({"cleared": true, "removed_count": removed_count}),
    ))
}

fn parse_actor_id(headers: &HeaderMap) -> AppResult<String> {
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

    Ok(actor_id)
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
