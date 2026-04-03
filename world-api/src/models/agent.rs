use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub occupation: String,
    pub current_location_id: String,
    pub state: String,
    pub current_activity: Option<String>,
    pub is_npc: bool,
    pub is_active: bool,
    pub state_updated_at: DateTime<Utc>,
}
