use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use serde::Deserialize;

use crate::auth::AuthContext;
use crate::error::{AppError, AppResult};
use crate::models::agent::Agent;
use crate::models::common::{ApiResponse, NotificationMode, NotificationPayload};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct EconomyUpdateRequest {
    pub amount_cents: i64,
    pub reason: Option<String>,
}

pub async fn update_economy(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(agent_id): Path<String>,
    Json(payload): Json<EconomyUpdateRequest>,
) -> AppResult<Json<ApiResponse<Agent>>> {
    auth.ensure_agent(&agent_id)?;

    if payload.amount_cents == 0 {
        return Err(AppError::BadRequest("amount cannot be zero".to_string()));
    }

    let mut tx = state.pool().begin().await?;

    let agent = sqlx::query_as::<_, Agent>(
        r#"
        UPDATE agents
        SET balance_cents = balance_cents + $1,
            last_income_cents = CASE WHEN $1 > 0 THEN $1 ELSE last_income_cents END,
            last_income_reason = CASE WHEN $1 > 0 THEN $2 ELSE last_income_reason END,
            last_income_at = CASE WHEN $1 > 0 THEN NOW() ELSE last_income_at END,
            last_expense_cents = CASE WHEN $1 < 0 THEN ABS($1) ELSE last_expense_cents END,
            last_expense_reason = CASE WHEN $1 < 0 THEN $2 ELSE last_expense_reason END,
            last_expense_at = CASE WHEN $1 < 0 THEN NOW() ELSE last_expense_at END,
            updated_at = NOW()
        WHERE id = $3
        RETURNING *
        "#,
    )
    .bind(payload.amount_cents)
    .bind(payload.reason.clone())
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind(if payload.amount_cents > 0 { "economy.credit" } else { "economy.debit" })
    .bind(&agent_id)
    .bind(&agent.current_location_id)
    .bind("Agent balance updated")
    .bind(
        serde_json::json!({
            "delta_cents": payload.amount_cents,
            "reason": payload.reason,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let notification = NotificationPayload {
        message: format!(
            "Balance updated by {} cents (new balance: {})",
            payload.amount_cents,
            agent.balance_cents
        ),
        mode: NotificationMode::Instant,
        eta_seconds: None,
    };

    Ok(Json(ApiResponse {
        data: agent,
        notification: Some(notification),
    }))
}
