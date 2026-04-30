use axum::{Json, extract::State};
use chrono::{Local, Timelike, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::error::AppResult;
use crate::models::board::BoardPost;
use crate::models::common::ApiResponse;
use crate::models::event::SimEvent;
use crate::models::world::WorldTimeResponse;
use crate::state::AppState;

const BOARD_OBJECT_ID: &str = "notice_board_main";

#[derive(Debug, Serialize)]
pub struct TownPulseResponse {
    pub world_time: WorldTimeResponse,
    pub headline: String,
    pub highlights: Vec<String>,
    pub active_agents: Vec<PulseAgent>,
    pub board_posts: Vec<BoardPost>,
    pub recent_events: Vec<SimEvent>,
    pub busy_locations: Vec<BusyLocation>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct PulseAgent {
    pub agent_id: String,
    pub name: String,
    pub occupation: String,
    pub current_location_id: String,
    pub location_name: String,
    pub state: String,
    pub current_activity: Option<String>,
    pub intention_summary: Option<String>,
    pub intention_reason: Option<String>,
    pub expected_location_id: Option<String>,
    pub primary_job_id: Option<String>,
    pub primary_job_name: Option<String>,
    pub primary_job_kind: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct BusyLocation {
    pub location_id: String,
    pub name: String,
    pub agent_count: i64,
    pub recent_event_count: i64,
}

#[derive(Debug, Deserialize, Default)]
struct NoticeBoardState {
    posts: Vec<BoardPost>,
}

pub async fn get_town_pulse(
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<TownPulseResponse>>> {
    let world_time = load_world_time(&state).await?;
    let active_agents = load_pulse_agents(&state).await?;
    let board_posts = load_board_posts(&state).await?;
    let recent_events = load_recent_events(&state, 10).await?;
    let busy_locations = load_busy_locations(&state).await?;
    let headline = build_headline(&active_agents, &board_posts, &busy_locations, &world_time);
    let highlights = build_highlights(
        &active_agents,
        &board_posts,
        &recent_events,
        &busy_locations,
    );

    Ok(Json(ApiResponse::from(TownPulseResponse {
        world_time,
        headline,
        highlights,
        active_agents,
        board_posts,
        recent_events,
        busy_locations,
    })))
}

async fn load_world_time(state: &AppState) -> AppResult<WorldTimeResponse> {
    let paused = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT COALESCE((value->>'simulation_paused')::boolean, false)
        FROM simulation_state
        WHERE key = 'world'
        LIMIT 1
        "#,
    )
    .fetch_optional(state.pool())
    .await?
    .unwrap_or(false);

    let now_utc = Utc::now();
    let hour = Local::now().hour();
    let time_of_day = match hour {
        5..=10 => "morning",
        11..=16 => "afternoon",
        17..=20 => "evening",
        _ => "night",
    }
    .to_string();

    Ok(WorldTimeResponse {
        timestamp: now_utc.to_rfc3339(),
        time_of_day,
        simulation_paused: paused,
    })
}

async fn load_pulse_agents(state: &AppState) -> AppResult<Vec<PulseAgent>> {
    let agents = sqlx::query_as::<_, PulseAgent>(
        r#"
        SELECT a.id AS agent_id,
               a.name,
               a.occupation,
               a.current_location_id,
               l.name AS location_name,
               a.state,
               a.current_activity,
               ai.summary AS intention_summary,
               ai.reason AS intention_reason,
               ai.expected_location_id,
               job.job_id AS primary_job_id,
               job.name AS primary_job_name,
               job.kind AS primary_job_kind
        FROM agents a
        INNER JOIN locations l ON l.id = a.current_location_id
        LEFT JOIN agent_intentions ai ON ai.agent_id = a.id AND ai.status = 'active'
        LEFT JOIN LATERAL (
            SELECT j.id AS job_id, j.name, j.kind
            FROM agent_jobs aj
            INNER JOIN jobs j ON j.id = aj.job_id
            WHERE aj.agent_id = a.id
            ORDER BY aj.is_primary DESC, j.kind, j.name
            LIMIT 1
        ) job ON true
        WHERE a.is_active = TRUE
        ORDER BY CASE WHEN ai.id IS NULL THEN 1 ELSE 0 END, a.name
        "#,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(agents)
}

async fn load_board_posts(state: &AppState) -> AppResult<Vec<BoardPost>> {
    let board_state = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT state
        FROM world_objects
        WHERE id = $1
        "#,
    )
    .bind(BOARD_OBJECT_ID)
    .fetch_optional(state.pool())
    .await?
    .unwrap_or_else(|| serde_json::json!({ "posts": [] }));

    let mut posts = serde_json::from_value::<NoticeBoardState>(board_state)
        .unwrap_or_default()
        .posts;
    posts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    posts.truncate(5);

    Ok(posts)
}

async fn load_recent_events(state: &AppState, limit: i64) -> AppResult<Vec<SimEvent>> {
    let events = sqlx::query_as::<_, SimEvent>(
        r#"
        SELECT id, occurred_at, type, actor_id, location_id, description, metadata
        FROM events
        ORDER BY occurred_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(state.pool())
    .await?;

    Ok(events)
}

async fn load_busy_locations(state: &AppState) -> AppResult<Vec<BusyLocation>> {
    let locations = sqlx::query_as::<_, BusyLocation>(
        r#"
        SELECT l.id AS location_id,
               l.name,
               COUNT(DISTINCT a.id)::bigint AS agent_count,
               COUNT(DISTINCT e.id)::bigint AS recent_event_count
        FROM locations l
        LEFT JOIN agents a ON a.current_location_id = l.id AND a.is_active = TRUE
        LEFT JOIN events e ON e.location_id = l.id AND e.occurred_at >= NOW() - INTERVAL '2 hours'
        GROUP BY l.id, l.name
        HAVING COUNT(DISTINCT a.id) > 0 OR COUNT(e.id) > 0
        ORDER BY recent_event_count DESC, agent_count DESC, l.name
        LIMIT 5
        "#,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(locations)
}

fn build_headline(
    agents: &[PulseAgent],
    board_posts: &[BoardPost],
    busy_locations: &[BusyLocation],
    world_time: &WorldTimeResponse,
) -> String {
    let intention_count = agents
        .iter()
        .filter(|agent| agent.intention_summary.is_some())
        .count();
    let active_count = agents.len();

    if intention_count > 0 && !board_posts.is_empty() {
        return format!(
            "{} {} carrying intentions, with {} notice board {} shaping the {}.",
            intention_count,
            plural(intention_count, "agent is", "agents are"),
            board_posts.len(),
            plural(board_posts.len(), "post", "posts"),
            world_time.time_of_day
        );
    }

    if intention_count > 0 {
        return format!(
            "{} {} acting with visible intentions this {}.",
            intention_count,
            plural(intention_count, "agent is", "agents are"),
            world_time.time_of_day
        );
    }

    if !busy_locations.is_empty() {
        return format!(
            "{} {} active across {} busy {} this {}.",
            active_count,
            plural(active_count, "agent is", "agents are"),
            busy_locations.len(),
            plural(busy_locations.len(), "place", "places"),
            world_time.time_of_day
        );
    }

    format!("Smallville is quiet this {}.", world_time.time_of_day)
}

fn build_highlights(
    agents: &[PulseAgent],
    board_posts: &[BoardPost],
    recent_events: &[SimEvent],
    busy_locations: &[BusyLocation],
) -> Vec<String> {
    let mut highlights = Vec::new();

    for agent in agents
        .iter()
        .filter(|agent| agent.intention_summary.is_some())
        .take(4)
    {
        let summary = agent.intention_summary.as_deref().unwrap_or_default();
        highlights.push(format!(
            "{} is at {} working on: {}.",
            agent.name, agent.location_name, summary
        ));
    }

    for post in board_posts.iter().take(2) {
        highlights.push(format!("Notice board: {}", post.text));
    }

    if highlights.len() < 4 {
        for event in recent_events.iter().take(4 - highlights.len()) {
            highlights.push(event.description.clone());
        }
    }

    if highlights.is_empty() {
        for location in busy_locations.iter().take(3) {
            highlights.push(format!(
                "{} has {} {} and {} recent {}.",
                location.name,
                location.agent_count,
                plural_count(location.agent_count, "agent", "agents"),
                location.recent_event_count,
                plural_count(location.recent_event_count, "event", "events")
            ));
        }
    }

    if highlights.is_empty() {
        highlights.push("No active public activity yet. The city is waiting for an agent to do something interesting.".to_string());
    }

    highlights.truncate(6);
    highlights
}

fn plural(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        singular.to_string()
    } else {
        plural.to_string()
    }
}

fn plural_count(count: i64, singular: &str, plural: &str) -> String {
    if count == 1 {
        singular.to_string()
    } else {
        plural.to_string()
    }
}
