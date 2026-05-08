use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct ConversationMessage {
    pub id: String,
    pub conversation_id: String,
    pub agent_id: String,
    pub sender_name: String,
    pub content: String,
    pub sent_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ConversationParticipant {
    pub agent_id: String,
    pub agent_name: String,
    pub status: String,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ConversationSummary {
    pub id: String,
    pub location_id: String,
    pub topic: Option<String>,
    pub participant_count: i64,
    pub active_participant_count: i64,
    pub last_message_preview: Option<String>,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ConversationDetail {
    pub id: String,
    pub location_id: String,
    pub topic: Option<String>,
    pub started_at: DateTime<Utc>,
    pub participants: Vec<ConversationParticipant>,
    pub messages: Vec<ConversationMessage>,
}
