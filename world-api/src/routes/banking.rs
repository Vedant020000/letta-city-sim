use axum::{Json, extract::State};
use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::AgentId;
use crate::error::{AppError, AppResult};
use crate::models::common::ApiResponse;
use crate::state::AppState;

const DEFAULT_BANK_ID: &str = "smallville_bank";
const MAX_DAILY_RATE: f64 = 0.05;
const MAX_AGENT_OUTSTANDING_LOANS_CENTS: i64 = 50_000;

#[derive(Debug, Clone, sqlx::FromRow)]
struct BankRow {
    id: String,
    name: String,
    location_prefix: String,
    balance_cents: i64,
    banker_job_id: Option<String>,
    deposit_rate_daily: f64,
    loan_rate_daily: f64,
    reserve_ratio: f64,
    opens_at: i16,
    closes_at: i16,
}

#[derive(Debug, Serialize)]
pub struct BankRatesResponse {
    pub bank_id: String,
    pub bank_name: String,
    pub open: bool,
    pub hours: String,
    pub deposit_rate_daily: f64,
    pub loan_rate_daily: f64,
    pub deposit_apy_estimate: f64,
    pub loan_apy_estimate: f64,
    pub reserve_ratio: f64,
}

#[derive(Debug, Serialize)]
pub struct BankLoanSummary {
    pub loan_id: String,
    pub principal_cents: i64,
    pub outstanding_cents: i64,
    pub daily_rate: f64,
    pub status: String,
    pub last_accrued_at: String,
}

#[derive(Debug, Serialize)]
pub struct BankAccountResponse {
    pub bank_id: String,
    pub agent_id: String,
    pub cash_balance_cents: i64,
    pub deposit_balance_cents: i64,
    pub deposit_rate_daily: f64,
    pub active_loans: Vec<BankLoanSummary>,
    pub total_outstanding_loan_cents: i64,
    pub as_of: String,
}

#[derive(Debug, Deserialize)]
pub struct AmountRequest {
    pub amount_cents: i64,
}

