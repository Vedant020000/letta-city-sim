use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::models::common::ApiResponse;
use crate::models::job::{AssignedJob, Job, JobAgent, UpsertAgentJobRequest};
use crate::state::AppState;
use crate::ws_events::WorldEventEnvelope;

#[derive(Debug, Deserialize)]
pub struct AgentJobPath {
    pub id: String,
    pub job_id: String,
}

pub async fn list_jobs(State(state): State<AppState>) -> AppResult<Json<ApiResponse<Vec<Job>>>> {
    let jobs = sqlx::query_as::<_, Job>(
        r#"
        SELECT id, name, kind, summary, metadata, created_at, updated_at
        FROM jobs
        ORDER BY kind, name
        "#,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(jobs)))
}

pub async fn get_job_by_id(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> AppResult<Json<ApiResponse<Job>>> {
    let job = sqlx::query_as::<_, Job>(
        r#"
        SELECT id, name, kind, summary, metadata, created_at, updated_at
        FROM jobs
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(ApiResponse::from(job)))
}

pub async fn list_job_agents(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> AppResult<Json<ApiResponse<Vec<JobAgent>>>> {
    ensure_job_exists(state.pool(), &job_id).await?;

    let agents = sqlx::query_as::<_, JobAgent>(
        r#"
        SELECT a.id AS agent_id,
               a.name AS agent_name,
               a.occupation,
               a.current_location_id,
               a.state,
               aj.is_primary,
               aj.notes,
               aj.metadata AS assignment_metadata,
               aj.assigned_at,
               aj.updated_at
        FROM agent_jobs aj
        INNER JOIN agents a ON a.id = aj.agent_id
        WHERE aj.job_id = $1
        ORDER BY aj.is_primary DESC, a.name
        "#,
    )
    .bind(job_id)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(agents)))
}

pub async fn list_agent_jobs(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> AppResult<Json<ApiResponse<Vec<AssignedJob>>>> {
    ensure_agent_exists(state.pool(), &agent_id).await?;

    let jobs = sqlx::query_as::<_, AssignedJob>(
        r#"
        SELECT j.id AS job_id,
               j.name,
               j.kind,
               j.summary,
               j.metadata AS job_metadata,
               aj.is_primary,
               aj.notes,
               aj.metadata AS assignment_metadata,
               aj.assigned_at,
               aj.updated_at
        FROM agent_jobs aj
        INNER JOIN jobs j ON j.id = aj.job_id
        WHERE aj.agent_id = $1
        ORDER BY aj.is_primary DESC, j.kind, j.name
        "#,
    )
    .bind(agent_id)
    .fetch_all(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(jobs)))
}

pub async fn upsert_agent_job(
    State(state): State<AppState>,
    Path(path): Path<AgentJobPath>,
    Json(payload): Json<UpsertAgentJobRequest>,
) -> AppResult<Json<ApiResponse<AssignedJob>>> {
    let mut tx = state.pool().begin().await?;
    let current_location_id = ensure_agent_exists_for_update(&mut tx, &path.id).await?;
    ensure_job_exists_for_update(&mut tx, &path.job_id).await?;

    let existing = load_assigned_job_for_update(&mut tx, &path.id, &path.job_id).await?;
    let is_primary = payload
        .is_primary
        .unwrap_or_else(|| existing.as_ref().map(|job| job.is_primary).unwrap_or(false));
    let notes = match payload.notes {
        Some(value) => normalize_optional_text(Some(value)),
        None => existing.as_ref().and_then(|job| job.notes.clone()),
    };
    let metadata = payload.metadata.unwrap_or_else(|| {
        existing
            .as_ref()
            .map(|job| job.assignment_metadata.clone())
            .unwrap_or_else(|| serde_json::json!({}))
    });

    if is_primary {
        sqlx::query(
            r#"
            UPDATE agent_jobs
            SET is_primary = FALSE,
                updated_at = NOW()
            WHERE agent_id = $1 AND job_id <> $2 AND is_primary = TRUE
            "#,
        )
        .bind(&path.id)
        .bind(&path.job_id)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO agent_jobs (agent_id, job_id, is_primary, notes, metadata, assigned_at, updated_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, NOW(), NOW())
        ON CONFLICT (agent_id, job_id) DO UPDATE
        SET is_primary = EXCLUDED.is_primary,
            notes = EXCLUDED.notes,
            metadata = EXCLUDED.metadata,
            updated_at = NOW()
        "#,
    )
    .bind(&path.id)
    .bind(&path.job_id)
    .bind(is_primary)
    .bind(notes.clone())
    .bind(metadata.to_string())
    .execute(&mut *tx)
    .await?;

    let assigned = load_assigned_job_for_update(&mut tx, &path.id, &path.job_id)
        .await?
        .ok_or_else(|| {
            AppError::Unexpected("agent job assignment missing after upsert".to_string())
        })?;

    insert_job_event(
        &mut tx,
        "agent.job.assigned",
        &path.id,
        &current_location_id,
        &assigned,
    )
    .await?;
    tx.commit().await?;

    broadcast_job_event(
        &state,
        "agent.job.assigned",
        &path.id,
        current_location_id,
        &assigned,
    );

    Ok(Json(ApiResponse::from(assigned)))
}

pub async fn remove_agent_job(
    State(state): State<AppState>,
    Path(path): Path<AgentJobPath>,
) -> AppResult<Json<ApiResponse<AssignedJob>>> {
    let mut tx = state.pool().begin().await?;
    let current_location_id = ensure_agent_exists_for_update(&mut tx, &path.id).await?;

    let assigned = load_assigned_job_for_update(&mut tx, &path.id, &path.job_id)
        .await?
        .ok_or(AppError::NotFound)?;

    sqlx::query(
        r#"
        DELETE FROM agent_jobs
        WHERE agent_id = $1 AND job_id = $2
        "#,
    )
    .bind(&path.id)
    .bind(&path.job_id)
    .execute(&mut *tx)
    .await?;

    insert_job_event(
        &mut tx,
        "agent.job.unassigned",
        &path.id,
        &current_location_id,
        &assigned,
    )
    .await?;
    tx.commit().await?;

    broadcast_job_event(
        &state,
        "agent.job.unassigned",
        &path.id,
        current_location_id,
        &assigned,
    );

    Ok(Json(ApiResponse::from(assigned)))
}

async fn ensure_job_exists(pool: &sqlx::Pool<sqlx::Postgres>, job_id: &str) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM jobs
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .fetch_optional(pool)
    .await?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    Ok(())
}

