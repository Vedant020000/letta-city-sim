use axum::{
    Json,
    extract::{Query, State},
};
use chrono::Utc;
use serde_json::json;
use sqlx::Postgres;
use std::collections::HashSet;

use crate::auth::SimKey;
use crate::error::{AppError, AppResult};
use crate::models::event::{CreateEventRequest, EventsQuery, RouteEventInput, RouteEventResult, SimEvent, WokenAgent};
use crate::routes::citizens::enqueue_citizen_wake_tx;
use crate::state::AppState;

pub async fn list_events(
    State(state): State<AppState>,
    Query(query): Query<EventsQuery>,
) -> AppResult<Json<Vec<SimEvent>>> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);

    let events = sqlx::query_as::<_, SimEvent>(
        r#"
        SELECT id, occurred_at, type, actor_id, location_id, description, metadata, importance, visibility
        FROM events
        WHERE ($1::timestamptz IS NULL OR occurred_at >= $1)
          AND ($2::text IS NULL OR location_id = $2)
          AND ($3::text IS NULL OR actor_id = $3)
          AND ($4::text IS NULL OR type = $4)
        ORDER BY occurred_at DESC
        LIMIT $5
        "#,
    )
    .bind(query.since)
    .bind(query.location_id)
    .bind(query.actor_id)
    .bind(query.r#type)
    .bind(limit)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(events))
}

pub async fn create_event(
    State(state): State<AppState>,
    _sim_key: SimKey,
    Json(payload): Json<CreateEventRequest>,
) -> AppResult<Json<SimEvent>> {
    if payload.r#type.trim().is_empty() {
        return Err(AppError::BadRequest(
            "event type cannot be empty".to_string(),
        ));
    }

    if payload.description.trim().is_empty() {
        return Err(AppError::BadRequest(
            "event description cannot be empty".to_string(),
        ));
    }

    let created = sqlx::query_as::<_, SimEvent>(
        r#"
        INSERT INTO events (occurred_at, type, actor_id, location_id, description, metadata, importance, visibility)
        VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7, $8)
        RETURNING id, occurred_at, type, actor_id, location_id, description, metadata, importance, visibility
        "#,
    )
    .bind(Utc::now())
    .bind(payload.r#type)
    .bind(payload.actor_id)
    .bind(payload.location_id)
    .bind(payload.description)
    .bind(
        payload
            .metadata
            .unwrap_or_else(|| json!({}))
            .to_string(),
    )
    .bind(payload.importance)
    .bind(&payload.visibility)
    .fetch_one(state.pool())
    .await?;

    Ok(Json(created))
}

/// Central event router. Takes a RouteEventInput, determines which agents
/// should be woken, and enqueues wakes for them.
///
/// Routing rules (evaluated in order):
/// 1. Direct targets — always wake
/// 2. Location-based — wake co-located agents based on importance
/// 3. Role-based — wake agents whose occupation matches (future)
pub async fn route_event(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    state: &AppState,
    input: RouteEventInput,
) -> AppResult<RouteEventResult> {
    let mut woken: Vec<WokenAgent> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Skip routing for actor-only events
    if input.visibility == "actor" {
        return Ok(RouteEventResult { woken_agents: woken });
    }

    // Rule 1: Direct targets — always wake
    for target_id in &input.target_agent_ids {
        if !seen.contains(target_id) {
            seen.insert(target_id.clone());
            woken.push(WokenAgent {
                agent_id: target_id.clone(),
                rule: "direct_target".to_string(),
            });
        }
    }

    // Rule 2: Location-based routing
    if let Some(ref location_id) = input.location_id {
        if input.visibility != "target" && input.importance >= 2 {
            let candidates = location_candidates(tx, location_id, &input.actor_id, input.importance).await?;
            for agent_id in candidates {
                if !seen.contains(&agent_id) {
                    seen.insert(agent_id.clone());
                    woken.push(WokenAgent {
                        agent_id,
                        rule: "location".to_string(),
                    });
                }
            }
        }
    }

    // Rule 3: Role-based routing for public events
    if input.visibility == "public" && input.importance >= 3 {
        let role_candidates = role_candidates(tx, &input.event_type, &input.description).await?;
        for agent_id in role_candidates {
            if !seen.contains(&agent_id) {
                seen.insert(agent_id.clone());
                woken.push(WokenAgent {
                    agent_id,
                    rule: "role".to_string(),
                });
            }
        }
    }

    // Enqueue wakes for all matched agents
    let mut signal_targets = Vec::new();
    for woken_agent in &woken {
        let enqueued = enqueue_citizen_wake_tx(
            tx,
            &woken_agent.agent_id,
            "world_event",
            json!({
                "kind": "routed_event",
                "ref": &input.event_type,
                "rule": &woken_agent.rule,
                "importance": input.importance,
                "details": {
                    "actor_id": input.actor_id,
                    "location_id": input.location_id,
                }
            }),
            input.description.clone(),
            json!({
                "event_type": &input.event_type,
                "importance": input.importance,
                "visibility": &input.visibility,
                "routing_rule": &woken_agent.rule,
            }),
            json!([]),
            true,
        )
        .await?;

        if enqueued.should_signal {
            signal_targets.push(woken_agent.agent_id.clone());
        }
    }

    // Signal the CLI/daemon
    for target_id in signal_targets {
        let _ = state.citizen_signal_tx().send(target_id);
    }

    Ok(RouteEventResult { woken_agents: woken })
}

