use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct AgentIntention {
    pub id: String,
    pub agent_id: String,
    pub summary: String,
    pub reason: String,
    pub status: String,
    pub expected_location_id: Option<String>,
    pub expected_action: Option<String>,
    pub outcome: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAgentIntentionRequest {
    pub summary: String,
    pub reason: String,
    pub expected_location_id: Option<String>,
    pub expected_action: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAgentIntentionRequest {
    pub summary: Option<String>,
    pub reason: Option<String>,
    pub status: Option<String>,
    pub expected_location_id: Option<String>,
    pub expected_action: Option<String>,
    pub outcome: Option<String>,
    pub metadata: Option<serde_json::Value>,
}
