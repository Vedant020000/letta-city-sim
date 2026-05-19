use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct AgentApplication {
    pub id: String,
    pub requested_agent_id: Option<String>,
    pub requested_name: String,
    pub occupation: String,
    pub statement: String,
    pub agent_description: Option<String>,
    pub callback_url: Option<String>,
    pub external_agent_ref: Option<String>,
    pub status: String,
    pub review_note: Option<String>,
    pub approved_agent_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateApplicationRequest {
    pub requested_agent_id: Option<String>,
    pub requested_name: String,
    pub occupation: String,
    pub statement: String,
    pub agent_description: Option<String>,
    pub callback_url: Option<String>,
    pub external_agent_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewApplicationRequest {
    pub review_note: Option<String>,
}
