use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct Location {
    pub id: String,
    pub name: String,
    pub description: String,
    pub map_x: i32,
    pub map_y: i32,
    pub kind: String,
    pub capacity: Option<i32>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct LocationRole {
    pub location_id: String,
    pub agent_id: String,
    pub role: String,
    pub agent_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct AdjacentLocation {
    pub id: String,
    pub name: String,
    pub description: String,
    pub map_x: i32,
    pub map_y: i32,
    pub travel_secs: i32,
}

#[derive(Debug, Serialize)]
pub struct LocationDetailResponse {
    pub location: Location,
    pub nearby: Vec<AdjacentLocation>,
    pub roles: Vec<LocationRole>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct AgentLocationRole {
    pub location_id: String,
    pub location_name: String,
    pub location_kind: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AgentLocationsResponse {
    pub agent_id: String,
    pub home_location_id: Option<String>,
    pub roles: Vec<AgentLocationRole>,
}