#[derive(Debug, Deserialize)]
pub struct TakeLoanRequest {
    pub amount_cents: i64,
    pub purpose: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RepayLoanRequest {
    pub loan_id: String,
    pub amount_cents: i64,
}

#[derive(Debug, Deserialize)]
pub struct SetBankRatesRequest {
    pub deposit_rate_daily: f64,
    pub loan_rate_daily: f64,
}

#[derive(Debug, Serialize)]
pub struct BankTransferResponse {
    pub bank_id: String,
    pub agent_id: String,
    pub amount_cents: i64,
    pub cash_balance_cents: i64,
    pub deposit_balance_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct TakeLoanResponse {
    pub bank_id: String,
    pub agent_id: String,
    pub loan_id: String,
    pub amount_cents: i64,
    pub outstanding_cents: i64,
    pub daily_rate: f64,
    pub cash_balance_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct RepayLoanResponse {
    pub bank_id: String,
    pub agent_id: String,
    pub loan_id: String,
    pub amount_applied_cents: i64,
    pub outstanding_cents: i64,
    pub loan_status: String,
    pub cash_balance_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct SetBankRatesResponse {
    pub bank_id: String,
    pub deposit_rate_daily: f64,
    pub loan_rate_daily: f64,
    pub updated_by: String,
}

#[derive(Debug, Serialize)]
pub struct BalanceSheetResponse {
    pub bank_id: String,
    pub bank_name: String,
    pub bank_balance_cents: i64,
    pub total_deposits_cents: i64,
    pub total_outstanding_loans_cents: i64,
    pub reserve_requirement_cents: i64,
    pub available_to_lend_cents: i64,
    pub deposit_rate_daily: f64,
    pub loan_rate_daily: f64,
    pub reserve_ratio: f64,
}

pub async fn action_check_bank_rates(
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<BankRatesResponse>>> {
    let bank = get_default_bank(state.pool()).await?;
    let (sim_time, _, _, _) = crate::routes::world::compute_sim_time(state.pool()).await;
    let sim_hour = sim_time.hour() as i16;
    let open = bank.opens_at <= sim_hour && bank.closes_at > sim_hour;
    Ok(Json(ApiResponse::from(BankRatesResponse {
        bank_id: bank.id,
        bank_name: bank.name,
        open,
        hours: format!("{}am - {}pm", bank.opens_at, if bank.closes_at > 12 { bank.closes_at - 12 } else { bank.closes_at }),
        deposit_rate_daily: bank.deposit_rate_daily,
        loan_rate_daily: bank.loan_rate_daily,
        deposit_apy_estimate: (1.0 + bank.deposit_rate_daily).powf(365.0) - 1.0,
        loan_apy_estimate: (1.0 + bank.loan_rate_daily).powf(365.0) - 1.0,
        reserve_ratio: bank.reserve_ratio,
    })))
}

pub async fn action_check_bank_account(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<BankAccountResponse>>> {
    let (sim_time, _, _, _) = crate::routes::world::compute_sim_time(state.pool()).await;
    let mut tx = state.pool().begin().await?;
    let bank = get_default_bank_tx(&mut tx).await?;
    ensure_account_tx(&mut tx, &bank.id, &agent_id, sim_time).await?;
    accrue_account_tx(&mut tx, &bank, &agent_id, sim_time).await?;
    accrue_agent_loans_tx(&mut tx, &bank.id, &agent_id, sim_time).await?;
    let response = account_response_tx(&mut tx, &bank, &agent_id, sim_time).await?;
    tx.commit().await?;
    Ok(Json(ApiResponse::from(response)))
}

pub async fn action_deposit_money(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<AmountRequest>,
) -> AppResult<Json<ApiResponse<BankTransferResponse>>> {
    validate_amount(payload.amount_cents)?;
    let (sim_time, _, _, _) = crate::routes::world::compute_sim_time(state.pool()).await;
    let mut tx = state.pool().begin().await?;
    let bank = get_default_bank_tx(&mut tx).await?;
    ensure_account_tx(&mut tx, &bank.id, &agent_id, sim_time).await?;
    accrue_account_tx(&mut tx, &bank, &agent_id, sim_time).await?;

    let cash_balance = sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE agents
        SET balance_cents = balance_cents - $1,
            last_expense_cents = $1,
            last_expense_reason = 'bank deposit',
            last_expense_at = NOW(),
            updated_at = NOW()
        WHERE id = $2 AND balance_cents >= $1
        RETURNING balance_cents
        "#,
    )
    .bind(payload.amount_cents)
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::BadRequest("insufficient cash balance for deposit".to_string()))?;

    let deposit_balance = sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE bank_accounts
        SET balance_cents = balance_cents + $1,
            updated_at = NOW()
        WHERE bank_id = $2 AND agent_id = $3
        RETURNING balance_cents
        "#,
    )
    .bind(payload.amount_cents)
    .bind(&bank.id)
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE banks SET balance_cents = balance_cents + $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(payload.amount_cents)
    .bind(&bank.id)
    .execute(&mut *tx)
    .await?;

    insert_ledger_tx(
        &mut tx,
        &bank.id,
        &agent_id,
        None,
        "deposit",
        payload.amount_cents,
        json!({}),
    )
    .await?;
    insert_bank_event_tx(
        &mut tx,
        "bank.deposit",
        &agent_id,
        &bank.location_prefix,
        "Agent deposited money",
        json!({"amount_cents": payload.amount_cents, "bank_id": bank.id}),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(ApiResponse::from(BankTransferResponse {
        bank_id: bank.id,
        agent_id,
        amount_cents: payload.amount_cents,
        cash_balance_cents: cash_balance,
        deposit_balance_cents: deposit_balance,
    })))
}

pub async fn action_withdraw_money(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<AmountRequest>,
) -> AppResult<Json<ApiResponse<BankTransferResponse>>> {
    validate_amount(payload.amount_cents)?;
    let (sim_time, _, _, _) = crate::routes::world::compute_sim_time(state.pool()).await;
    let mut tx = state.pool().begin().await?;
    let bank = get_default_bank_tx(&mut tx).await?;
    ensure_account_tx(&mut tx, &bank.id, &agent_id, sim_time).await?;
    accrue_account_tx(&mut tx, &bank, &agent_id, sim_time).await?;

    let deposit_balance = sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE bank_accounts
        SET balance_cents = balance_cents - $1,
            updated_at = NOW()
        WHERE bank_id = $2 AND agent_id = $3 AND balance_cents >= $1
        RETURNING balance_cents
        "#,
    )
    .bind(payload.amount_cents)
    .bind(&bank.id)
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest("insufficient bank deposit balance for withdrawal".to_string())
    })?;

