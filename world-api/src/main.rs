use std::net::SocketAddr;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{
    Router,
    routing::{delete, get, patch, post},
};
use dotenvy::dotenv;
use tower_http::cors::{Any, CorsLayer};
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

mod auth;
mod error;
mod models;
mod routes;
mod state;
mod ws_events;

use auth::require_sim_key;
use error::AppResult;
use routes::agents::{
    agent_health_check, clear_agent_activity, get_agent_by_id, list_agents, move_agent_with_header,
    update_agent_activity, update_agent_location,
};
use routes::board::{
    clear_board, create_board_post, delete_board_post, get_board_posts, get_public_board,
};
use routes::economy::update_economy;
use routes::events::{create_event, list_events};
use routes::intentions::{
    create_agent_intention, get_current_agent_intention, list_agent_intentions,
    list_current_intentions, update_agent_intention,
};
use routes::inventory::transfer_item_between_agents;
use routes::inventory::{
    add_item_to_agent_inventory, get_agent_inventory, remove_item_from_agent_inventory, use_item,
};
use routes::jobs::{
    get_job_by_id, list_agent_jobs, list_job_agents, list_jobs, remove_agent_job, upsert_agent_job,
};
use routes::locations::{get_location_by_id, get_nearby_locations, list_locations};
use routes::objects::{list_objects_by_location, update_object_state};
use routes::pathfind::get_path;
use routes::pulse::get_town_pulse;
use routes::sleep::{start_sleep, wake_up};
use routes::world::get_world_time;
use state::AppState;
use ws_events::ws_events;

#[tokio::main]
async fn main() -> AppResult<()> {
    setup_tracing();
    dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")?;
    let max_connections: u32 = std::env::var("DB_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3001);

    let state = AppState::new(&database_url, max_connections).await?;

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/board", get(get_public_board))
        .route("/board/posts", get(get_board_posts))
        .route("/board/posts", patch(create_board_post))
        .route("/board/posts/:post_id", delete(delete_board_post))
        .route("/board/clear", delete(clear_board))
        .route("/events", get(list_events))
        .route("/events", post(create_event))
        .route("/intentions/current", get(list_current_intentions))
        .route("/ws/events", get(ws_events))
        .route("/locations", get(list_locations))
        .route("/world/time", get(get_world_time))
        .route("/locations/:id", get(get_location_by_id))
        .route("/locations/:id/nearby", get(get_nearby_locations))
        .route(
            "/locations/:location_id/objects",
            get(list_objects_by_location),
        )
        .route("/objects/:id", patch(update_object_state))
        .route("/pathfind", get(get_path))
        .route("/town/pulse", get(get_town_pulse))
        .route("/agents", get(list_agents))
        .route("/agents/health", get(agent_health_check))
        .route("/agents/move", patch(move_agent_with_header))
        .route("/jobs", get(list_jobs))
        .route("/jobs/:id", get(get_job_by_id))
        .route("/jobs/:id/agents", get(list_job_agents))
        .route("/agents/:id", get(get_agent_by_id))
        .route("/agents/:id/intentions", get(list_agent_intentions))
        .route("/agents/:id/intentions", post(create_agent_intention))
        .route("/agents/:id/jobs", get(list_agent_jobs))
        .route("/agents/:id/jobs/:job_id", patch(upsert_agent_job))
        .route("/agents/:id/jobs/:job_id", delete(remove_agent_job))
        .route(
            "/agents/:id/intentions/current",
            get(get_current_agent_intention),
        )
        .route(
            "/agents/:id/intentions/:intention_id",
            patch(update_agent_intention),
        )
        .route("/agents/:id/location", patch(update_agent_location))
        .route("/agents/:id/activity", patch(update_agent_activity))
        .route("/agents/:id/activity", delete(clear_agent_activity))
        .route("/agents/sleep", post(start_sleep))
        .route("/agents/sleep", delete(wake_up))
        .route("/agents/use-item", post(use_item))
        .route("/agents/:id/economy", patch(update_economy))
        .route("/inventory/:id", get(get_agent_inventory))
        .route("/inventory/:id/add", patch(add_item_to_agent_inventory))
        .route(
            "/inventory/:id/remove",
            patch(remove_item_from_agent_inventory),
        )
        .route(
            "/agents/:id/inventory/transfer",
            patch(transfer_item_between_agents),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(axum::middleware::from_fn(require_sim_key))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Starting World API on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

fn setup_tracing() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
}
