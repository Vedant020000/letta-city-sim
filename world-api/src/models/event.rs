use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct SimEvent {
    pub id: i64,
    pub occurred_at: DateTime<Utc>,
    pub r#type: String,
    pub actor_id: Option<String>,
    pub location_id: Option<String>,
    pub description: String,
    pub metadata: serde_json::Value,
    pub importance: i16,
    pub visibility: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateEventRequest {
    pub r#type: String,
    pub actor_id: Option<String>,
    pub location_id: Option<String>,
    pub description: String,
    pub metadata: Option<serde_json::Value>,
    #[serde(default = "default_importance")]
    pub importance: i16,
    #[serde(default = "default_visibility")]
    pub visibility: String,
}

fn default_importance() -> i16 { 2 }
fn default_visibility() -> String { "location".to_string() }

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    pub since: Option<DateTime<Utc>>,
    pub location_id: Option<String>,
    pub actor_id: Option<String>,
    pub r#type: Option<String>,
    pub limit: Option<i64>,
}

/// Input for the event router — what to route and to whom.
#[derive(Debug, Clone)]
pub struct RouteEventInput {
    pub event_type: String,
    pub actor_id: Option<String>,
    pub location_id: Option<String>,
    pub importance: i16,
    pub visibility: String,
    pub description: String,
    pub metadata: serde_json::Value,
    /// Agents that must always be woken (direct targets).
    pub target_agent_ids: Vec<String>,
}

/// Result of routing an event — which agents were woken and why.
#[derive(Debug, Clone)]
pub struct RouteEventResult {
    pub woken_agents: Vec<WokenAgent>,
}

#[derive(Debug, Clone)]
pub struct WokenAgent {
    pub agent_id: String,
    pub rule: String,
}
