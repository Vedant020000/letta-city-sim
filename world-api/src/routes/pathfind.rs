use std::collections::HashMap;

use axum::{
    Json,
    extract::{Query, State},
};
use pathfinding::prelude::dijkstra;
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::models::pathfind::PathfindResponse;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct PathfindQuery {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone)]
struct Edge {
    to_id: String,
    travel_secs: i32,
}

pub async fn get_path(
    Query(query): Query<PathfindQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<PathfindResponse>> {
    if query.from == query.to {
        return Ok(Json(PathfindResponse {
            path: vec![query.from],
            travel_time_seconds: 0,
        }));
    }

    let edges = sqlx::query_as::<_, (String, String, i32)>(
        r#"
        SELECT from_id, to_id, travel_secs
        FROM location_adjacency
        "#,
    )
    .fetch_all(state.pool())
    .await?;

    let mut graph: HashMap<String, Vec<Edge>> = HashMap::new();
    for (from_id, to_id, travel_secs) in edges {
        graph
            .entry(from_id)
            .or_default()
            .push(Edge { to_id, travel_secs });
    }

    let start = query.from.clone();
    let goal = query.to.clone();

    let result = dijkstra(
        &start,
        |node| {
            graph
                .get(node)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|edge| (edge.to_id, edge.travel_secs))
                .collect::<Vec<(String, i32)>>()
        },
        |node| node == &goal,
    )
    .ok_or(AppError::NotFound)?;

    let (path, total_cost) = result;

    if path.is_empty() {
        return Err(AppError::BadRequest("No route found".to_string()));
    }

    Ok(Json(PathfindResponse {
        path,
        travel_time_seconds: total_cost,
    }))
}
