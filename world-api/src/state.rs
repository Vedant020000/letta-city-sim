use sqlx::postgres::PgPoolOptions;
use tokio::sync::broadcast;

use crate::error::AppResult;

#[derive(Clone)]
pub struct AppState {
    pool: sqlx::Pool<sqlx::Postgres>,
    event_tx: broadcast::Sender<crate::ws_events::WorldEventEnvelope>,
    citizen_signal_tx: broadcast::Sender<String>,
}

impl AppState {
    pub async fn new(database_url: &str, max_connections: u32) -> AppResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await?;

        // Size is small; if consumers lag they can drop messages.
        let (event_tx, _event_rx) = broadcast::channel(256);
        let (citizen_signal_tx, _citizen_signal_rx) = broadcast::channel(256);

        Ok(Self {
            pool,
            event_tx,
            citizen_signal_tx,
        })
    }

    pub fn pool(&self) -> &sqlx::Pool<sqlx::Postgres> {
        &self.pool
    }

    pub fn event_tx(&self) -> &broadcast::Sender<crate::ws_events::WorldEventEnvelope> {
        &self.event_tx
    }

    pub fn citizen_signal_tx(&self) -> &broadcast::Sender<String> {
        &self.citizen_signal_tx
    }
}
