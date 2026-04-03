use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardPost {
    pub id: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct PublicBoardState {
    pub location_id: String,
    pub posts: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BoardStateWithIds {
    pub location_id: String,
    pub posts: Vec<BoardPost>,
}