    let cash_balance = sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE agents
        SET balance_cents = balance_cents + $1,
            last_income_cents = $1,
            last_income_reason = 'bank withdrawal',
            last_income_at = NOW(),
            updated_at = NOW()
        WHERE id = $2
        RETURNING balance_cents
        "#,
    )
    .bind(payload.amount_cents)
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE banks SET balance_cents = balance_cents - $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(payload.amount_cents)
    .bind(&bank.id)
    .execute(&mut *tx)
    .await?;

    insert_ledger_tx(
        &mut tx,
        &bank.id,
        &agent_id,
        None,
        "withdrawal",
        payload.amount_cents,
        json!({}),
    )
    .await?;
    insert_bank_event_tx(
        &mut tx,
        "bank.withdrawal",
        &agent_id,
        &bank.location_prefix,
        "Agent withdrew money",
        json!({"amount_cents": payload.amount_cents, "bank_id": bank.id}),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(ApiResponse::from(BankTransferResponse {
        bank_id: bank.id,
        agent_id,
        amount_cents: payload.amount_cents,
        cash_balance_cents: cash_balance,
        deposit_balance_cents: deposit_balance,
    })))
}

pub async fn action_take_loan(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<TakeLoanRequest>,
) -> AppResult<Json<ApiResponse<TakeLoanResponse>>> {
    validate_amount(payload.amount_cents)?;
    let (sim_time, _, _, _) = crate::routes::world::compute_sim_time(state.pool()).await;
    let mut tx = state.pool().begin().await?;
    let bank = get_default_bank_tx(&mut tx).await?;

    // Check bank is open
    let sim_hour = sim_time.hour() as i16;
    if bank.opens_at > sim_hour || bank.closes_at <= sim_hour {
        return Err(AppError::BadRequest("the bank is closed right now".to_string()));
    }

    // Accrue only this agent's accounts/loans (not everyone's)
    ensure_account_tx(&mut tx, &bank.id, &agent_id, sim_time).await?;
    accrue_account_tx(&mut tx, &bank, &agent_id, sim_time).await?;
    accrue_agent_loans_tx(&mut tx, &bank.id, &agent_id, sim_time).await?;

    let outstanding = total_agent_outstanding_loans_tx(&mut tx, &bank.id, &agent_id).await?;
    if outstanding + payload.amount_cents > MAX_AGENT_OUTSTANDING_LOANS_CENTS {
        return Err(AppError::BadRequest(format!(
            "loan limit exceeded: active loans plus requested amount cannot exceed {} cents",
            MAX_AGENT_OUTSTANDING_LOANS_CENTS
        )));
    }

    let available = available_to_lend_tx(&mut tx, &bank).await?;
    if payload.amount_cents > available {
        return Err(AppError::BadRequest(format!(
            "bank cannot lend that much right now (available: {} cents)",
            available
        )));
    }

    let loan_id = format!("loan_{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO bank_loans (
            id, bank_id, agent_id, principal_cents, outstanding_cents,
            daily_rate, status, last_accrued_at
        )
        VALUES ($1, $2, $3, $4, $4, $5, 'active', $6)
        "#,
    )
    .bind(&loan_id)
    .bind(&bank.id)
    .bind(&agent_id)
    .bind(payload.amount_cents)
    .bind(bank.loan_rate_daily)
    .bind(sim_time)
    .execute(&mut *tx)
    .await?;

    let cash_balance = sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE agents
        SET balance_cents = balance_cents + $1,
            last_income_cents = $1,
            last_income_reason = 'bank loan',
            last_income_at = NOW(),
            updated_at = NOW()
        WHERE id = $2
        RETURNING balance_cents
        "#,
    )
    .bind(payload.amount_cents)
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE banks SET balance_cents = balance_cents - $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(payload.amount_cents)
    .bind(&bank.id)
    .execute(&mut *tx)
    .await?;

    insert_ledger_tx(
        &mut tx,
        &bank.id,
        &agent_id,
        Some(&loan_id),
        "loan_disbursement",
        payload.amount_cents,
        json!({"purpose": payload.purpose}),
    )
    .await?;
    insert_bank_event_tx(
        &mut tx,
        "bank.loan_disbursement",
        &agent_id,
        &bank.location_prefix,
        "Agent took out a bank loan",
        json!({"amount_cents": payload.amount_cents, "loan_id": loan_id, "bank_id": bank.id}),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(ApiResponse::from(TakeLoanResponse {
        bank_id: bank.id,
        agent_id,
        loan_id,
        amount_cents: payload.amount_cents,
        outstanding_cents: payload.amount_cents,
        daily_rate: bank.loan_rate_daily,
        cash_balance_cents: cash_balance,
    })))
}

