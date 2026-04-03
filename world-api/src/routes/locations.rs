use axum::{Json, extract::State};

use crate::error::AppResult;
use crate::models::location::Location;
use crate::state::AppState;

pub async fn list_locations(State(state): State<AppState>) -> AppResult<Json<Vec<Location>>> {
    let locations = sqlx::query_as::<_, Location>(
        r#"
        SELECT id, name, description, map_x, map_y
        FROM locations
        ORDER BY name
        "#,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(locations))
}
