use axum::{
    Json,
    extract::{
        Path, State,
        rejection::JsonRejection,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    auth::{AuthContext, SimKey},
    error::{AppError, AppResult},
    models::common::ApiResponse,
    routes::agents::perform_agent_activity_update_in_tx,
    state::AppState,
};

const HEADER_AGENT_ID: &str = "x-agent-id";
const HEADER_WAKE_TOKEN: &str = "x-wake-token";
const WAKE_QUEUE_CAP: i64 = 16;
const WAKE_TOKEN_TTL_SECONDS: i64 = 300;

#[derive(Debug, Deserialize)]
pub struct CitizenActionRequest {
    pub action: String,
    #[serde(default = "default_args")]
    pub args: Value,
    pub client_event_id: String,
    pub wake_event_id: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct AdminTestWakeRequest {
    pub narrative: Option<String>,
    pub structured: Option<Value>,
    pub expects_response: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct TestCitizenWakeResponse {
    pub event_id: String,
    pub agent_id: String,
    pub seq: i64,
    pub status: String,
    pub wake_token_expires_at: String,
}

#[derive(Debug, Clone)]
pub struct EnqueuedCitizenWake {
    pub event_id: String,
    pub seq: i64,
    pub status: String,
    pub wake_token_expires_at: String,
    pub should_signal: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct OpenWakeRow {
    event_id: String,
    seq: i64,
    wake_type: String,
    world_time: DateTime<Utc>,
    wall_time: DateTime<Utc>,
    agent_snapshot: Value,
    trigger_payload: Value,
    prompt_narrative: String,
    prompt_structured: Option<Value>,
    tools: Value,
    wake_token_expires_at: DateTime<Utc>,
    expects_response: bool,
    dropped_for_overflow_count: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct LockedWakeRow {
    event_id: String,
    agent_id: String,
    status: String,
    seq: i64,
    wake_token_expires_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct CachedReceiptRow {
    response_status: i32,
    response_body: Value,
}

#[derive(Debug, sqlx::FromRow)]
struct AgentWakeSnapshotRow {
    id: String,
    name: String,
    current_location_id: String,
    location_name: String,
}

pub async fn ws_citizen(
    State(state): State<AppState>,
    auth: AuthContext,
    ws: WebSocketUpgrade,
) -> AppResult<impl IntoResponse> {
    let agent_id = auth.agent_id().ok_or(AppError::Forbidden)?.to_string();

    Ok(ws.on_upgrade(move |socket| handle_citizen_socket(state, agent_id, socket)))
}

pub async fn citizen_action(
    State(state): State<AppState>,
    auth: AuthContext,
    headers: HeaderMap,
    payload: Result<Json<CitizenActionRequest>, JsonRejection>,
) -> Response {
    let Some(resolved_agent_id) = auth.agent_id().map(str::to_string) else {
        return citizen_protocol_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Citizen actions require bearer-token agent auth.",
            false,
            false,
            None,
        );
    };

    let header_agent_id = match required_header(&headers, HEADER_AGENT_ID) {
        Ok(value) => value,
        Err(message) => {
            return citizen_protocol_response(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                &message,
                false,
                false,
                None,
            );
        }
    };

    if header_agent_id != resolved_agent_id {
        return citizen_protocol_response(
            StatusCode::FORBIDDEN,
            "agent_mismatch",
            "x-agent-id does not match the authenticated citizen identity.",
            false,
            false,
            None,
        );
    }

    let wake_token = match required_header(&headers, HEADER_WAKE_TOKEN) {
        Ok(value) => value,
        Err(message) => {
            return citizen_protocol_response(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                &message,
                false,
                false,
                None,
            );
        }
    };

    let payload = match payload {
        Ok(Json(payload)) => payload,
        Err(_) => {
            return citizen_protocol_response(
                StatusCode::BAD_REQUEST,
                "invalid_json",
                "Request body must be valid JSON.",
                false,
                false,
                None,
            );
        }
    };

    if let Some(cached) = match load_cached_receipt(state.pool(), &resolved_agent_id, &payload.client_event_id).await {
        Ok(value) => value,
        Err(err) => return citizen_internal_error_response(err),
    } {
        return cached_receipt_to_response(cached);
    }

    let mut tx = match state.pool().begin().await {
        Ok(tx) => tx,
        Err(err) => return citizen_internal_error_response(err.into()),
    };

    let locked_wake = match lock_wake_for_action(&mut tx, &resolved_agent_id, &payload.wake_event_id).await {
        Ok(Some(wake)) => wake,
        Ok(None) => {
            return citizen_protocol_response(
                StatusCode::CONFLICT,
                "wake_closed",
                "That wake is no longer open.",
                true,
                true,
                None,
            );
        }
        Err(err) => return citizen_internal_error_response(err),
    };

    if let Some(cached) = match load_cached_receipt_tx(&mut tx, &resolved_agent_id, &payload.client_event_id).await {
        Ok(value) => value,
        Err(err) => return citizen_internal_error_response(err),
    } {
        return cached_receipt_to_response(cached);
    }

    if locked_wake.agent_id != resolved_agent_id {
        return citizen_protocol_response(
            StatusCode::FORBIDDEN,
            "agent_mismatch",
            "The requested wake does not belong to the authenticated citizen.",
            false,
            false,
            None,
        );
    }

    let expected_wake_token = match derive_wake_token(
        &locked_wake.event_id,
        &locked_wake.agent_id,
        locked_wake.seq,
        locked_wake.wake_token_expires_at,
    ) {
        Ok(token) => token,
        Err(err) => return citizen_internal_error_response(err),
    };

    if wake_token != expected_wake_token {
        return citizen_protocol_response(
            StatusCode::FORBIDDEN,
            "invalid_wake_token",
            "The supplied wake token is not valid for this wake.",
            false,
            false,
            None,
        );
    }

    if locked_wake.status != "open" {
        return citizen_protocol_response(
            StatusCode::CONFLICT,
            "wake_closed",
            "That wake is no longer open.",
            true,
            true,
            None,
        );
    }

    if locked_wake.wake_token_expires_at <= Utc::now() {
        let promoted = match expire_locked_wake_and_promote(&mut tx, &resolved_agent_id, &locked_wake.event_id).await {
            Ok(promoted) => promoted,
            Err(err) => return citizen_internal_error_response(err),
        };

        if let Err(err) = tx.commit().await {
            return citizen_internal_error_response(err.into());
        }

        if promoted {
            let _ = state
                .citizen_signal_tx()
                .send(resolved_agent_id.clone());
        }

        return citizen_protocol_response(
            StatusCode::CONFLICT,
            "wake_closed",
            "That wake has expired.",
            true,
            true,
            None,
        );
    }

    let action = payload.action.trim();
    if action.is_empty()
        || payload.client_event_id.trim().is_empty()
        || payload.wake_event_id.trim().is_empty()
    {
        let world = match build_world_block_tx(&mut tx, &resolved_agent_id).await {
            Ok(world) => world,
            Err(err) => return citizen_internal_error_response(err),
        };
        let body = citizen_semantic_error_body(
            "invalid_args",
            "Action, client_event_id, and wake_event_id are required.",
            false,
            false,
            Some(world),
            None,
        );
        if let Err(err) = store_receipt_tx(
            &mut tx,
            &resolved_agent_id,
            &payload.client_event_id,
            &payload.wake_event_id,
            StatusCode::OK,
            &body,
        )
        .await
        {
            return citizen_internal_error_response(err);
        }
        if let Err(err) = tx.commit().await {
            return citizen_internal_error_response(err.into());
        }
        return json_value_response(StatusCode::OK, body);
    }

    let mut signal_agent = false;

    let body = match action {
        "set_activity" => {
            let activity = match extract_activity(&payload.args) {
                Ok(activity) => activity,
                Err(message) => {
                    let world = match build_world_block_tx(&mut tx, &resolved_agent_id).await {
                        Ok(world) => world,
                        Err(err) => return citizen_internal_error_response(err),
                    };
                    citizen_semantic_error_body(
                        "invalid_args",
                        &message,
                        false,
                        false,
                        Some(world),
                        None,
                    )
                }
            };

            if activity.is_string() {
                let activity = activity.as_str().unwrap_or_default();
                let updated_agent = match perform_agent_activity_update_in_tx(
                    &mut tx,
                    &resolved_agent_id,
                    activity,
                )
                .await
                {
                    Ok(agent) => agent,
                    Err(err) => return citizen_internal_error_response(err),
                };

                let tick = match latest_tick_tx(&mut tx).await {
                    Ok(tick) => tick,
                    Err(err) => return citizen_internal_error_response(err),
                };

                citizen_ok_body(
                    json!({
                        "message": format!("Visible activity set to '{}'.", activity),
                        "activity": activity,
                    }),
                    false,
                    false,
                    Some(json!({
                        "tick": tick,
                        "world_time": Utc::now().to_rfc3339(),
                        "location_id": updated_agent.current_location_id,
                        "agent_state": updated_agent.state,
                    })),
                )
            } else {
                activity
            }
        }
        "wake_done" => {
            let world = match build_world_block_tx(&mut tx, &resolved_agent_id).await {
                Ok(world) => world,
                Err(err) => return citizen_internal_error_response(err),
            };
            signal_agent = match close_wake_tx(&mut tx, &resolved_agent_id, &payload.wake_event_id, "done", None).await {
                Ok(promoted) => promoted,
                Err(err) => return citizen_internal_error_response(err),
            };
            citizen_ok_body(
                json!({ "message": "Wake completed." }),
                true,
                true,
                Some(world),
            )
        }
        "wake_abort" => {
            let reason = payload
                .args
                .get("reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("client_abort")
                .to_string();
            let world = match build_world_block_tx(&mut tx, &resolved_agent_id).await {
                Ok(world) => world,
                Err(err) => return citizen_internal_error_response(err),
            };
            signal_agent = match close_wake_tx(
                &mut tx,
                &resolved_agent_id,
                &payload.wake_event_id,
                "aborted",
                Some(reason.clone()),
            )
            .await
            {
                Ok(promoted) => promoted,
                Err(err) => return citizen_internal_error_response(err),
            };
            citizen_ok_body(
                json!({
                    "message": "Wake aborted.",
                    "reason": reason,
                }),
                true,
                true,
                Some(world),
            )
        }
        _ => {
            let world = match build_world_block_tx(&mut tx, &resolved_agent_id).await {
                Ok(world) => world,
                Err(err) => return citizen_internal_error_response(err),
            };
            citizen_semantic_error_body(
                "unknown_action",
                "That citizen action is not supported in v1.",
                false,
                false,
                Some(world),
                Some(json!({ "action": action })),
            )
        }
    };

    if let Err(err) = store_receipt_tx(
        &mut tx,
        &resolved_agent_id,
        &payload.client_event_id,
        &payload.wake_event_id,
        StatusCode::OK,
        &body,
    )
    .await
    {
        return citizen_internal_error_response(err);
    }

    if let Err(err) = tx.commit().await {
        return citizen_internal_error_response(err.into());
    }

    if signal_agent {
        let _ = state.citizen_signal_tx().send(resolved_agent_id);
    }

    json_value_response(StatusCode::OK, body)
}

pub async fn create_test_citizen_wake(
    State(state): State<AppState>,
    _sim_key: SimKey,
    Path(agent_id): Path<String>,
    payload: Result<Json<AdminTestWakeRequest>, JsonRejection>,
) -> AppResult<Json<ApiResponse<TestCitizenWakeResponse>>> {
    let payload = match payload {
        Ok(Json(payload)) => payload,
        Err(_) => AdminTestWakeRequest::default(),
    };

    let response = enqueue_test_wake(&state, &agent_id, payload).await?;
    if response.status == "open" {
        let _ = state.citizen_signal_tx().send(agent_id);
    }

    Ok(Json(ApiResponse::from(response)))
}

async fn handle_citizen_socket(state: AppState, agent_id: String, mut socket: WebSocket) {
    let mut signal_rx = state.citizen_signal_tx().subscribe();
    let mut last_sent_event_id: Option<String> = None;

    if deliver_open_wake(&state, &agent_id, &mut socket, &mut last_sent_event_id)
        .await
        .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            message = socket.recv() => {
                match message {
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
            signal = signal_rx.recv() => {
                match signal {
                    Ok(target_agent_id) if target_agent_id == agent_id => {
                        if deliver_open_wake(&state, &agent_id, &mut socket, &mut last_sent_event_id).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        if deliver_open_wake(&state, &agent_id, &mut socket, &mut last_sent_event_id).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

async fn deliver_open_wake(
    state: &AppState,
    agent_id: &str,
    socket: &mut WebSocket,
    last_sent_event_id: &mut Option<String>,
) -> AppResult<()> {
    expire_open_wake_if_needed(state, agent_id).await?;

    let wake = sqlx::query_as::<_, OpenWakeRow>(
        r#"
        SELECT event_id, seq, wake_type, world_time, wall_time, agent_snapshot, trigger_payload,
               prompt_narrative, prompt_structured, tools, wake_token_expires_at,
               expects_response, dropped_for_overflow_count
        FROM citizen_wakes
        WHERE agent_id = $1
          AND status = 'open'
        ORDER BY seq ASC
        LIMIT 1
        "#,
    )
    .bind(agent_id)
    .fetch_optional(state.pool())
    .await?;

    let Some(wake) = wake else {
        return Ok(());
    };

    if last_sent_event_id.as_deref() == Some(wake.event_id.as_str()) {
        return Ok(());
    }

    let wake_token = derive_wake_token(
        &wake.event_id,
        agent_id,
        wake.seq,
        wake.wake_token_expires_at,
    )?;

    let payload = json!({
        "event_id": wake.event_id,
        "seq": wake.seq,
        "type": wake.wake_type,
        "world_time": wake.world_time.to_rfc3339(),
        "wall_time": wake.wall_time.to_rfc3339(),
        "agent": wake.agent_snapshot,
        "trigger": wake.trigger_payload,
        "prompt": {
            "narrative": wake.prompt_narrative,
            "structured": wake.prompt_structured,
        },
        "tools": wake.tools,
        "wake_token": wake_token,
        "wake_token_expires_at": wake.wake_token_expires_at.to_rfc3339(),
        "expects_response": wake.expects_response,
        "meta": {
            "dropped_for_overflow_count": wake.dropped_for_overflow_count,
        }
    });

    socket
        .send(Message::Text(payload.to_string()))
        .await
        .map_err(|err| AppError::Unexpected(err.to_string()))?;

    *last_sent_event_id = payload["event_id"].as_str().map(str::to_string);
    Ok(())
}

async fn enqueue_test_wake(
    state: &AppState,
    agent_id: &str,
    payload: AdminTestWakeRequest,
) -> AppResult<TestCitizenWakeResponse> {
    let mut tx = state.pool().begin().await?;
    let prompt_narrative = payload
        .narrative
        .and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .unwrap_or_else(|| {
            "You have entered the city harness proof-of-life test. Set a short visible activity describing what you are doing.".to_string()
        });

    let enqueued = enqueue_citizen_wake_tx(
        &mut tx,
        agent_id,
        "system_notice",
        json!({
            "kind": "system",
            "ref": "admin_test",
            "details": {
                "source": "admin_test_endpoint"
            }
        }),
        prompt_narrative,
        payload.structured.unwrap_or_else(|| json!({})),
        set_activity_tools_json(),
        payload.expects_response.unwrap_or(true),
    )
    .await?;

    tx.commit().await?;

    Ok(TestCitizenWakeResponse {
        event_id: enqueued.event_id,
        agent_id: agent_id.to_string(),
        seq: enqueued.seq,
        status: enqueued.status,
        wake_token_expires_at: enqueued.wake_token_expires_at,
    })
}

pub async fn enqueue_citizen_wake_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
    wake_type: &str,
    trigger_payload: Value,
    prompt_narrative: String,
    prompt_structured: Value,
    tools: Value,
    expects_response: bool,
) -> AppResult<EnqueuedCitizenWake> {
    let snapshot = load_agent_wake_snapshot_tx(tx, agent_id).await?;
    ensure_runtime_state_tx(tx, agent_id).await?;

    let runtime = sqlx::query_as::<_, (i64, i32)>(
        r#"
        SELECT last_seq, pending_dropped_overflow_count
        FROM citizen_runtime_state
        WHERE agent_id = $1
        FOR UPDATE
        "#,
    )
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await?;

    let has_open = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM citizen_wakes
            WHERE agent_id = $1
              AND status = 'open'
        )
        "#,
    )
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await?;

    let seq = runtime.0 + 1;
    let status = if has_open { "queued" } else { "open" };
    let now = Utc::now();
    let wake_token_expires_at = now + Duration::seconds(WAKE_TOKEN_TTL_SECONDS);
    let event_id = format!("evt_{}", Uuid::new_v4().simple());
    let dropped_for_overflow_count = if status == "open" { runtime.1 } else { 0 };

    sqlx::query(
        r#"
        INSERT INTO citizen_wakes (
            event_id, agent_id, seq, wake_type, world_time, wall_time, agent_snapshot,
            trigger_payload, prompt_narrative, prompt_structured, tools,
            wake_token_expires_at, expects_response, dropped_for_overflow_count, status,
            opened_at
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7::jsonb,
            $8::jsonb, $9, $10::jsonb, $11::jsonb, $12,
            $13, $14, $15,
            $16
        )
        "#,
    )
    .bind(&event_id)
    .bind(agent_id)
    .bind(seq)
    .bind(wake_type)
    .bind(now)
    .bind(now)
    .bind(snapshot)
    .bind(trigger_payload)
    .bind(prompt_narrative)
    .bind(prompt_structured)
    .bind(tools)
    .bind(wake_token_expires_at)
    .bind(expects_response)
    .bind(dropped_for_overflow_count)
    .bind(status)
    .bind(if status == "open" { Some(now) } else { None })
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE citizen_runtime_state
        SET last_seq = $2,
            pending_dropped_overflow_count = CASE WHEN $3 = 'open' THEN 0 ELSE pending_dropped_overflow_count END,
            updated_at = NOW()
        WHERE agent_id = $1
        "#,
    )
    .bind(agent_id)
    .bind(seq)
    .bind(status)
    .execute(&mut **tx)
    .await?;

    if status == "queued" {
        enforce_queue_cap_tx(tx, agent_id).await?;
    }

    Ok(EnqueuedCitizenWake {
        event_id,
        seq,
        status: status.to_string(),
        wake_token_expires_at: wake_token_expires_at.to_rfc3339(),
        should_signal: status == "open",
    })
}

async fn ensure_runtime_state_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO citizen_runtime_state (agent_id)
        VALUES ($1)
        ON CONFLICT (agent_id) DO NOTHING
        "#,
    )
    .bind(agent_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn enforce_queue_cap_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
) -> AppResult<()> {
    let queued_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM citizen_wakes
        WHERE agent_id = $1
          AND status = 'queued'
        "#,
    )
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await?;

    if queued_count <= WAKE_QUEUE_CAP {
        return Ok(());
    }

    let overflow_count = queued_count - WAKE_QUEUE_CAP;
    let dropped_event_ids = sqlx::query_scalar::<_, String>(
        r#"
        SELECT event_id
        FROM citizen_wakes
        WHERE agent_id = $1
          AND status = 'queued'
        ORDER BY seq ASC
        LIMIT $2
        "#,
    )
    .bind(agent_id)
    .bind(overflow_count)
    .fetch_all(&mut **tx)
    .await?;

    for event_id in dropped_event_ids {
        sqlx::query(
            r#"
            UPDATE citizen_wakes
            SET status = 'dropped',
                closed_at = NOW()
            WHERE event_id = $1
            "#,
        )
        .bind(event_id)
        .execute(&mut **tx)
        .await?;
    }

    sqlx::query(
        r#"
        UPDATE citizen_runtime_state
        SET pending_dropped_overflow_count = pending_dropped_overflow_count + $2,
            updated_at = NOW()
        WHERE agent_id = $1
        "#,
    )
    .bind(agent_id)
    .bind(overflow_count as i32)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn load_agent_wake_snapshot_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
) -> AppResult<Value> {
    let snapshot = sqlx::query_as::<_, AgentWakeSnapshotRow>(
        r#"
        SELECT a.id, a.name, a.current_location_id, l.name AS location_name
        FROM agents a
        JOIN locations l ON l.id = a.current_location_id
        WHERE a.id = $1
        LIMIT 1
        "#,
    )
    .bind(agent_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(json!({
        "agent_id": snapshot.id,
        "citizen_id": snapshot.id,
        "display_name": snapshot.name,
        "location": {
            "id": snapshot.current_location_id,
            "type": Value::Null,
            "name": snapshot.location_name,
        }
    }))
}

async fn load_cached_receipt(
    pool: &sqlx::Pool<Postgres>,
    agent_id: &str,
    client_event_id: &str,
) -> AppResult<Option<CachedReceiptRow>> {
    sqlx::query_as::<_, CachedReceiptRow>(
        r#"
        SELECT response_status, response_body
        FROM citizen_action_receipts
        WHERE agent_id = $1
          AND client_event_id = $2
        LIMIT 1
        "#,
    )
    .bind(agent_id)
    .bind(client_event_id)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

async fn load_cached_receipt_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
    client_event_id: &str,
) -> AppResult<Option<CachedReceiptRow>> {
    sqlx::query_as::<_, CachedReceiptRow>(
        r#"
        SELECT response_status, response_body
        FROM citizen_action_receipts
        WHERE agent_id = $1
          AND client_event_id = $2
        LIMIT 1
        "#,
    )
    .bind(agent_id)
    .bind(client_event_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
}

async fn lock_wake_for_action(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
    wake_event_id: &str,
) -> AppResult<Option<LockedWakeRow>> {
    sqlx::query_as::<_, LockedWakeRow>(
        r#"
        SELECT event_id, agent_id, status, seq, wake_token_expires_at
        FROM citizen_wakes
        WHERE event_id = $1
          AND agent_id = $2
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(wake_event_id)
    .bind(agent_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
}

async fn expire_locked_wake_and_promote(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
    event_id: &str,
) -> AppResult<bool> {
    sqlx::query(
        r#"
        UPDATE citizen_wakes
        SET status = 'expired',
            closed_at = NOW()
        WHERE event_id = $1
        "#,
    )
    .bind(event_id)
    .execute(&mut **tx)
    .await?;

    promote_next_wake_tx(tx, agent_id).await
}

async fn close_wake_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
    event_id: &str,
    status: &str,
    abort_reason: Option<String>,
) -> AppResult<bool> {
    sqlx::query(
        r#"
        UPDATE citizen_wakes
        SET status = $2,
            abort_reason = $3,
            closed_at = NOW()
        WHERE event_id = $1
        "#,
    )
    .bind(event_id)
    .bind(status)
    .bind(abort_reason)
    .execute(&mut **tx)
    .await?;

    promote_next_wake_tx(tx, agent_id).await
}

async fn promote_next_wake_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
) -> AppResult<bool> {
    ensure_runtime_state_tx(tx, agent_id).await?;

    let pending_drops = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT pending_dropped_overflow_count
        FROM citizen_runtime_state
        WHERE agent_id = $1
        FOR UPDATE
        "#,
    )
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await?;

    let next_event_id = sqlx::query_scalar::<_, String>(
        r#"
        SELECT event_id
        FROM citizen_wakes
        WHERE agent_id = $1
          AND status = 'queued'
        ORDER BY seq ASC
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(agent_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(next_event_id) = next_event_id else {
        return Ok(false);
    };

    sqlx::query(
        r#"
        UPDATE citizen_wakes
        SET status = 'open',
            opened_at = NOW(),
            dropped_for_overflow_count = $2
        WHERE event_id = $1
        "#,
    )
    .bind(&next_event_id)
    .bind(pending_drops)
    .execute(&mut **tx)
    .await?;

    if pending_drops > 0 {
        sqlx::query(
            r#"
            UPDATE citizen_runtime_state
            SET pending_dropped_overflow_count = 0,
                updated_at = NOW()
            WHERE agent_id = $1
            "#,
        )
        .bind(agent_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(true)
}

async fn expire_open_wake_if_needed(state: &AppState, agent_id: &str) -> AppResult<()> {
    let mut tx = state.pool().begin().await?;

    let expired_event_id = sqlx::query_scalar::<_, String>(
        r#"
        SELECT event_id
        FROM citizen_wakes
        WHERE agent_id = $1
          AND status = 'open'
          AND wake_token_expires_at <= NOW()
        ORDER BY seq ASC
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(agent_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(expired_event_id) = expired_event_id else {
        tx.rollback().await?;
        return Ok(());
    };

    sqlx::query(
        r#"
        UPDATE citizen_wakes
        SET status = 'expired',
            closed_at = NOW()
        WHERE event_id = $1
        "#,
    )
    .bind(expired_event_id)
    .execute(&mut *tx)
    .await?;

    let promoted = promote_next_wake_tx(&mut tx, agent_id).await?;
    tx.commit().await?;

    if promoted {
        let _ = state.citizen_signal_tx().send(agent_id.to_string());
    }

    Ok(())
}

async fn build_world_block_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
) -> AppResult<Value> {
    let row = sqlx::query(
        r#"
        SELECT current_location_id, state
        FROM agents
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(agent_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let tick = latest_tick_tx(tx).await?;

    Ok(json!({
        "tick": tick,
        "world_time": Utc::now().to_rfc3339(),
        "location_id": row.get::<String, _>("current_location_id"),
        "agent_state": row.get::<String, _>("state"),
    }))
}

async fn latest_tick_tx(tx: &mut Transaction<'_, Postgres>) -> AppResult<i64> {
    let tick = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(MAX(id), 0)
        FROM events
        "#,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(tick)
}

async fn store_receipt_tx(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: &str,
    client_event_id: &str,
    wake_event_id: &str,
    status: StatusCode,
    body: &Value,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO citizen_action_receipts (
            agent_id, client_event_id, wake_event_id, response_status, response_body
        )
        VALUES ($1, $2, $3, $4, $5::jsonb)
        ON CONFLICT (agent_id, client_event_id) DO NOTHING
        "#,
    )
    .bind(agent_id)
    .bind(client_event_id)
    .bind(wake_event_id)
    .bind(i32::from(status.as_u16()))
    .bind(body)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn set_activity_tools_json() -> Value {
    json!([
        {
            "name": "set_activity",
            "description": "Set your current visible activity in the city. Use a short public status like 'reading in the park' or 'waiting at Hobbs Cafe'. This does not move you and does not speak aloud.",
            "parameters": {
                "type": "object",
                "properties": {
                    "activity": {
                        "type": "string",
                        "description": "Short visible activity text for other observers in the city.",
                        "minLength": 1,
                        "maxLength": 120
                    }
                },
                "required": ["activity"]
            }
        }
    ])
}

fn derive_wake_token(
    event_id: &str,
    agent_id: &str,
    seq: i64,
    wake_token_expires_at: DateTime<Utc>,
) -> AppResult<String> {
    let secret = std::env::var("SIM_API_KEY")?;
    let material = format!(
        "{event_id}:{agent_id}:{seq}:{}:{secret}",
        wake_token_expires_at.timestamp()
    );

    let mut hasher = Sha256::new();
    hasher.update(material.as_bytes());
    let digest = hasher.finalize();
    let token: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();

    Ok(format!("wk_{token}"))
}

fn extract_activity(args: &Value) -> Result<Value, String> {
    let activity = args
        .get("activity")
        .and_then(Value::as_str)
        .map(str::trim)
        .ok_or_else(|| "Activity is required and must be a string.".to_string())?;

    if activity.is_empty() {
        return Err("Activity cannot be empty.".to_string());
    }

    if activity.chars().count() > 120 {
        return Err("Activity must be 120 characters or fewer.".to_string());
    }

    Ok(Value::String(activity.to_string()))
}

fn required_header(headers: &HeaderMap, name: &str) -> Result<String, String> {
    let value = headers
        .get(name)
        .ok_or_else(|| format!("missing {name} header"))?
        .to_str()
        .map_err(|_| format!("invalid {name} header"))?
        .trim()
        .to_string();

    if value.is_empty() {
        Err(format!("{name} header cannot be empty"))
    } else {
        Ok(value)
    }
}

fn citizen_ok_body(result: Value, ends_turn: bool, wake_closed: bool, world: Option<Value>) -> Value {
    let mut body = json!({
        "ok": true,
        "result": result,
        "control": {
            "ends_turn": ends_turn,
            "wake_closed": wake_closed,
        }
    });

    if let Some(world) = world {
        body["world"] = world;
    }

    body
}

fn citizen_semantic_error_body(
    code: &str,
    message: &str,
    ends_turn: bool,
    wake_closed: bool,
    world: Option<Value>,
    details: Option<Value>,
) -> Value {
    let mut body = json!({
        "ok": false,
        "error": {
            "code": code,
            "message": message,
            "details": details.unwrap_or_else(|| json!({})),
        },
        "control": {
            "ends_turn": ends_turn,
            "wake_closed": wake_closed,
        }
    });

    if let Some(world) = world {
        body["world"] = world;
    }

    body
}

fn citizen_protocol_response(
    status: StatusCode,
    code: &str,
    message: &str,
    ends_turn: bool,
    wake_closed: bool,
    world: Option<Value>,
) -> Response {
    let body = citizen_semantic_error_body(code, message, ends_turn, wake_closed, world, None);
    json_value_response(status, body)
}

fn cached_receipt_to_response(cached: CachedReceiptRow) -> Response {
    let status = StatusCode::from_u16(cached.response_status as u16)
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    json_value_response(status, cached.response_body)
}

fn json_value_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn citizen_internal_error_response(err: AppError) -> Response {
    citizen_protocol_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        &err.to_string(),
        false,
        false,
        None,
    )
}

fn default_args() -> Value {
    json!({})
}