pub async fn action_repay_loan(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<RepayLoanRequest>,
) -> AppResult<Json<ApiResponse<RepayLoanResponse>>> {
    validate_amount(payload.amount_cents)?;
    let (sim_time, _, _, _) = crate::routes::world::compute_sim_time(state.pool()).await;
    let mut tx = state.pool().begin().await?;
    let bank = get_default_bank_tx(&mut tx).await?;
    accrue_loan_tx(&mut tx, &bank.id, &payload.loan_id, sim_time).await?;

    let outstanding = sqlx::query_scalar::<_, i64>(
        r#"SELECT outstanding_cents FROM bank_loans WHERE id = $1 AND bank_id = $2 AND agent_id = $3 AND status = 'active' FOR UPDATE"#,
    )
    .bind(&payload.loan_id)
    .bind(&bank.id)
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let amount_applied = payload.amount_cents.min(outstanding);
    let cash_balance = sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE agents
        SET balance_cents = balance_cents - $1,
            last_expense_cents = $1,
            last_expense_reason = 'bank loan repayment',
            last_expense_at = NOW(),
            updated_at = NOW()
        WHERE id = $2 AND balance_cents >= $1
        RETURNING balance_cents
        "#,
    )
    .bind(amount_applied)
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest("insufficient cash balance for loan repayment".to_string())
    })?;

    let new_outstanding = outstanding - amount_applied;
    let status = if new_outstanding == 0 {
        "paid"
    } else {
        "active"
    };
    sqlx::query(
        r#"
        UPDATE bank_loans
        SET outstanding_cents = $1,
            status = $2,
            closed_at = CASE WHEN $2 = 'paid' THEN NOW() ELSE closed_at END,
            updated_at = NOW()
        WHERE id = $3
        "#,
    )
    .bind(new_outstanding)
    .bind(status)
    .bind(&payload.loan_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE banks SET balance_cents = balance_cents + $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(amount_applied)
    .bind(&bank.id)
    .execute(&mut *tx)
    .await?;

    insert_ledger_tx(
        &mut tx,
        &bank.id,
        &agent_id,
        Some(&payload.loan_id),
        "loan_repayment",
        amount_applied,
        json!({}),
    )
    .await?;
    insert_bank_event_tx(
        &mut tx,
        "bank.loan_repayment",
        &agent_id,
        &bank.location_prefix,
        "Agent repaid a bank loan",
        json!({"amount_cents": amount_applied, "loan_id": payload.loan_id, "bank_id": bank.id}),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(ApiResponse::from(RepayLoanResponse {
        bank_id: bank.id,
        agent_id,
        loan_id: payload.loan_id,
        amount_applied_cents: amount_applied,
        outstanding_cents: new_outstanding,
        loan_status: status.to_string(),
        cash_balance_cents: cash_balance,
    })))
}

