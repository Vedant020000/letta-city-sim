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
    pub balance_cents: i64,
    pub last_income_cents: Option<i64>,
    pub last_income_reason: Option<String>,
    pub last_income_at: Option<DateTime<Utc>>,
    pub last_expense_cents: Option<i64>,
    pub last_expense_reason: Option<String>,
    pub last_expense_at: Option<DateTime<Utc>>,
    pub food_level: i16,
    pub water_level: i16,
    pub stamina_level: i16,
    pub sleep_level: i16,
    pub last_vitals_update: DateTime<Utc>,
}
