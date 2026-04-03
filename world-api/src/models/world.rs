use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct WorldTimeResponse {
    pub timestamp: String,
    pub time_of_day: String,
    pub simulation_paused: bool,
}