pub async fn action_set_bank_rates(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<SetBankRatesRequest>,
) -> AppResult<Json<ApiResponse<SetBankRatesResponse>>> {
    validate_rates(payload.deposit_rate_daily, payload.loan_rate_daily)?;
    let (sim_time, _, _, _) = crate::routes::world::compute_sim_time(state.pool()).await;
    let mut tx = state.pool().begin().await?;
    ensure_banker_tx(&mut tx, &agent_id).await?;
    let bank = get_default_bank_tx(&mut tx).await?;
    // Full accrual for rate changes (affects all accounts)
    accrue_bank_tx(&mut tx, &bank, sim_time).await?;

    sqlx::query(
        r#"
        UPDATE banks
        SET deposit_rate_daily = $1,
            loan_rate_daily = $2,
            updated_by = $3,
            updated_at = NOW()
        WHERE id = $4
        "#,
    )
    .bind(payload.deposit_rate_daily)
    .bind(payload.loan_rate_daily)
    .bind(&agent_id)
    .bind(&bank.id)
    .execute(&mut *tx)
    .await?;

    insert_ledger_tx(&mut tx, &bank.id, &agent_id, None, "rate_change", 0, json!({"deposit_rate_daily": payload.deposit_rate_daily, "loan_rate_daily": payload.loan_rate_daily})).await?;
    insert_bank_event_tx(&mut tx, "bank.rate_change", &agent_id, &bank.location_prefix, "Banker updated bank rates", json!({"bank_id": bank.id, "deposit_rate_daily": payload.deposit_rate_daily, "loan_rate_daily": payload.loan_rate_daily})).await?;
    tx.commit().await?;

    Ok(Json(ApiResponse::from(SetBankRatesResponse {
        bank_id: bank.id,
        deposit_rate_daily: payload.deposit_rate_daily,
        loan_rate_daily: payload.loan_rate_daily,
        updated_by: agent_id,
    })))
}

