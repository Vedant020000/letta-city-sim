use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub summary: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct AssignedJob {
    pub job_id: String,
    pub name: String,
    pub kind: String,
    pub summary: String,
    pub job_metadata: serde_json::Value,
    pub is_primary: bool,
    pub notes: Option<String>,
    pub assignment_metadata: serde_json::Value,
    pub assigned_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct JobAgent {
    pub agent_id: String,
    pub agent_name: String,
    pub occupation: String,
    pub current_location_id: String,
    pub state: String,
    pub is_primary: bool,
    pub notes: Option<String>,
    pub assignment_metadata: serde_json::Value,
    pub assigned_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertAgentJobRequest {
    pub is_primary: Option<bool>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}
