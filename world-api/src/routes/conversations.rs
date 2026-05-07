use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::auth::AgentId;
use crate::error::{AppError, AppResult};
use crate::models::common::ApiResponse;
use crate::models::conversation::{
    ConversationDetail, ConversationMessage, ConversationParticipant, ConversationSummary,
};
use crate::routes::citizens::enqueue_citizen_wake_tx;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListConversationsQuery {
    pub location_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateConversationRequest {
    pub topic: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct JoinConversationRequest {
    pub conversation_id: String,
}

#[derive(Debug, Deserialize)]
pub struct AcceptRequestRequest {
    pub conversation_id: String,
    pub requester_agent_id: String,
}

#[derive(Debug, Deserialize)]
pub struct AcceptInviteRequest {
    pub conversation_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SpeakToRequest {
    pub target_agent_id: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Public route handlers
// ---------------------------------------------------------------------------

pub async fn list_active_conversations(
    State(state): State<AppState>,
    Query(query): Query<ListConversationsQuery>,
) -> AppResult<Json<ApiResponse<Vec<ConversationSummary>>>> {
    let conversations = if let Some(location_id) = query.location_id {
        sqlx::query_as::<_, ConversationSummary>(
            r#"
            SELECT
                c.id,
                c.location_id,
                c.topic,
                COUNT(cp.agent_id) AS participant_count,
                COUNT(cp.agent_id) FILTER (WHERE cp.status = 'active') AS active_participant_count,
                (SELECT content FROM conversation_messages WHERE conversation_id = c.id ORDER BY sent_at DESC LIMIT 1) AS last_message_preview,
                c.started_at
            FROM conversations c
            LEFT JOIN conversation_participants cp ON cp.conversation_id = c.id
            WHERE c.location_id = $1
              AND c.ended_at IS NULL
            GROUP BY c.id
            ORDER BY c.started_at DESC
            "#,
        )
        .bind(&location_id)
        .fetch_all(state.pool())
        .await?
    } else {
        sqlx::query_as::<_, ConversationSummary>(
            r#"
            SELECT
                c.id,
                c.location_id,
                c.topic,
                COUNT(cp.agent_id) AS participant_count,
                COUNT(cp.agent_id) FILTER (WHERE cp.status = 'active') AS active_participant_count,
                (SELECT content FROM conversation_messages WHERE conversation_id = c.id ORDER BY sent_at DESC LIMIT 1) AS last_message_preview,
                c.started_at
            FROM conversations c
            LEFT JOIN conversation_participants cp ON cp.conversation_id = c.id
            WHERE c.ended_at IS NULL
            GROUP BY c.id
            ORDER BY c.started_at DESC
            "#,
        )
        .fetch_all(state.pool())
        .await?
    };

    Ok(Json(ApiResponse::from(conversations)))
}

pub async fn get_conversation_detail(
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
) -> AppResult<Json<ApiResponse<ConversationDetail>>> {
    let conversation = sqlx::query_as::<_, (String, String, Option<String>, chrono::DateTime<Utc>)>(
        r#"
        SELECT id, location_id, topic, started_at
        FROM conversations
        WHERE id = $1
        "#,
    )
    .bind(&conversation_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    let participants = sqlx::query_as::<_, ConversationParticipant>(
        r#"
        SELECT cp.agent_id, a.name AS agent_name, cp.status, cp.joined_at
        FROM conversation_participants cp
        JOIN agents a ON a.id = cp.agent_id
        WHERE cp.conversation_id = $1
        ORDER BY cp.joined_at
        "#,
    )
    .bind(&conversation_id)
    .fetch_all(state.pool())
    .await?;

    let messages = sqlx::query_as::<_, ConversationMessage>(
        r#"
        SELECT cm.id, cm.conversation_id, cm.agent_id, a.name AS sender_name, cm.content, cm.sent_at
        FROM conversation_messages cm
        JOIN agents a ON a.id = cm.agent_id
        WHERE cm.conversation_id = $1
        ORDER BY cm.sent_at DESC
        LIMIT 50
        "#,
    )
    .bind(&conversation_id)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(ConversationDetail {
        id: conversation.0,
        location_id: conversation.1,
        topic: conversation.2,
        started_at: conversation.3,
        participants,
        messages,
    })))
}

// ---------------------------------------------------------------------------
// Action handlers (called from actions.rs)
// ---------------------------------------------------------------------------

pub async fn action_speak_to(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<SpeakToRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let target_id = payload.target_agent_id.trim();
    let message = payload.message.trim();

    if target_id.is_empty() {
        return Err(AppError::BadRequest("target_agent_id cannot be empty".to_string()));
    }
    if message.is_empty() {
        return Err(AppError::BadRequest("message cannot be empty".to_string()));
    }
    if target_id == agent_id {
        return Err(AppError::BadRequest("cannot speak to yourself".to_string()));
    }

    let mut tx = state.pool().begin().await?;

    // Verify both agents are in the same location and target is not sleeping
    let location_check = sqlx::query_as::<_, (String, String, String)>(
        r#"
        SELECT
            a1.current_location_id,
            a1.name,
            a2.state
        FROM agents a1
        JOIN agents a2 ON a2.id = $2
        WHERE a1.id = $1
        "#,
    )
    .bind(&agent_id)
    .bind(&target_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    if location_check.2 == "sleeping" {
        return Err(AppError::BadRequest(
            format!("{} is sleeping and cannot be disturbed.", target_id)
        ));
    }

    let location_id = location_check.0;
    let sender_name = location_check.1;

    // Find existing 1:1 conversation between these two agents, or create one
    let conversation_id = find_or_create_1on1_conversation(&mut tx, &location_id, &agent_id, &target_id).await?;

    // Add the message
    add_message_tx(&mut tx, &conversation_id, &agent_id, message).await?;

    tx.commit().await?;

    // Wake the target agent
    let _ = enqueue_citizen_wake_tx(
        &mut state.pool().begin().await?,
        &target_id,
        "conversation",
        serde_json::json!({
            "kind": "conversation",
            "ref": "conversation.message",
            "details": {
                "conversation_id": &conversation_id,
                "sender_id": &agent_id,
                "sender_name": &sender_name,
            }
        }),
        format!("{} says: {}", sender_name, message),
        serde_json::json!({
            "event_type": "conversation.message",
            "conversation_id": &conversation_id,
            "sender_id": &agent_id,
            "sender_name": &sender_name,
            "message": message,
        }),
        serde_json::json!([]),
        true,
    )
    .await?;

    Ok(Json(ApiResponse::from(serde_json::json!({
        "conversation_id": conversation_id,
        "message": "Message sent.",
    }))))
}

pub async fn action_join_conversation(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<JoinConversationRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let conversation_id = payload.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err(AppError::BadRequest("conversation_id cannot be empty".to_string()));
    }

    let mut tx = state.pool().begin().await?;

    // Verify conversation exists and agent is in same location
    let conv = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT c.id, c.location_id
        FROM conversations c
        WHERE c.id = $1 AND c.ended_at IS NULL
        "#,
    )
    .bind(&conversation_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let agent_loc = sqlx::query_scalar::<_, String>(
        r#"
        SELECT current_location_id FROM agents WHERE id = $1
        "#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if agent_loc != conv.1 {
        return Err(AppError::BadRequest(
            "You must be in the same location as the conversation to join.".to_string()
        ));
    }

    // Check current participant status
    let existing = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT status FROM conversation_participants
        WHERE conversation_id = $1 AND agent_id = $2
        "#,
    )
    .bind(&conversation_id)
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?;

    match existing.as_deref() {
        Some("active") => {
            tx.commit().await?;
            return Ok(Json(ApiResponse::from(serde_json::json!({
                "status": "already_joined",
                "message": "You are already in this conversation.",
            }))));
        }
        Some("invited") => {
            // Accept the invitation
            sqlx::query(
                r#"
                UPDATE conversation_participants
                SET status = 'active', joined_at = NOW()
                WHERE conversation_id = $1 AND agent_id = $2
                "#,
            )
            .bind(&conversation_id)
            .bind(&agent_id)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;

            return Ok(Json(ApiResponse::from(serde_json::json!({
                "status": "joined",
                "message": "You have joined the conversation.",
            }))));
        }
        Some("requested") => {
            tx.commit().await?;
            return Ok(Json(ApiResponse::from(serde_json::json!({
                "status": "pending",
                "message": "Your request to join is still pending approval.",
            }))));
        }
        _ => {
            // New join request
            sqlx::query(
                r#"
                INSERT INTO conversation_participants (conversation_id, agent_id, status)
                VALUES ($1, $2, 'requested')
                ON CONFLICT (conversation_id, agent_id) DO UPDATE SET status = 'requested', left_at = NULL
                "#,
            )
            .bind(&conversation_id)
            .bind(&agent_id)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;

            // Wake all active participants to ask them to approve
            let active_participants = sqlx::query_scalar::<_, String>(
                r#"
                SELECT agent_id FROM conversation_participants
                WHERE conversation_id = $1 AND status = 'active' AND agent_id != $2
                "#,
            )
            .bind(&conversation_id)
            .bind(&agent_id)
            .fetch_all(state.pool())
            .await?;

            let requester_name = sqlx::query_scalar::<_, String>(
                r#"
                SELECT name FROM agents WHERE id = $1
                "#,
            )
            .bind(&agent_id)
            .fetch_one(state.pool())
            .await?;

            for participant_id in active_participants {
                let _ = enqueue_citizen_wake_tx(
                    &mut state.pool().begin().await?,
                    &participant_id,
                    "conversation",
                    serde_json::json!({
                        "kind": "conversation",
                        "ref": "conversation.join_request",
                        "details": {
                            "conversation_id": &conversation_id,
                            "requester_id": &agent_id,
                            "requester_name": &requester_name,
                        }
                    }),
                    format!("{} wants to join your conversation. Do you accept?", requester_name),
                    serde_json::json!({
                        "event_type": "conversation.join_request",
                        "conversation_id": &conversation_id,
                        "requester_id": &agent_id,
                        "requester_name": &requester_name,
                    }),
                    serde_json::json!([]),
                    true,
                )
                .await?;
            }

            return Ok(Json(ApiResponse::from(serde_json::json!({
                "status": "requested",
                "message": "Join request sent. Waiting for approval from current participants.",
            }))));
        }
    }
}

pub async fn action_leave_conversation(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<JoinConversationRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let conversation_id = payload.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err(AppError::BadRequest("conversation_id cannot be empty".to_string()));
    }

    let mut tx = state.pool().begin().await?;

    sqlx::query(
        r#"
        UPDATE conversation_participants
        SET left_at = NOW()
        WHERE conversation_id = $1 AND agent_id = $2
        "#,
    )
    .bind(&conversation_id)
    .bind(&agent_id)
    .execute(&mut *tx)
    .await?;

    // Auto-close if no active participants remain
    let remaining_active = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM conversation_participants
        WHERE conversation_id = $1 AND status = 'active' AND left_at IS NULL
        "#,
    )
    .bind(&conversation_id)
    .fetch_one(&mut *tx)
    .await?;

    if remaining_active == 0 {
        sqlx::query(
            r#"
            UPDATE conversations SET ended_at = NOW() WHERE id = $1
            "#,
        )
        .bind(&conversation_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(Json(ApiResponse::from(serde_json::json!({
        "status": "left",
        "message": "You have left the conversation.",
    }))))
}

pub async fn action_send_message(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<SendMessageRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let content = payload.content.trim();
    if content.is_empty() {
        return Err(AppError::BadRequest("message content cannot be empty".to_string()));
    }

    let mut tx = state.pool().begin().await?;

    // Find an active conversation the agent is in
    let conversation_id = sqlx::query_scalar::<_, String>(
        r#"
        SELECT conversation_id FROM conversation_participants
        WHERE agent_id = $1 AND status = 'active' AND left_at IS NULL
        ORDER BY joined_at DESC
        LIMIT 1
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(conversation_id) = conversation_id else {
        return Err(AppError::BadRequest("You are not in any active conversation.".to_string()));
    };

    add_message_tx(&mut tx, &conversation_id, &agent_id, content).await?;

    let sender_name = sqlx::query_scalar::<_, String>(
        r#"
        SELECT name FROM agents WHERE id = $1
        "#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    // Wake all other active participants
    let other_participants = sqlx::query_scalar::<_, String>(
        r#"
        SELECT agent_id FROM conversation_participants
        WHERE conversation_id = $1 AND status = 'active' AND agent_id != $2 AND left_at IS NULL
        "#,
    )
    .bind(&conversation_id)
    .bind(&agent_id)
    .fetch_all(state.pool())
    .await?;

    for participant_id in other_participants {
        let _ = enqueue_citizen_wake_tx(
            &mut state.pool().begin().await?,
            &participant_id,
            "conversation",
            serde_json::json!({
                "kind": "conversation",
                "ref": "conversation.message",
                "details": {
                    "conversation_id": &conversation_id,
                    "sender_id": &agent_id,
                    "sender_name": &sender_name,
                }
            }),
            format!("{} says: {}", sender_name, content),
            serde_json::json!({
                "event_type": "conversation.message",
                "conversation_id": &conversation_id,
                "sender_id": &agent_id,
                "sender_name": &sender_name,
                "message": content,
            }),
            serde_json::json!([]),
            true,
        )
        .await?;
    }

    Ok(Json(ApiResponse::from(serde_json::json!({
        "conversation_id": conversation_id,
        "status": "sent",
        "message": "Message sent.",
    }))))
}

pub async fn action_accept_join_request(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<AcceptRequestRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let conversation_id = payload.conversation_id.trim();
    let requester_id = payload.requester_agent_id.trim();

    if conversation_id.is_empty() || requester_id.is_empty() {
        return Err(AppError::BadRequest("conversation_id and requester_agent_id are required".to_string()));
    }

    let mut tx = state.pool().begin().await?;

    // Verify the accepting agent is an active participant
    let is_participant = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM conversation_participants
            WHERE conversation_id = $1 AND agent_id = $2 AND status = 'active' AND left_at IS NULL
        )
        "#,
    )
    .bind(&conversation_id)
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if !is_participant {
        return Err(AppError::Forbidden);
    }

    // Verify the requester has a pending request
    let has_request = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM conversation_participants
            WHERE conversation_id = $1 AND agent_id = $2 AND status = 'requested'
        )
        "#,
    )
    .bind(&conversation_id)
    .bind(&requester_id)
    .fetch_one(&mut *tx)
    .await?;

    if !has_request {
        return Err(AppError::BadRequest("No pending join request from that agent.".to_string()));
    }

    // Approve the request
    sqlx::query(
        r#"
        UPDATE conversation_participants
        SET status = 'active', joined_at = NOW()
        WHERE conversation_id = $1 AND agent_id = $2
        "#,
    )
    .bind(&conversation_id)
    .bind(&requester_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // Wake the requester to tell them they're in
    let accepter_name = sqlx::query_scalar::<_, String>(
        r#"
        SELECT name FROM agents WHERE id = $1
        "#,
    )
    .bind(&agent_id)
    .fetch_one(state.pool())
    .await?;

    let _ = enqueue_citizen_wake_tx(
        &mut state.pool().begin().await?,
        &requester_id,
        "conversation",
        serde_json::json!({
            "kind": "conversation",
            "ref": "conversation.join_accepted",
            "details": {
                "conversation_id": &conversation_id,
                "accepter_id": &agent_id,
                "accepter_name": &accepter_name,
            }
        }),
        format!("{} has accepted you into the conversation.", accepter_name),
        serde_json::json!({
            "event_type": "conversation.join_accepted",
            "conversation_id": &conversation_id,
            "accepter_id": &agent_id,
            "accepter_name": &accepter_name,
        }),
        serde_json::json!([]),
        true,
    )
    .await?;

    Ok(Json(ApiResponse::from(serde_json::json!({
        "status": "accepted",
        "message": "Join request accepted.",
    }))))
}

pub async fn action_accept_invitation(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<AcceptInviteRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let conversation_id = payload.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err(AppError::BadRequest("conversation_id cannot be empty".to_string()));
    }

    let mut tx = state.pool().begin().await?;

    let has_invite = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM conversation_participants
            WHERE conversation_id = $1 AND agent_id = $2 AND status = 'invited'
        )
        "#,
    )
    .bind(&conversation_id)
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if !has_invite {
        return Err(AppError::BadRequest("You have no pending invitation to this conversation.".to_string()));
    }

    sqlx::query(
        r#"
        UPDATE conversation_participants
        SET status = 'active', joined_at = NOW()
        WHERE conversation_id = $1 AND agent_id = $2
        "#,
    )
    .bind(&conversation_id)
    .bind(&agent_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(serde_json::json!({
        "status": "joined",
        "message": "You have accepted the invitation and joined the conversation.",
    }))))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async fn find_or_create_1on1_conversation(
    tx: &mut Transaction<'_, Postgres>,
    location_id: &str,
    agent_a: &str,
    agent_b: &str,
) -> AppResult<String> {
    // Look for an existing active 1:1 conversation between these two agents
    let existing = sqlx::query_scalar::<_, String>(
        r#"
        SELECT c.id
        FROM conversations c
        WHERE c.location_id = $1
          AND c.ended_at IS NULL
          AND (
            SELECT COUNT(*) FROM conversation_participants
            WHERE conversation_id = c.id AND status = 'active' AND left_at IS NULL
          ) = 2
          AND EXISTS (
            SELECT 1 FROM conversation_participants
            WHERE conversation_id = c.id AND agent_id = $2 AND status = 'active' AND left_at IS NULL
          )
          AND EXISTS (
            SELECT 1 FROM conversation_participants
            WHERE conversation_id = c.id AND agent_id = $3 AND status = 'active' AND left_at IS NULL
          )
        LIMIT 1
        "#,
    )
    .bind(location_id)
    .bind(agent_a)
    .bind(agent_b)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(id) = existing {
        return Ok(id);
    }

    // Create new conversation
    let conversation_id = format!("conv_{}", Uuid::new_v4().simple());

    sqlx::query(
        r#"
        INSERT INTO conversations (id, location_id, topic)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(&conversation_id)
    .bind(location_id)
    .bind("Private conversation")
    .execute(&mut **tx)
    .await?;

    for agent in [agent_a, agent_b] {
        sqlx::query(
            r#"
            INSERT INTO conversation_participants (conversation_id, agent_id, status)
            VALUES ($1, $2, 'active')
            "#,
        )
        .bind(&conversation_id)
        .bind(agent)
        .execute(&mut **tx)
        .await?;
    }

    Ok(conversation_id)
}

async fn add_message_tx(
    tx: &mut Transaction<'_, Postgres>,
    conversation_id: &str,
    agent_id: &str,
    content: &str,
) -> AppResult<()> {
    let message_id = format!("msg_{}", Uuid::new_v4().simple());

    sqlx::query(
        r#"
        INSERT INTO conversation_messages (id, conversation_id, agent_id, content, sent_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(&message_id)
    .bind(conversation_id)
    .bind(agent_id)
    .bind(content)
    .bind(Utc::now())
    .execute(&mut **tx)
    .await?;

    Ok(())
}
