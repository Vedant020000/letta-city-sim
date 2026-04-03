use axum::{
    Json,
    extract::{Path, State},
};

use crate::error::AppError;
use crate::error::AppResult;
use crate::models::location::{AdjacentLocation, Location, LocationDetailResponse};
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

pub async fn get_location_by_id(
    State(state): State<AppState>,
    Path(location_id): Path<String>,
) -> AppResult<Json<LocationDetailResponse>> {
    let location = sqlx::query_as::<_, Location>(
        r#"
        SELECT id, name, description, map_x, map_y
        FROM locations
        WHERE id = $1
        "#,
    )
    .bind(&location_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    let nearby = sqlx::query_as::<_, AdjacentLocation>(
        r#"
        SELECT l.id, l.name, l.description, l.map_x, l.map_y, la.travel_secs
        FROM location_adjacency la
        JOIN locations l ON l.id = la.to_id
        WHERE la.from_id = $1
        ORDER BY la.travel_secs ASC, l.name ASC
        "#,
    )
    .bind(&location_id)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(LocationDetailResponse { location, nearby }))
}

pub async fn get_nearby_locations(
    State(state): State<AppState>,
    Path(location_id): Path<String>,
) -> AppResult<Json<Vec<AdjacentLocation>>> {
    let exists = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM locations
        WHERE id = $1
        "#,
    )
    .bind(&location_id)
    .fetch_optional(state.pool())
    .await?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    let nearby = sqlx::query_as::<_, AdjacentLocation>(
        r#"
        SELECT l.id, l.name, l.description, l.map_x, l.map_y, la.travel_secs
        FROM location_adjacency la
        JOIN locations l ON l.id = la.to_id
        WHERE la.from_id = $1
        ORDER BY la.travel_secs ASC, l.name ASC
        "#,
    )
    .bind(&location_id)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(nearby))
}