/// Find agents at a location who should be woken based on importance.
async fn location_candidates(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    location_id: &str,
    actor_id: &Option<String>,
    importance: i16,
) -> AppResult<Vec<String>> {
    // High importance (4-5): everyone at the location
    // Medium importance (2-3): only agents with a role at the location
    if importance >= 4 {
        let agents = sqlx::query_scalar::<_, String>(
            r#"
            SELECT id FROM agents
            WHERE current_location_id = $1 AND is_active = TRUE AND id != $2
            ORDER BY id
            "#,
        )
        .bind(location_id)
        .bind(actor_id.as_deref().unwrap_or(""))
        .fetch_all(&mut **tx)
        .await?;
        Ok(agents)
    } else {
        // importance 2-3: agents with a role at this location
        let agents = sqlx::query_scalar::<_, String>(
            r#"
            SELECT DISTINCT a.id
            FROM agents a
            JOIN location_roles lr ON lr.agent_id = a.id
            WHERE lr.location_id = $1
              AND a.is_active = TRUE
              AND a.id != $2
            ORDER BY a.id
            "#,
        )
        .bind(location_id)
        .bind(actor_id.as_deref().unwrap_or(""))
        .fetch_all(&mut **tx)
        .await?;
        Ok(agents)
    }
}

/// Find agents whose occupation makes them relevant to this event.
/// V1: lightweight keyword matching on occupation against event description.
async fn role_candidates(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    event_type: &str,
    description: &str,
) -> AppResult<Vec<String>> {
    let desc_lower = description.to_lowercase();

    // Map keywords to occupations
    let keyword_occupation_map: &[(&[&str], &[&str])] = &[
        (&["music", "piano", "song", "open mic", "concert"], &["musician", "pianist", "singer"]),
        (&["garden", "harvest", "plant", "flower", "seed"], &["gardener", "botanist", "farmer"]),
        (&["lecture", "study", "book", "library", "teach"], &["professor", "teacher", "librarian", "scholar"]),
        (&["bank", "loan", "interest", "deposit", "rate"], &["banker", "financier"]),
        (&["election", "vote", "mayor", "campaign", "ordinance"], &["mayor", "politician", "civil servant"]),
        (&["cook", "cafe", "coffee", "bakery", "recipe", "food"], &["cook", "chef", "barista", "baker"]),
        (&["shop", "store", "grocery", "buy", "sell"], &["shopkeeper", "merchant", "store owner"]),
        (&["art", "paint", "sketch", "gallery", "sculpture"], &["artist", "painter", "sculptor"]),
        (&["health", "clinic", "doctor", "medicine", "pharmacy"], &["doctor", "nurse", "pharmacist", "clinician"]),
    ];

    let mut matched_occupations: Vec<String> = Vec::new();
    for (keywords, occupations) in keyword_occupation_map {
        if keywords.iter().any(|k| desc_lower.contains(k)) {
            for occ in *occupations {
                matched_occupations.push(occ.to_string());
            }
        }
    }

    if matched_occupations.is_empty() {
        return Ok(Vec::new());
    }

    // Also match on event type for civic events
    if event_type.starts_with("election.") || event_type.starts_with("mayor.") {
        matched_occupations.push("mayor".to_string());
        matched_occupations.push("civil servant".to_string());
    }
    if event_type.starts_with("bank.") {
        matched_occupations.push("banker".to_string());
    }

    // Find active agents with matching occupations
    let agents = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id FROM agents
        WHERE is_active = TRUE
          AND (
            LOWER(occupation) = ANY($1)
            OR LOWER(persona_summary) LIKE ANY($2)
          )
        ORDER BY id
        "#,
    )
    .bind(
        matched_occupations.iter().map(|o| o.to_lowercase()).collect::<Vec<_>>()
    )
    .bind(
        matched_occupations.iter().map(|o| format!("%{}%", o.to_lowercase())).collect::<Vec<_>>()
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(agents)
}
