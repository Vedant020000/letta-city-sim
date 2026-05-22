use axum::{Json, extract::State};
use chrono::Utc;
use serde_json::{Map, Value, json};

use crate::auth::AgentId;
use crate::error::{AppError, AppResult};
use crate::models::agent::Agent;
use crate::models::common::{ApiResponse, NotificationMode, NotificationPayload};
use crate::models::object::WorldObject;
use crate::routes::citizens::enqueue_citizen_wake_tx;
use crate::routes::vitals::SleepRecoveryPlan;
use crate::state::AppState;
use crate::ws_events::WorldEventEnvelope;

fn normalize_bed_state(state: &Value) -> Map<String, Value> {
    match state.as_object() {
        Some(map) => map.clone(),
        None => Map::new(),
    }
}

fn occupied_by(state: &Value) -> Option<String> {
    state
        .get("occupied_by")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn is_shared_sleep_site(state: &Value) -> bool {
    state
        .get("shared_sleep_site")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn format_cents(cents: i64) -> String {
    format!("${:.2}", cents as f64 / 100.0)
}

fn recovery_reason(plan: &SleepRecoveryPlan) -> String {
    format!(
        "{} recovery: +{:.1} sleep/min, +{:.1} stamina/min",
        plan.label, plan.sleep_recovery_per_min, plan.stamina_recovery_per_min
    )
}

async fn location_targets(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    location_id: &str,
    agent_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    let mut targets = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM agents
        WHERE current_location_id = $1
        ORDER BY id
        "#,
    )
    .bind(location_id)
    .fetch_all(&mut **tx)
    .await?;

    if !targets.iter().any(|target| target == agent_id) {
        targets.push(agent_id.to_string());
    }

    Ok(targets)
}

pub async fn start_sleep(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<Agent>>> {
    let mut tx = state.pool().begin().await?;

    let agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT *
        FROM agents
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    if agent.state == "sleeping" {
        return Err(AppError::BadRequest(
            "agent is already sleeping".to_string(),
        ));
    }

    // Apply vitals decay before entering sleep
    let agent = crate::routes::vitals::apply_vitals_decay_tx(&mut tx, &agent_id).await?;

    let beds = sqlx::query_as::<_, WorldObject>(
        r#"
        SELECT id, name, location_id, state, actions
        FROM world_objects
        WHERE location_id = $1
          AND 'sleep' = ANY(actions)
        ORDER BY id
        FOR UPDATE
        "#,
    )
    .bind(&agent.current_location_id)
    .fetch_all(&mut *tx)
    .await?;

    let bed = match beds.len() {
        0 => {
            return Err(AppError::BadRequest(
                "no usable bed found in the current location".to_string(),
            ));
        }
        1 => beds.into_iter().next().ok_or_else(|| {
            AppError::BadRequest("no usable bed found in the current location".to_string())
        })?,
        _ => {
            return Err(AppError::BadRequest(
                "multiple usable beds found in the current location; sleep target is ambiguous"
                    .to_string(),
            ));
        }
    };

    let shared_sleep_site = is_shared_sleep_site(&bed.state);

    if !shared_sleep_site
        && let Some(current_occupant) = occupied_by(&bed.state)
        && current_occupant != agent.id
    {
        return Err(AppError::BadRequest(format!(
            "bed {} is already occupied by {}",
            bed.id, current_occupant
        )));
    }

    let sleep_plan = crate::routes::vitals::sleep_recovery_plan_for_bed_tx(
        &mut tx,
        &agent.id,
        bed.location_id.as_deref(),
        &bed.state,
    )
    .await?;

    let mut sleep_cost_cents = 0_i64;
    let mut post_payment_balance = agent.balance_cents;
    if sleep_plan.cost_cents > 0 {
        if agent.balance_cents < sleep_plan.cost_cents {
            return Err(AppError::BadRequest(format!(
                "insufficient balance for {} (have {}, need {}); try the campground fallback",
                sleep_plan.label,
                format_cents(agent.balance_cents),
                format_cents(sleep_plan.cost_cents)
            )));
        }

        post_payment_balance = sqlx::query_scalar::<_, i64>(
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
        .bind(sleep_plan.cost_cents)
        .bind(format!("{} lodging", sleep_plan.label))
        .bind(&agent.id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "insufficient balance for {} (need {})",
                sleep_plan.label,
                format_cents(sleep_plan.cost_cents)
            ))
        })?;

        sqlx::query(
            r#"
            INSERT INTO economy_transactions (
                from_agent_id, to_agent_id, amount_cents, reason,
                transaction_type, status, location_id, resolved_at
            )
            VALUES ($1, NULL, $2, $3, 'payment', 'completed', $4, NOW())
            "#,
        )
        .bind(&agent.id)
        .bind(sleep_plan.cost_cents)
        .bind(format!("{} lodging", sleep_plan.label))
        .bind(&agent.current_location_id)
        .execute(&mut *tx)
        .await?;

        sleep_cost_cents = sleep_plan.cost_cents;
    }

    let mut bed_state = normalize_bed_state(&bed.state);
    if shared_sleep_site {
        bed_state.insert("last_used_by".to_string(), Value::String(agent.id.clone()));
    } else {
        bed_state.insert("occupied_by".to_string(), Value::String(agent.id.clone()));
    }

    sqlx::query(
        r#"
        UPDATE world_objects
        SET state = $1::jsonb
        WHERE id = $2
        "#,
    )
    .bind(Value::Object(bed_state.clone()).to_string())
    .bind(&bed.id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE agents
        SET state = 'sleeping',
            current_activity = 'Sleeping',
            activity_started_at = NOW(),
            state_updated_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(&agent.id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.sleep.started")
    .bind(&agent.id)
    .bind(&agent.current_location_id)
    .bind(format!("Agent {} went to sleep", agent.id))
    .bind(
        json!({
            "agent_id": agent.id,
            "bed_id": bed.id,
            "location_id": agent.current_location_id,
            "housing_tier": &sleep_plan.tier,
            "recovery_reason": recovery_reason(&sleep_plan),
            "sleep_recovery_per_min": sleep_plan.sleep_recovery_per_min,
            "stamina_recovery_per_min": sleep_plan.stamina_recovery_per_min,
            "cost_cents": sleep_cost_cents,
            "balance_cents": post_payment_balance,
            "shared_sleep_site": shared_sleep_site,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    let targets = location_targets(&mut tx, &agent.current_location_id, &agent.id).await?;
    let mut citizen_signal_targets = Vec::new();

    for target_agent_id in targets
        .iter()
        .filter(|target| target.as_str() != agent.id.as_str())
    {
        let enqueued = enqueue_citizen_wake_tx(
            &mut tx,
            target_agent_id,
            "world_event",
            json!({
                "kind": "sleep",
                "ref": "agent.sleep.started",
                "details": {
                    "agent_id": agent.id,
                    "bed_id": bed.id,
                    "location_id": agent.current_location_id,
                    "housing_tier": &sleep_plan.tier,
                    "recovery_reason": recovery_reason(&sleep_plan),
                    "cost_cents": sleep_cost_cents,
                    "shared_sleep_site": shared_sleep_site,
                }
            }),
            format!("{} just went to sleep.", agent.name),
            json!({
                "event_type": "agent.sleep.started",
                "agent_id": agent.id,
                "agent_name": agent.name,
                "bed_id": bed.id,
                "bed_name": bed.name,
                "location_id": agent.current_location_id,
                "housing_tier": &sleep_plan.tier,
                "recovery_reason": recovery_reason(&sleep_plan),
                "cost_cents": sleep_cost_cents,
                "shared_sleep_site": shared_sleep_site,
            }),
            json!([]),
            true,
        )
        .await?;

        if enqueued.should_signal {
            citizen_signal_targets.push(target_agent_id.clone());
        }
    }

    let updated_agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT *
        FROM agents
        WHERE id = $1
        "#,
    )
    .bind(&agent.id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    for target_agent_id in citizen_signal_targets {
        let _ = state.citizen_signal_tx().send(target_agent_id);
    }

    let _ = state.event_tx().send(WorldEventEnvelope::new(
        "agent.sleep.started",
        targets,
        Some(updated_agent.current_location_id.clone()),
        json!({
            "agent_id": updated_agent.id,
            "bed_id": bed.id,
            "location_id": updated_agent.current_location_id,
            "housing_tier": &sleep_plan.tier,
            "recovery_reason": recovery_reason(&sleep_plan),
            "cost_cents": sleep_cost_cents,
            "shared_sleep_site": shared_sleep_site,
        }),
    ));

    let payment_prefix = if sleep_cost_cents > 0 {
        format!("Paid {} and ", format_cents(sleep_cost_cents))
    } else {
        String::new()
    };

    Ok(Json(ApiResponse {
        data: updated_agent,
        notification: Some(NotificationPayload {
            message: format!(
                "{}went to sleep in {}. {}.",
                payment_prefix,
                bed.name,
                recovery_reason(&sleep_plan)
            ),
            mode: NotificationMode::Instant,
            eta_seconds: None,
        }),
    }))
}

pub async fn wake_up(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<Agent>>> {
    let mut tx = state.pool().begin().await?;

    let agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT *
        FROM agents
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    if agent.state != "sleeping" {
        return Err(AppError::BadRequest(
            "agent is not currently sleeping".to_string(),
        ));
    }

    let occupied_beds = sqlx::query_as::<_, WorldObject>(
        r#"
        SELECT id, name, location_id, state, actions
        FROM world_objects
        WHERE state->>'occupied_by' = $1
        ORDER BY id
        FOR UPDATE
        "#,
    )
    .bind(&agent.id)
    .fetch_all(&mut *tx)
    .await?;

    let empty_state = Value::Null;
    let sleep_plan = match occupied_beds.first() {
        Some(bed) => {
            crate::routes::vitals::sleep_recovery_plan_for_bed_tx(
                &mut tx,
                &agent.id,
                bed.location_id.as_deref(),
                &bed.state,
            )
            .await?
        }
        None => {
            crate::routes::vitals::sleep_recovery_plan_for_bed_tx(
                &mut tx,
                &agent.id,
                Some(&agent.current_location_id),
                &empty_state,
            )
            .await?
        }
    };

    let sleep_before = agent.sleep_level;
    let stamina_before = agent.stamina_level;

    let agent =
        crate::routes::vitals::apply_sleep_recovery_with_plan_tx(&mut tx, &agent_id, &sleep_plan)
            .await?;

    let sleep_delta = agent.sleep_level - sleep_before;
    let stamina_delta = agent.stamina_level - stamina_before;

    for bed in &occupied_beds {
        let mut bed_state = normalize_bed_state(&bed.state);
        bed_state.insert("occupied_by".to_string(), Value::Null);

        sqlx::query(
            r#"
            UPDATE world_objects
            SET state = $1::jsonb
            WHERE id = $2
            "#,
        )
        .bind(Value::Object(bed_state).to_string())
        .bind(&bed.id)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        r#"
        UPDATE agents
        SET state = 'idle',
            current_activity = NULL,
            activity_started_at = NULL,
            state_updated_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(&agent.id)
    .execute(&mut *tx)
    .await?;

    let bed_id = occupied_beds.first().map(|bed| bed.id.clone());

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.sleep.ended")
    .bind(&agent.id)
    .bind(&agent.current_location_id)
    .bind(format!("Agent {} woke up", agent.id))
    .bind(
        json!({
            "agent_id": agent.id,
            "bed_id": bed_id,
            "location_id": agent.current_location_id,
            "housing_tier": &sleep_plan.tier,
            "recovery_reason": recovery_reason(&sleep_plan),
            "sleep_recovery_per_min": sleep_plan.sleep_recovery_per_min,
            "stamina_recovery_per_min": sleep_plan.stamina_recovery_per_min,
            "sleep_delta": sleep_delta,
            "stamina_delta": stamina_delta,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    let targets = location_targets(&mut tx, &agent.current_location_id, &agent.id).await?;
    let mut citizen_signal_targets = Vec::new();

    for target_agent_id in targets
        .iter()
        .filter(|target| target.as_str() != agent.id.as_str())
    {
        let enqueued = enqueue_citizen_wake_tx(
            &mut tx,
            target_agent_id,
            "world_event",
            json!({
                "kind": "sleep",
                "ref": "agent.sleep.ended",
                "details": {
                    "agent_id": agent.id,
                    "bed_id": bed_id,
                    "location_id": agent.current_location_id,
                    "housing_tier": &sleep_plan.tier,
                    "recovery_reason": recovery_reason(&sleep_plan),
                    "sleep_delta": sleep_delta,
                    "stamina_delta": stamina_delta,
                }
            }),
            format!("{} just woke up.", agent.name),
            json!({
                "event_type": "agent.sleep.ended",
                "agent_id": agent.id,
                "agent_name": agent.name,
                "bed_id": bed_id,
                "location_id": agent.current_location_id,
                "housing_tier": &sleep_plan.tier,
                "recovery_reason": recovery_reason(&sleep_plan),
                "sleep_delta": sleep_delta,
                "stamina_delta": stamina_delta,
            }),
            json!([]),
            true,
        )
        .await?;

        if enqueued.should_signal {
            citizen_signal_targets.push(target_agent_id.clone());
        }
    }

    let updated_agent = sqlx::query_as::<_, Agent>(
        r#"
        SELECT *
        FROM agents
        WHERE id = $1
        "#,
    )
    .bind(&agent.id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    for target_agent_id in citizen_signal_targets {
        let _ = state.citizen_signal_tx().send(target_agent_id);
    }

    let _ = state.event_tx().send(WorldEventEnvelope::new(
        "agent.sleep.ended",
        targets,
        Some(updated_agent.current_location_id.clone()),
        json!({
            "agent_id": updated_agent.id,
            "bed_id": bed_id,
            "location_id": updated_agent.current_location_id,
            "housing_tier": &sleep_plan.tier,
            "recovery_reason": recovery_reason(&sleep_plan),
            "sleep_delta": sleep_delta,
            "stamina_delta": stamina_delta,
        }),
    ));

    Ok(Json(ApiResponse {
        data: updated_agent,
        notification: Some(NotificationPayload {
            message: format!(
                "Woke up after {} sleep: sleep {:+}, stamina {:+}. {}.",
                sleep_plan.label,
                sleep_delta,
                stamina_delta,
                recovery_reason(&sleep_plan)
            ),
            mode: NotificationMode::Instant,
            eta_seconds: None,
        }),
    }))
}
