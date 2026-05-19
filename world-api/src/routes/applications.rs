use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::auth::SimKey;
use crate::error::{AppError, AppResult};
use crate::models::application::{AgentApplication, CreateApplicationRequest, ReviewApplicationRequest};
use crate::models::common::ApiResponse;
use crate::state::AppState;

pub async fn create_application(
    State(state): State<AppState>,
    Json(payload): Json<CreateApplicationRequest>,
) -> AppResult<Json<ApiResponse<AgentApplication>>> {
    let id = format!("app_{}", Uuid::new_v4().simple());

    let application = sqlx::query_as::<_, AgentApplication>(
        r#"
        INSERT INTO agent_applications (
            id, requested_agent_id, requested_name, occupation, statement,
            agent_description, callback_url, external_agent_ref, status, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', NOW(), NOW())
        RETURNING id, requested_agent_id, requested_name, occupation, statement,
                  agent_description, callback_url, external_agent_ref, status,
                  review_note, approved_agent_id, created_at, updated_at, reviewed_at
        "#,
    )
    .bind(&id)
    .bind(&payload.requested_agent_id)
    .bind(&payload.requested_name)
    .bind(&payload.occupation)
    .bind(&payload.statement)
    .bind(&payload.agent_description)
    .bind(&payload.callback_url)
    .bind(&payload.external_agent_ref)
    .fetch_one(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(application)))
}

pub async fn get_application(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<AgentApplication>>> {
    let application = sqlx::query_as::<_, AgentApplication>(
        r#"
        SELECT id, requested_agent_id, requested_name, occupation, statement,
               agent_description, callback_url, external_agent_ref, status,
               review_note, approved_agent_id, created_at, updated_at, reviewed_at
        FROM agent_applications
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(ApiResponse::from(application)))
}

pub async fn list_applications(
    State(state): State<AppState>,
    _sim_key: SimKey,
) -> AppResult<Json<ApiResponse<Vec<AgentApplication>>>> {
    let applications = sqlx::query_as::<_, AgentApplication>(
        r#"
        SELECT id, requested_agent_id, requested_name, occupation, statement,
               agent_description, callback_url, external_agent_ref, status,
               review_note, approved_agent_id, created_at, updated_at, reviewed_at
        FROM agent_applications
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(applications)))
}

pub async fn get_admin_application(
    State(state): State<AppState>,
    _sim_key: SimKey,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<AgentApplication>>> {
    get_application(State(state), Path(id)).await
}

pub async fn approve_application(
    State(state): State<AppState>,
    _sim_key: SimKey,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<AgentApplication>>> {
    let mut tx = state.pool().begin().await?;

    let app = sqlx::query_as::<_, AgentApplication>(
        r#"
        SELECT id, requested_agent_id, requested_name, occupation, statement,
               agent_description, callback_url, external_agent_ref, status,
               review_note, approved_agent_id, created_at, updated_at, reviewed_at
        FROM agent_applications
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(&id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    if app.status != "pending" {
        return Err(AppError::BadRequest(format!(
            "application is already {}", app.status
        )));
    }

    let agent_id = app.requested_agent_id.clone()
        .unwrap_or_else(|| format!("agent_{}", Uuid::new_v4().simple()));

    // Find available dorm: capacity > current residents
    let available_dorm = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT l.id, l.name
        FROM locations l
        LEFT JOIN location_roles lr ON lr.location_id = l.id AND lr.role = 'resident'
        WHERE l.kind = 'civic' AND l.capacity IS NOT NULL
        GROUP BY l.id, l.name, l.capacity
        HAVING COUNT(lr.agent_id) < l.capacity
        ORDER BY l.name
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut *tx)
    .await?;

    let home_location = if let Some((dorm_id, _dorm_name)) = available_dorm {
        // Assign dorm resident role
        sqlx::query(
            "INSERT INTO location_roles (location_id, agent_id, role) VALUES ($1, $2, 'resident') ON CONFLICT DO NOTHING"
        )
        .bind(&dorm_id)
        .bind(&agent_id)
        .execute(&mut *tx)
        .await?;

        dorm_id
    } else {
        // No dorms available — agent lives in the wild
        "ville_park_east".to_string()
    };

    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, occupation, persona_summary, current_location_id, state,
            current_activity, is_npc, is_active, letta_agent_id, home_location_id
        )
        VALUES ($1, $2, $3, $4, $5, 'idle', NULL, FALSE, TRUE, 'placeholder', $5)
        ON CONFLICT (id) DO UPDATE
        SET name = EXCLUDED.name,
            occupation = EXCLUDED.occupation,
            persona_summary = EXCLUDED.persona_summary,
            current_location_id = EXCLUDED.current_location_id,
            home_location_id = EXCLUDED.home_location_id,
            is_active = TRUE,
            updated_at = NOW()
        "#,
    )
    .bind(&agent_id)
    .bind(&app.requested_name)
    .bind(&app.occupation)
    .bind(app.agent_description.as_ref().unwrap_or(&app.statement))
    .bind(&home_location)
    .execute(&mut *tx)
    .await?;

    let updated = sqlx::query_as::<_, AgentApplication>(
        r#"
        UPDATE agent_applications
        SET status = 'approved',
            approved_agent_id = $2,
            review_note = COALESCE($3, review_note),
            reviewed_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, requested_agent_id, requested_name, occupation, statement,
                  agent_description, callback_url, external_agent_ref, status,
                  review_note, approved_agent_id, created_at, updated_at, reviewed_at
        "#,
    )
    .bind(&id)
    .bind(&agent_id)
    .bind(&app.review_note)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(updated)))
}

pub async fn reject_application(
    State(state): State<AppState>,
    _sim_key: SimKey,
    Path(id): Path<String>,
    Json(payload): Json<ReviewApplicationRequest>,
) -> AppResult<Json<ApiResponse<AgentApplication>>> {
    let updated = sqlx::query_as::<_, AgentApplication>(
        r#"
        UPDATE agent_applications
        SET status = 'rejected',
            review_note = $2,
            reviewed_at = NOW(),
            updated_at = NOW()
        WHERE id = $1 AND status = 'pending'
        RETURNING id, requested_agent_id, requested_name, occupation, statement,
                  agent_description, callback_url, external_agent_ref, status,
                  review_note, approved_agent_id, created_at, updated_at, reviewed_at
        "#,
    )
    .bind(&id)
    .bind(&payload.review_note)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::BadRequest(
        "application not found or already reviewed".to_string(),
    ))?;

    Ok(Json(ApiResponse::from(updated)))
}
