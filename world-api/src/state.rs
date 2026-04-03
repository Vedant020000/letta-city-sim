use sqlx::postgres::PgPoolOptions;

use crate::error::AppResult;

#[derive(Clone)]
pub struct AppState {
    pool: sqlx::Pool<sqlx::Postgres>,
}

impl AppState {
    pub async fn new(database_url: &str, max_connections: u32) -> AppResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &sqlx::Pool<sqlx::Postgres> {
        &self.pool
    }
}