async fn ensure_agent_exists(pool: &sqlx::Pool<sqlx::Postgres>, agent_id: &str) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM agents
        WHERE id = $1
        "#,
    )
    .bind(agent_id)
    .fetch_optional(pool)
    .await?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    Ok(())
}

async fn ensure_job_exists_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job_id: &str,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM jobs
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(job_id)
    .fetch_optional(&mut **tx)
    .await?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    Ok(())
}

async fn ensure_agent_exists_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    agent_id: &str,
) -> AppResult<String> {
    let current_location_id = sqlx::query_scalar::<_, String>(
        r#"
        SELECT current_location_id
        FROM agents
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(agent_id)
    .fetch_optional(&mut **tx)
    .await?;

    current_location_id.ok_or(AppError::NotFound)
}

async fn load_assigned_job_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    agent_id: &str,
    job_id: &str,
) -> AppResult<Option<AssignedJob>> {
    let job = sqlx::query_as::<_, AssignedJob>(
        r#"
        SELECT j.id AS job_id,
               j.name,
               j.kind,
               j.summary,
               j.metadata AS job_metadata,
               aj.is_primary,
               aj.notes,
               aj.metadata AS assignment_metadata,
               aj.assigned_at,
               aj.updated_at
        FROM agent_jobs aj
        INNER JOIN jobs j ON j.id = aj.job_id
        WHERE aj.agent_id = $1 AND aj.job_id = $2
        FOR UPDATE OF aj
        "#,
    )
    .bind(agent_id)
    .bind(job_id)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(job)
}

async fn insert_job_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_type: &str,
    agent_id: &str,
    current_location_id: &str,
    assigned: &AssignedJob,
) -> AppResult<()> {
    let verb = if event_type == "agent.job.unassigned" {
        "removed from"
    } else {
        "assigned to"
    };
    let description = format!(
        "Agent {agent_id} was {verb} job {}",
        assigned.job_id.as_str()
    );
    let metadata = serde_json::json!({
        "agent_id": agent_id,
        "job_id": assigned.job_id.as_str(),
        "job_name": assigned.name.as_str(),
        "job_kind": assigned.kind.as_str(),
        "is_primary": assigned.is_primary,
        "notes": assigned.notes.clone(),
        "assignment_metadata": assigned.assignment_metadata.clone(),
    });

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind(event_type)
    .bind(agent_id)
    .bind(current_location_id)
    .bind(description)
    .bind(metadata.to_string())
    .bind(Utc::now())
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn broadcast_job_event(
    state: &AppState,
    event_type: &str,
    agent_id: &str,
    current_location_id: String,
    assigned: &AssignedJob,
) {
    let _ = state.event_tx().send(WorldEventEnvelope::new(
        event_type,
        vec![agent_id.to_string()],
        Some(current_location_id),
        serde_json::json!({
            "agent_id": agent_id,
            "job_id": assigned.job_id.as_str(),
            "job_name": assigned.name.as_str(),
            "job_kind": assigned.kind.as_str(),
            "is_primary": assigned.is_primary,
        }),
    ));
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}
