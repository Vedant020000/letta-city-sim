use std::net::SocketAddr;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{
    Router,
    routing::{delete, get, patch},
};
use dotenvy::dotenv;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

mod error;
mod models;
mod routes;
mod state;

use error::AppResult;
use routes::agents::{
    clear_agent_activity, get_agent_by_id, list_agents, update_agent_activity,
    update_agent_location,
};
use routes::locations::list_locations;
use state::AppState;

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
        .route("/locations", get(list_locations))
        .route("/agents", get(list_agents))
        .route("/agents/:id", get(get_agent_by_id))
        .route("/agents/:id/location", patch(update_agent_location))
        .route("/agents/:id/activity", patch(update_agent_activity))
        .route("/agents/:id/activity", delete(clear_agent_activity))
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
