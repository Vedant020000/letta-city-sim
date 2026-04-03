use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct PathfindResponse {
    pub path: Vec<String>,
    pub travel_time_seconds: i32,
}