pub async fn action_check_bank_balance_sheet(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<BalanceSheetResponse>>> {
    let (sim_time, _, _, _) = crate::routes::world::compute_sim_time(state.pool()).await;
    let mut tx = state.pool().begin().await?;
    ensure_banker_tx(&mut tx, &agent_id).await?;
    let mut bank = get_default_bank_tx(&mut tx).await?;
    accrue_bank_tx(&mut tx, &bank, sim_time).await?;
    bank = get_default_bank_tx(&mut tx).await?;
    let response = balance_sheet_tx(&mut tx, &bank).await?;
    tx.commit().await?;
    Ok(Json(ApiResponse::from(response)))
}

fn validate_amount(amount_cents: i64) -> AppResult<()> {
    if amount_cents <= 0 {
        return Err(AppError::BadRequest(
            "amount_cents must be positive".to_string(),
        ));
    }
    Ok(())
}

fn validate_rates(deposit_rate_daily: f64, loan_rate_daily: f64) -> AppResult<()> {
    if !deposit_rate_daily.is_finite() || !loan_rate_daily.is_finite() {
        return Err(AppError::BadRequest(
            "rates must be finite numbers".to_string(),
        ));
    }
    if deposit_rate_daily < 0.0 || loan_rate_daily < 0.0 {
        return Err(AppError::BadRequest("rates cannot be negative".to_string()));
    }
    if deposit_rate_daily > MAX_DAILY_RATE || loan_rate_daily > MAX_DAILY_RATE {
        return Err(AppError::BadRequest(format!(
            "rates cannot exceed {} daily",
            MAX_DAILY_RATE
        )));
    }
    if loan_rate_daily < deposit_rate_daily {
        return Err(AppError::BadRequest(
            "loan rate must be greater than or equal to deposit rate".to_string(),
        ));
    }
    Ok(())
}

async fn get_default_bank(pool: &sqlx::PgPool) -> AppResult<BankRow> {
    sqlx::query_as::<_, BankRow>(
        r#"
        SELECT id, name, location_prefix, balance_cents, banker_job_id, deposit_rate_daily, loan_rate_daily, reserve_ratio, opens_at, closes_at
        FROM banks
        WHERE id = $1
        "#,
    )
    .bind(DEFAULT_BANK_ID)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

async fn get_default_bank_tx(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> AppResult<BankRow> {
    sqlx::query_as::<_, BankRow>(
        r#"
        SELECT id, name, location_prefix, balance_cents, banker_job_id, deposit_rate_daily, loan_rate_daily, reserve_ratio, opens_at, closes_at
        FROM banks
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(DEFAULT_BANK_ID)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

async fn ensure_account_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bank_id: &str,
    agent_id: &str,
    sim_time: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO bank_accounts (bank_id, agent_id, balance_cents, last_accrued_at)
        VALUES ($1, $2, 0, $3)
        ON CONFLICT (bank_id, agent_id) DO NOTHING
        "#,
    )
    .bind(bank_id)
    .bind(agent_id)
    .bind(sim_time)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn accrue_account_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bank: &BankRow,
    agent_id: &str,
    sim_time: DateTime<Utc>,
) -> AppResult<()> {
    let row = sqlx::query_as::<_, (i64, DateTime<Utc>)>(
        r#"
        SELECT balance_cents, last_accrued_at
        FROM bank_accounts
        WHERE bank_id = $1 AND agent_id = $2
        FOR UPDATE
        "#,
    )
    .bind(&bank.id)
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await?;

    let new_balance = apply_interest(row.0, bank.deposit_rate_daily, row.1, sim_time);
    let delta = new_balance - row.0;
    sqlx::query(
        r#"
        UPDATE bank_accounts
        SET balance_cents = $1,
            last_accrued_at = $2,
            updated_at = NOW()
        WHERE bank_id = $3 AND agent_id = $4
        "#,
    )
    .bind(new_balance)
    .bind(sim_time)
    .bind(&bank.id)
    .bind(agent_id)
    .execute(&mut **tx)
    .await?;

    if delta > 0 {
        sqlx::query(
            "UPDATE banks SET balance_cents = balance_cents - $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(delta)
        .bind(&bank.id)
        .execute(&mut **tx)
        .await?;
        insert_ledger_tx(
            tx,
            &bank.id,
            agent_id,
            None,
            "deposit_interest",
            delta,
            json!({"rate_daily": bank.deposit_rate_daily}),
        )
        .await?;
    }

    Ok(())
}

async fn accrue_bank_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bank: &BankRow,
    sim_time: DateTime<Utc>,
) -> AppResult<()> {
    let account_agent_ids = sqlx::query_scalar::<_, String>(
        "SELECT agent_id FROM bank_accounts WHERE bank_id = $1 ORDER BY agent_id",
    )
    .bind(&bank.id)
    .fetch_all(&mut **tx)
    .await?;

    for account_agent_id in account_agent_ids {
        accrue_account_tx(tx, bank, &account_agent_id, sim_time).await?;
    }

    let loan_ids = sqlx::query_scalar::<_, String>(
        "SELECT id FROM bank_loans WHERE bank_id = $1 AND status = 'active' ORDER BY created_at",
    )
    .bind(&bank.id)
    .fetch_all(&mut **tx)
    .await?;

    for loan_id in loan_ids {
        accrue_loan_tx(tx, &bank.id, &loan_id, sim_time).await?;
    }

    Ok(())
}

async fn accrue_agent_loans_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bank_id: &str,
    agent_id: &str,
    sim_time: DateTime<Utc>,
) -> AppResult<()> {
    let loan_ids = sqlx::query_scalar::<_, String>(
        "SELECT id FROM bank_loans WHERE bank_id = $1 AND agent_id = $2 AND status = 'active' ORDER BY created_at",
    )
    .bind(bank_id)
    .bind(agent_id)
    .fetch_all(&mut **tx)
    .await?;

    for loan_id in loan_ids {
        accrue_loan_tx(tx, bank_id, &loan_id, sim_time).await?;
    }

    Ok(())
}

async fn accrue_loan_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bank_id: &str,
    loan_id: &str,
    sim_time: DateTime<Utc>,
) -> AppResult<()> {
    let row = sqlx::query_as::<_, (String, i64, f64, DateTime<Utc>)>(
        r#"
        SELECT agent_id, outstanding_cents, daily_rate, last_accrued_at
        FROM bank_loans
        WHERE id = $1 AND bank_id = $2 AND status = 'active'
        FOR UPDATE
        "#,
    )
    .bind(loan_id)
    .bind(bank_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some((agent_id, outstanding, daily_rate, last_accrued_at)) = row else {
        return Ok(());
    };

    let new_outstanding = apply_interest(outstanding, daily_rate, last_accrued_at, sim_time);
    let delta = new_outstanding - outstanding;
    sqlx::query(
        r#"
        UPDATE bank_loans
        SET outstanding_cents = $1,
            last_accrued_at = $2,
            updated_at = NOW()
        WHERE id = $3
        "#,
    )
    .bind(new_outstanding)
    .bind(sim_time)
    .bind(loan_id)
    .execute(&mut **tx)
    .await?;

    if delta > 0 {
        insert_ledger_tx(
            tx,
            bank_id,
            &agent_id,
            Some(loan_id),
            "loan_interest",
            delta,
            json!({"rate_daily": daily_rate}),
        )
        .await?;
    }

    Ok(())
}

fn apply_interest(
    balance_cents: i64,
    rate_daily: f64,
    last_accrued_at: DateTime<Utc>,
    sim_time: DateTime<Utc>,
) -> i64 {
    if balance_cents <= 0 || rate_daily <= 0.0 || sim_time <= last_accrued_at {
        return balance_cents;
    }

    let elapsed_days = (sim_time - last_accrued_at).num_seconds() as f64 / 86_400.0;
    if elapsed_days <= 0.0 {
        return balance_cents;
    }

    ((balance_cents as f64) * (1.0 + rate_daily).powf(elapsed_days)).round() as i64
}

async fn account_response_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bank: &BankRow,
    agent_id: &str,
    sim_time: DateTime<Utc>,
) -> AppResult<BankAccountResponse> {
    let cash_balance =
        sqlx::query_scalar::<_, i64>("SELECT balance_cents FROM agents WHERE id = $1")
            .bind(agent_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(AppError::NotFound)?;
    let deposit_balance = sqlx::query_scalar::<_, i64>(
        "SELECT balance_cents FROM bank_accounts WHERE bank_id = $1 AND agent_id = $2",
    )
    .bind(&bank.id)
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await?;
    let loans = sqlx::query_as::<_, (String, i64, i64, f64, String, DateTime<Utc>)>(
        r#"
        SELECT id, principal_cents, outstanding_cents, daily_rate, status, last_accrued_at
        FROM bank_loans
        WHERE bank_id = $1 AND agent_id = $2 AND status = 'active'
        ORDER BY created_at
        "#,
    )
    .bind(&bank.id)
    .bind(agent_id)
    .fetch_all(&mut **tx)
    .await?;

    let active_loans: Vec<BankLoanSummary> = loans
        .into_iter()
        .map(|loan| BankLoanSummary {
            loan_id: loan.0,
            principal_cents: loan.1,
            outstanding_cents: loan.2,
            daily_rate: loan.3,
            status: loan.4,
            last_accrued_at: loan.5.to_rfc3339(),
        })
        .collect();
    let total_outstanding_loan_cents = active_loans.iter().map(|loan| loan.outstanding_cents).sum();

    Ok(BankAccountResponse {
        bank_id: bank.id.clone(),
        agent_id: agent_id.to_string(),
        cash_balance_cents: cash_balance,
        deposit_balance_cents: deposit_balance,
        deposit_rate_daily: bank.deposit_rate_daily,
        active_loans,
        total_outstanding_loan_cents,
        as_of: sim_time.to_rfc3339(),
    })
}

async fn total_agent_outstanding_loans_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bank_id: &str,
    agent_id: &str,
) -> AppResult<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(outstanding_cents), 0)::BIGINT FROM bank_loans WHERE bank_id = $1 AND agent_id = $2 AND status = 'active'",
    )
    .bind(bank_id)
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(Into::into)
}

