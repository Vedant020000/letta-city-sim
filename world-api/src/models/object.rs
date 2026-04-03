use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct WorldObject {
    pub id: String,
    pub name: String,
    pub location_id: Option<String>,
    pub state: serde_json::Value,
    pub actions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorldObjectRequest {
    pub state: serde_json::Value,
}
