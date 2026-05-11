use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct WorldTimeResponse {
    pub timestamp: String,
    pub hour: u32,
    pub time_of_day: String,
    pub day_number: i64,
    pub shops_open: bool,
    pub simulation_paused: bool,
}