async fn available_to_lend_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bank: &BankRow,
) -> AppResult<i64> {
    let total_deposits = total_deposits_tx(tx, &bank.id).await?;
    let reserve_requirement = ((total_deposits as f64) * bank.reserve_ratio).ceil() as i64;
    Ok((bank.balance_cents - reserve_requirement).max(0))
}

async fn total_deposits_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bank_id: &str,
) -> AppResult<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(balance_cents), 0)::BIGINT FROM bank_accounts WHERE bank_id = $1",
    )
    .bind(bank_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(Into::into)
}

async fn total_outstanding_loans_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bank_id: &str,
) -> AppResult<i64> {
    sqlx::query_scalar::<_, i64>("SELECT COALESCE(SUM(outstanding_cents), 0)::BIGINT FROM bank_loans WHERE bank_id = $1 AND status = 'active'")
        .bind(bank_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(Into::into)
}

async fn ensure_banker_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    agent_id: &str,
) -> AppResult<()> {
    let is_banker = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(
            SELECT 1 FROM agent_jobs aj
            JOIN banks b ON b.banker_job_id = aj.job_id
            WHERE aj.agent_id = $1 AND aj.status = 'active' AND b.id = $2
        )"#,
    )
    .bind(agent_id)
    .bind(DEFAULT_BANK_ID)
    .fetch_one(&mut **tx)
    .await?;

    if is_banker {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

async fn balance_sheet_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bank: &BankRow,
) -> AppResult<BalanceSheetResponse> {
    let total_deposits = total_deposits_tx(tx, &bank.id).await?;
    let total_loans = total_outstanding_loans_tx(tx, &bank.id).await?;
    let reserve_requirement = ((total_deposits as f64) * bank.reserve_ratio).ceil() as i64;
    let available = (bank.balance_cents - reserve_requirement).max(0);

    Ok(BalanceSheetResponse {
        bank_id: bank.id.clone(),
        bank_name: bank.name.clone(),
        bank_balance_cents: bank.balance_cents,
        total_deposits_cents: total_deposits,
        total_outstanding_loans_cents: total_loans,
        reserve_requirement_cents: reserve_requirement,
        available_to_lend_cents: available,
        deposit_rate_daily: bank.deposit_rate_daily,
        loan_rate_daily: bank.loan_rate_daily,
        reserve_ratio: bank.reserve_ratio,
    })
}

async fn insert_ledger_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bank_id: &str,
    agent_id: &str,
    loan_id: Option<&str>,
    entry_type: &str,
    amount_cents: i64,
    metadata: serde_json::Value,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO bank_ledger_entries (bank_id, agent_id, loan_id, entry_type, amount_cents, metadata)
        VALUES ($1, $2, $3, $4, $5, $6::jsonb)
        "#,
    )
    .bind(bank_id)
    .bind(agent_id)
    .bind(loan_id)
    .bind(entry_type)
    .bind(amount_cents)
    .bind(metadata.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_bank_event_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_type: &str,
    actor_id: &str,
    location_id: &str,
    description: &str,
    metadata: serde_json::Value,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind(event_type)
    .bind(actor_id)
    .bind(location_id)
    .bind(description)
    .bind(metadata.to_string())
    .bind(Utc::now())
    .execute(&mut **tx)
    .await?;
    Ok(())
}
