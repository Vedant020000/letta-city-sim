use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct Location {
    pub id: String,
    pub name: String,
    pub description: String,
    pub map_x: i32,
    pub map_y: i32,
}
