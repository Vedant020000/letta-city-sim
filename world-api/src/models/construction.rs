use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct ConstructionCompany {
    pub id: String,
    pub name: String,
    pub progress_per_sim_hour: i32,
    pub hiring_fee_cents: i64,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ConstructionProject {
    pub id: String,
    pub agent_id: String,
    pub location_name: String,
    pub status: String,
    pub cost_cents: i64,
    pub funded_cents: i64,
    pub progress: i32,
    pub company_id: Option<String>,
    pub last_progress_tick: Option<DateTime<Utc>>,
    pub location_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct StartProjectRequest {
    pub location_name: String,
}

#[derive(Debug, Deserialize)]
pub struct FundProjectRequest {
    pub amount_cents: i64,
}

#[derive(Debug, Deserialize)]
pub struct HireBuilderRequest {
    pub company_id: String,
}
