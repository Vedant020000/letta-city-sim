use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

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

// ---------------------------------------------------------------------------
// Agent-to-agent money transfer
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PayAgentRequest {
    pub to_agent_id: String,
    pub amount_cents: i64,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PayAgentResponse {
    pub transaction_id: i64,
    pub from_agent_id: String,
    pub to_agent_id: String,
    pub amount_cents: i64,
    pub from_new_balance: i64,
    pub to_new_balance: i64,
}

pub async fn pay_agent(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(from_agent_id): Path<String>,
    Json(payload): Json<PayAgentRequest>,
) -> AppResult<Json<ApiResponse<PayAgentResponse>>> {
    auth.ensure_agent(&from_agent_id)?;

    if from_agent_id == payload.to_agent_id {
        return Err(AppError::BadRequest(
            "cannot pay yourself".to_string(),
        ));
    }

    if payload.amount_cents <= 0 {
        return Err(AppError::BadRequest(
            "amount must be positive".to_string(),
        ));
    }

    let mut tx = state.pool().begin().await?;

    // Lock sender and get location
    let (from_balance, from_location) = sqlx::query_as::<_, (i64, String)>(
        r#"
        SELECT balance_cents, current_location_id
        FROM agents
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(&from_agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    // Check sufficient balance
    if from_balance < payload.amount_cents {
        return Err(AppError::BadRequest(
            format!("insufficient balance (have {}, need {})", from_balance, payload.amount_cents),
        ));
    }

    // Lock receiver and get location
    let (_to_balance, to_location) = sqlx::query_as::<_, (i64, String)>(
        r#"
        SELECT balance_cents, current_location_id
        FROM agents
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(&payload.to_agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    // Check adjacency (same location or adjacent)
    if from_location != to_location {
        let is_adjacent = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM location_adjacency
                WHERE (from_id = $1 AND to_id = $2)
                   OR (from_id = $2 AND to_id = $1)
            )
            "#,
        )
        .bind(&from_location)
        .bind(&to_location)
        .fetch_one(&mut *tx)
        .await?;

        if !is_adjacent {
            return Err(AppError::BadRequest(
                "agents must be at the same or adjacent locations to transfer money".to_string(),
            ));
        }
    }

    // Debit sender (with balance guard to prevent overdraft)
    let from_new = sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE agents
        SET balance_cents = balance_cents - $1,
            last_expense_cents = $1,
            last_expense_reason = $2,
            last_expense_at = NOW(),
            updated_at = NOW()
        WHERE id = $3 AND balance_cents >= $1
        RETURNING balance_cents
        "#,
    )
    .bind(payload.amount_cents)
    .bind(payload.reason.clone().unwrap_or_else(|| "Payment".to_string()))
    .bind(&from_agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::BadRequest(
        format!("insufficient balance (need {} cents)", payload.amount_cents)
    ))?;

    // Credit receiver
    let to_new = sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE agents
        SET balance_cents = balance_cents + $1,
            last_income_cents = $1,
            last_income_reason = $2,
            last_income_at = NOW(),
            updated_at = NOW()
        WHERE id = $3
        RETURNING balance_cents
        "#,
    )
    .bind(payload.amount_cents)
    .bind(payload.reason.clone().unwrap_or_else(|| "Payment received".to_string()))
    .bind(&payload.to_agent_id)
    .fetch_one(&mut *tx)
    .await?;

    // Insert transaction record
    let transaction_id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO economy_transactions (from_agent_id, to_agent_id, amount_cents, reason, transaction_type, status, location_id)
        VALUES ($1, $2, $3, $4, 'payment', 'completed', $5)
        RETURNING id
        "#,
    )
    .bind(&from_agent_id)
    .bind(&payload.to_agent_id)
    .bind(payload.amount_cents)
    .bind(&payload.reason)
    .bind(&from_location)
    .fetch_one(&mut *tx)
    .await?;

    // Log event
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("economy.payment")
    .bind(&from_agent_id)
    .bind(&from_location)
    .bind(format!(
        "Agent {} paid {} cents to agent {}",
        from_agent_id, payload.amount_cents, payload.to_agent_id
    ))
    .bind(
        serde_json::json!({
            "to_agent_id": payload.to_agent_id,
            "amount_cents": payload.amount_cents,
            "reason": payload.reason,
            "transaction_id": transaction_id,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(PayAgentResponse {
        transaction_id,
        from_agent_id,
        to_agent_id: payload.to_agent_id,
        amount_cents: payload.amount_cents,
        from_new_balance: from_new,
        to_new_balance: to_new,
    })))
}

// ---------------------------------------------------------------------------
// Request money
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RequestMoneyRequest {
    pub from_agent_id: String,
    pub amount_cents: i64,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RequestMoneyResponse {
    pub transaction_id: i64,
    pub from_agent_id: String,
    pub to_agent_id: String,
    pub amount_cents: i64,
    pub status: String,
}

pub async fn request_money(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(requester_id): Path<String>,
    Json(payload): Json<RequestMoneyRequest>,
) -> AppResult<Json<ApiResponse<RequestMoneyResponse>>> {
    auth.ensure_agent(&requester_id)?;

    if requester_id == payload.from_agent_id {
        return Err(AppError::BadRequest(
            "cannot request money from yourself".to_string(),
        ));
    }

    if payload.amount_cents <= 0 {
        return Err(AppError::BadRequest(
            "amount must be positive".to_string(),
        ));
    }

    let mut tx = state.pool().begin().await?;

    // Verify both agents exist and get locations
    let requester_location = sqlx::query_scalar::<_, String>(
        r#"
        SELECT current_location_id FROM agents WHERE id = $1
        "#,
    )
    .bind(&requester_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let _target_exists = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id FROM agents WHERE id = $1
        "#,
    )
    .bind(&payload.from_agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    // Insert pending transaction
    let transaction_id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO economy_transactions (from_agent_id, to_agent_id, amount_cents, reason, transaction_type, status, location_id)
        VALUES ($1, $2, $3, $4, 'money_request', 'pending', $5)
        RETURNING id
        "#,
    )
    .bind(&payload.from_agent_id)
    .bind(&requester_id)
    .bind(payload.amount_cents)
    .bind(&payload.reason)
    .bind(&requester_location)
    .fetch_one(&mut *tx)
    .await?;

    // Log event
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("economy.money_requested")
    .bind(&requester_id)
    .bind(&requester_location)
    .bind(format!(
        "Agent {} requested {} cents from agent {}",
        requester_id, payload.amount_cents, payload.from_agent_id
    ))
    .bind(
        serde_json::json!({
            "from_agent_id": payload.from_agent_id,
            "amount_cents": payload.amount_cents,
            "reason": payload.reason,
            "transaction_id": transaction_id,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let notification = NotificationPayload {
        message: format!(
            "Money request sent: {} cents from agent {}",
            payload.amount_cents, payload.from_agent_id
        ),
        mode: NotificationMode::Instant,
        eta_seconds: None,
    };

    Ok(Json(ApiResponse {
        data: RequestMoneyResponse {
            transaction_id,
            from_agent_id: payload.from_agent_id,
            to_agent_id: requester_id,
            amount_cents: payload.amount_cents,
            status: "pending".to_string(),
        },
        notification: Some(notification),
    }))
}

// ---------------------------------------------------------------------------
// Respond to money request
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RespondMoneyRequestRequest {
    pub transaction_id: i64,
    pub accept: bool,
}

#[derive(Debug, Serialize)]
pub struct RespondMoneyRequestResponse {
    pub transaction_id: i64,
    pub status: String,
    pub amount_cents: i64,
    pub to_agent_id: String,
}

pub async fn respond_money_request(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(agent_id): Path<String>,
    Json(payload): Json<RespondMoneyRequestRequest>,
) -> AppResult<Json<ApiResponse<RespondMoneyRequestResponse>>> {
    auth.ensure_agent(&agent_id)?;

    let mut tx = state.pool().begin().await?;

    // Load the pending transaction
    let (from_agent_id, to_agent_id, amount_cents, tx_status, tx_type) = sqlx::query_as::<_, (String, String, i64, String, String)>(
        r#"
        SELECT from_agent_id, to_agent_id, amount_cents, status, transaction_type
        FROM economy_transactions
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(payload.transaction_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    // Verify this agent is the one being asked (from_agent_id)
    if from_agent_id != agent_id {
        return Err(AppError::BadRequest(
            "only the requested agent can respond to this money request".to_string(),
        ));
    }

    if tx_type != "money_request" {
        return Err(AppError::BadRequest(
            "transaction is not a money request".to_string(),
        ));
    }

    if tx_status != "pending" {
        return Err(AppError::BadRequest(
            format!("transaction is already {}", tx_status),
        ));
    }

    if payload.accept {
        // Check sufficient balance
        let balance = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT balance_cents FROM agents WHERE id = $1 FOR UPDATE
            "#,
        )
        .bind(&agent_id)
        .fetch_one(&mut *tx)
        .await?;

        if balance < amount_cents {
            // Leave request as pending — payer can retry after earning more
            return Err(AppError::BadRequest(
                format!("insufficient balance (have {}, need {})", balance, amount_cents),
            ));
        }

        // Debit payer (with balance guard)
        let debit_rows = sqlx::query(
            r#"
            UPDATE agents
            SET balance_cents = balance_cents - $1,
                last_expense_cents = $1,
                last_expense_reason = 'Money request fulfilled',
                last_expense_at = NOW(),
                updated_at = NOW()
            WHERE id = $2 AND balance_cents >= $1
            "#,
        )
        .bind(amount_cents)
        .bind(&agent_id)
        .execute(&mut *tx)
        .await?;

        if debit_rows.rows_affected() == 0 {
            return Err(AppError::BadRequest(
                format!("insufficient balance (need {} cents)", amount_cents)
            ));
        }

        // Credit requester
        sqlx::query(
            r#"
            UPDATE agents
            SET balance_cents = balance_cents + $1,
                last_income_cents = $1,
                last_income_reason = 'Money request fulfilled',
                last_income_at = NOW(),
                updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(amount_cents)
        .bind(&to_agent_id)
        .execute(&mut *tx)
        .await?;

        // Mark transaction completed
        sqlx::query(
            r#"
            UPDATE economy_transactions
            SET status = 'completed', resolved_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(payload.transaction_id)
        .execute(&mut *tx)
        .await?;

        // Log event
        sqlx::query(
            r#"
            INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
            VALUES ($1, $2, $3, $4, $5::jsonb, $6)
            "#,
        )
        .bind("economy.money_request_accepted")
        .bind(&agent_id)
        .bind(
            sqlx::query_scalar::<_, String>(
                r#"SELECT current_location_id FROM agents WHERE id = $1"#,
            )
            .bind(&agent_id)
            .fetch_one(&mut *tx)
            .await?,
        )
        .bind(format!(
            "Agent {} accepted money request: {} cents to agent {}",
            agent_id, amount_cents, to_agent_id
        ))
        .bind(
            serde_json::json!({
                "transaction_id": payload.transaction_id,
                "amount_cents": amount_cents,
                "to_agent_id": to_agent_id,
            })
            .to_string(),
        )
        .bind(Utc::now())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(Json(ApiResponse::from(RespondMoneyRequestResponse {
            transaction_id: payload.transaction_id,
            status: "completed".to_string(),
            amount_cents,
            to_agent_id,
        })))
    } else {
        // Reject
        sqlx::query(
            r#"
            UPDATE economy_transactions
            SET status = 'rejected', resolved_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(payload.transaction_id)
        .execute(&mut *tx)
        .await?;

        // Log event
        sqlx::query(
            r#"
            INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
            VALUES ($1, $2, $3, $4, $5::jsonb, $6)
            "#,
        )
        .bind("economy.money_request_rejected")
        .bind(&agent_id)
        .bind(
            sqlx::query_scalar::<_, String>(
                r#"SELECT current_location_id FROM agents WHERE id = $1"#,
            )
            .bind(&agent_id)
            .fetch_one(&mut *tx)
            .await?,
        )
        .bind(format!(
            "Agent {} rejected money request from agent {}",
            agent_id, to_agent_id
        ))
        .bind(
            serde_json::json!({
                "transaction_id": payload.transaction_id,
                "amount_cents": amount_cents,
                "to_agent_id": to_agent_id,
            })
            .to_string(),
        )
        .bind(Utc::now())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(Json(ApiResponse::from(RespondMoneyRequestResponse {
            transaction_id: payload.transaction_id,
            status: "rejected".to_string(),
            amount_cents,
            to_agent_id,
        })))
    }
}

// ---------------------------------------------------------------------------
// Transaction log
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, FromRow)]
pub struct EconomyTransaction {
    pub id: i64,
    pub from_agent_id: Option<String>,
    pub to_agent_id: Option<String>,
    pub amount_cents: i64,
    pub reason: Option<String>,
    pub transaction_type: String,
    pub status: String,
    pub location_id: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
    pub resolved_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct GetTransactionLogRequest {
    pub limit: Option<i64>,
}

pub async fn get_transaction_log(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(payload): Json<GetTransactionLogRequest>,
) -> AppResult<Json<Vec<EconomyTransaction>>> {
    let limit = payload.limit.unwrap_or(20).clamp(1, 100);

    let transactions = sqlx::query_as::<_, EconomyTransaction>(
        r#"
        SELECT id, from_agent_id, to_agent_id, amount_cents, reason,
               transaction_type, status, location_id, created_at, resolved_at
        FROM economy_transactions
        WHERE from_agent_id = $1 OR to_agent_id = $1
        ORDER BY created_at DESC
        LIMIT $2
        "#,
    )
    .bind(&agent_id)
    .bind(limit)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(transactions))
}
