use serde_json::json;
use sqlx::Postgres;
use tracing::{info, warn};

use crate::error::AppResult;
use crate::state::AppState;

/// Vitals thresholds that trigger a proactive wake.
/// These are informational — the agent decides what to do.
const FOOD_LOW_THRESHOLD: f64 = 25.0;
const WATER_LOW_THRESHOLD: f64 = 25.0;
const STAMINA_LOW_THRESHOLD: f64 = 15.0;
const SLEEP_LOW_THRESHOLD: f64 = 15.0;

/// How often the heartbeat ticks (wall-clock seconds).
const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// How often each agent gets an idle wake (in heartbeat ticks).
/// At 30s ticks and 12x sim speed, 20 ticks = ~10 wall minutes = ~2 sim hours.
const IDLE_WAKE_EVERY_N_TICKS: u64 = 20;

/// Spawn the simulation heartbeat as a background task.
/// This drives the entire simulation forward by:
/// 1. Checking vitals thresholds and waking agents with critically low vitals
/// 2. Periodically waking idle agents so they have an opportunity to act
pub fn spawn_heartbeat(state: AppState) {
    tokio::spawn(async move {
        let mut tick: u64 = 0;
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(HEARTBEAT_INTERVAL_SECS),
        );

        info!("Simulation heartbeat started (interval={}s)", HEARTBEAT_INTERVAL_SECS);

        loop {
            interval.tick().await;
            tick += 1;

            if let Err(e) = heartbeat_tick(&state, tick).await {
                warn!("Heartbeat tick {} failed: {}", tick, e);
            }
        }
    });
}

async fn heartbeat_tick(state: &AppState, tick: u64) -> AppResult<()> {
    let mut tx = state.pool().begin().await?;

    // 0. Complete travel that has reached its ETA
    let arrivals = complete_arrived_travelers(&mut tx, state).await?;
    if arrivals > 0 {
        info!(tick, arrivals, "Travel arrivals completed");
    }

    // 1. Vitals-threshold wakes
    let vitals_wakes = check_vitals_thresholds(&mut tx).await?;
    if vitals_wakes > 0 {
        info!(tick, vitals_wakes, "Vitals-threshold wakes enqueued");
    }

    // 2. Idle heartbeat wakes (every N ticks)
    if tick % IDLE_WAKE_EVERY_N_TICKS == 0 {
        let idle_wakes = enqueue_idle_wakes(&mut tx, state).await?;
        info!(tick, idle_wakes, "Idle heartbeat wakes enqueued");
    }

    tx.commit().await?;
    Ok(())
}

async fn complete_arrived_travelers(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    state: &AppState,
) -> AppResult<usize> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        r#"
        SELECT id, current_location_id, travel_destination_id, name
        FROM agents
        WHERE state = 'traveling'
          AND travel_destination_id IS NOT NULL
          AND travel_arrives_at IS NOT NULL
          AND travel_arrives_at <= NOW()
        FOR UPDATE
        "#,
    )
    .fetch_all(&mut **tx)
    .await?;

    let mut count = 0;
    let mut signal_targets: Vec<String> = Vec::new();

    for (agent_id, from_location_id, to_location_id, agent_name) in rows {
        let updated_agent = sqlx::query_as::<_, crate::models::agent::Agent>(
            r#"
            UPDATE agents
            SET current_location_id = travel_destination_id,
                travel_destination_id = NULL,
                travel_started_at = NULL,
                travel_arrives_at = NULL,
                travel_path = NULL,
                travel_total_secs = NULL,
                travel_from_location_id = NULL,
                state = 'idle',
                current_activity = NULL,
                activity_started_at = NULL,
                state_updated_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(&agent_id)
        .fetch_one(&mut **tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
            VALUES ($1, $2, $3, $4, $5::jsonb, $6)
            "#,
        )
        .bind("agent.moved")
        .bind(&agent_id)
        .bind(&to_location_id)
        .bind(format!("Agent {} arrived at location {}", agent_id, to_location_id))
        .bind(json!({
            "from_location_id": from_location_id,
            "to_location_id": to_location_id,
            "completion": "travel_arrival",
        }).to_string())
        .bind(chrono::Utc::now())
        .execute(&mut **tx)
        .await?;

        let enqueued = crate::routes::citizens::enqueue_citizen_wake_tx(
            tx,
            &agent_id,
            "travel_arrival",
            json!({
                "kind": "travel",
                "ref": "agent.travel.arrived",
                "details": {
                    "from_location_id": from_location_id,
                    "to_location_id": to_location_id,
                }
            }),
            format!("You arrived at {}.", to_location_id),
            json!({
                "event_type": "travel.arrived",
                "from_location_id": from_location_id,
                "to_location_id": to_location_id,
                "agent_name": agent_name,
            }),
            json!([]),
            true,
        )
        .await?;

        let _ = state.event_tx().send(crate::ws_events::WorldEventEnvelope::new(
            "travel.arrived",
            vec![agent_id.clone()],
            Some(updated_agent.current_location_id.clone()),
            json!({
                "agent_id": agent_id,
                "from_location_id": from_location_id,
                "to_location_id": updated_agent.current_location_id,
            }),
        ));

        if enqueued.should_signal {
            signal_targets.push(agent_id.clone());
        }

        count += 1;
    }

    for target_id in signal_targets {
        let _ = state.citizen_signal_tx().send(target_id);
    }

    Ok(count)
}

