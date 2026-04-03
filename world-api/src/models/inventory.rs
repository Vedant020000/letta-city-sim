use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct InventoryItem {
    pub id: String,
    pub name: String,
    pub held_by: Option<String>,
    pub location_id: Option<String>,
    pub state: serde_json::Value,
}
