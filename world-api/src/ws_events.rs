use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEventEnvelope {
    pub id: String,
    pub ts: DateTime<Utc>,
    #[serde(rename = "type")]
    pub event_type: String,
    pub agent_targets: Vec<String>,
    pub location_id: Option<String>,
    pub payload: Value,
}

impl WorldEventEnvelope {
    pub fn new(
        event_type: impl Into<String>,
        agent_targets: Vec<String>,
        location_id: Option<String>,
        payload: Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            ts: Utc::now(),
            event_type: event_type.into(),
            agent_targets,
            location_id,
            payload,
        }
    }
}

pub async fn ws_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    if let Err(e) = authorize_ws(&headers) {
        return (StatusCode::UNAUTHORIZED, e.to_string()).into_response();
    }

    ws.on_upgrade(move |socket| handle_socket(state, socket))
}

fn authorize_ws(headers: &HeaderMap) -> AppResult<()> {
    let expected = std::env::var("SIM_API_KEY")?;
    let provided = headers
        .get("x-sim-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .unwrap_or("");

    if provided.is_empty() || provided != expected {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

async fn handle_socket(state: AppState, mut socket: WebSocket) {
    let mut rx: broadcast::Receiver<WorldEventEnvelope> = state.event_tx().subscribe();

    loop {
        match rx.recv().await {
            Ok(evt) => {
                if let Ok(text) = serde_json::to_string(&evt) {
                    if socket.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
            }
            Err(broadcast::error::RecvError::Closed) => break,
            Err(broadcast::error::RecvError::Lagged(_)) => {
                // drop lagged messages
                continue;
            }
        }
    }
}