/// Check all active agents for critically low vitals and enqueue informational wakes.
async fn check_vitals_thresholds(
    tx: &mut sqlx::Transaction<'_, Postgres>,
) -> AppResult<usize> {
    // Find agents with any vitals below threshold who don't already have an open wake
    let agents: Vec<(String, String, f64, f64, f64, f64)> = sqlx::query_as(
        r#"
        SELECT a.id, a.name, a.food_level, a.water_level, a.stamina_level, a.sleep_level
        FROM agents a
        WHERE a.is_active = TRUE
          AND (
            a.food_level < $1
            OR a.water_level < $2
            OR a.stamina_level < $3
            OR a.sleep_level < $4
          )
          AND NOT EXISTS (
            SELECT 1 FROM citizen_wakes cw
            WHERE cw.agent_id = a.id AND cw.status = 'open'
          )
        "#,
    )
    .bind(FOOD_LOW_THRESHOLD)
    .bind(WATER_LOW_THRESHOLD)
    .bind(STAMINA_LOW_THRESHOLD)
    .bind(SLEEP_LOW_THRESHOLD)
    .fetch_all(&mut **tx)
    .await?;

    let mut count = 0;
    for (agent_id, name, food, water, stamina, sleep) in &agents {
        let mut alerts: Vec<String> = Vec::new();
        if *food < FOOD_LOW_THRESHOLD {
            alerts.push(format!("food is low ({:.0})", food));
        }
        if *water < WATER_LOW_THRESHOLD {
            alerts.push(format!("water is low ({:.0})", water));
        }
        if *stamina < STAMINA_LOW_THRESHOLD {
            alerts.push(format!("stamina is low ({:.0})", stamina));
        }
        if *sleep < SLEEP_LOW_THRESHOLD {
            alerts.push(format!("sleep is low ({:.0})", sleep));
        }

        let alert_text = alerts.join(", ");
        let narrative = format!("You notice that your {}.", alert_text);
        let structured = json!({
            "event_type": "vitals.alert",
            "alerts": alerts,
            "vitals": {
                "food": food,
                "water": water,
                "stamina": stamina,
                "sleep": sleep,
            },
        });

        let enqueued = crate::routes::citizens::enqueue_citizen_wake_tx(
            tx,
            agent_id,
            "vitals_alert",
            json!({
                "kind": "vitals",
                "alerts": alerts,
            }),
            narrative,
            structured,
            json!([]),
            true,
        )
        .await?;

        if enqueued.should_signal {
            // Signal will be sent after commit
        }
        count += 1;
        info!(agent_id, name, alert_text, "Vitals alert wake enqueued");
    }

    Ok(count)
}

/// Enqueue idle wakes for all active agents who don't have an open wake.
/// This gives agents a periodic opportunity to act — no instruction, just context.
async fn enqueue_idle_wakes(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    state: &AppState,
) -> AppResult<usize> {
    let agents: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT a.id, a.name
        FROM agents a
        WHERE a.is_active = TRUE
          AND NOT EXISTS (
            SELECT 1 FROM citizen_wakes cw
            WHERE cw.agent_id = a.id AND cw.status = 'open'
          )
        "#,
    )
    .fetch_all(&mut **tx)
    .await?;

    let mut count = 0;
    let mut signal_targets: Vec<String> = Vec::new();

    for (agent_id, _name) in &agents {
        let enqueued = crate::routes::citizens::enqueue_citizen_wake_tx(
            tx,
            agent_id,
            "idle_tick",
            json!({
                "kind": "idle",
                "tick": true,
            }),
            "You have a moment to yourself. The world continues around you.".to_string(),
            json!({
                "event_type": "idle.tick",
            }),
            json!([]),
            true,
        )
        .await?;

        if enqueued.should_signal {
            signal_targets.push(agent_id.clone());
        }
        count += 1;
    }

    // Signal the CLI/daemon for each agent
    for target_id in signal_targets {
        let _ = state.citizen_signal_tx().send(target_id);
    }

    Ok(count)
}
