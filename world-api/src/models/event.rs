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
}

#[derive(Debug, Deserialize)]
pub struct CreateEventRequest {
    pub r#type: String,
    pub actor_id: Option<String>,
    pub location_id: Option<String>,
    pub description: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    pub since: Option<DateTime<Utc>>,
    pub location_id: Option<String>,
    pub actor_id: Option<String>,
    pub r#type: Option<String>,
    pub limit: Option<i64>,
}
