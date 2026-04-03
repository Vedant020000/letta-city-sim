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
}
