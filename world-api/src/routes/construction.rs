use axum::{
    Json,
    extract::State,
};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AgentId;
use crate::error::{AppError, AppResult};
use crate::models::common::{ApiResponse, NotificationMode, NotificationPayload};
use crate::models::construction::{
    ConstructionCompany, ConstructionProject, FundProjectRequest, HireBuilderRequest,
    StartProjectRequest,
};
use crate::state::AppState;

const HOME_COST_CENTS: i64 = 5000;

pub async fn start_home_project(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<StartProjectRequest>,
) -> AppResult<Json<ApiResponse<ConstructionProject>>> {
    let name = payload.location_name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("location_name cannot be empty".to_string()));
    }

    // Check no active project already exists
    let existing = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM construction_projects WHERE agent_id = $1 AND status != 'complete')"#,
    )
    .bind(&agent_id)
    .fetch_one(state.pool())
    .await?;

    if existing {
        return Err(AppError::BadRequest(
            "you already have an active home project".to_string(),
        ));
    }

    let id = format!("proj_{}", Uuid::new_v4().simple());
    let project = sqlx::query_as::<_, ConstructionProject>(
        r#"
        INSERT INTO construction_projects (id, agent_id, location_name, status, cost_cents, funded_cents, progress, created_at, updated_at)
        VALUES ($1, $2, $3, 'planning', $4, 0, 0, NOW(), NOW())
        RETURNING id, agent_id, location_name, status, cost_cents, funded_cents, progress,
                  company_id, last_progress_tick, location_id, created_at, updated_at, completed_at
        "#,
    )
    .bind(&id)
    .bind(&agent_id)
    .bind(name)
    .bind(HOME_COST_CENTS)
    .fetch_one(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(project)))
}

pub async fn fund_home_project(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<FundProjectRequest>,
) -> AppResult<Json<ApiResponse<ConstructionProject>>> {
    if payload.amount_cents <= 0 {
        return Err(AppError::BadRequest("amount must be positive".to_string()));
    }

    let mut tx = state.pool().begin().await?;

    let project = sqlx::query_as::<_, ConstructionProject>(
        r#"
        SELECT id, agent_id, location_name, status, cost_cents, funded_cents, progress,
               company_id, last_progress_tick, location_id, created_at, updated_at, completed_at
        FROM construction_projects
        WHERE agent_id = $1 AND status IN ('planning', 'funding')
        FOR UPDATE
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("no project awaiting funding".to_string()))?;

    // Check agent balance
    let balance = sqlx::query_scalar::<_, i64>(
        r#"SELECT balance_cents FROM agents WHERE id = $1 FOR UPDATE"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if balance < payload.amount_cents {
        return Err(AppError::BadRequest(format!(
            "insufficient funds: you have {}¢ but tried to pay {}¢",
            balance, payload.amount_cents
        )));
    }

    let remaining = project.cost_cents - project.funded_cents;
    let actual_payment = payload.amount_cents.min(remaining);

    // Deduct from agent
    sqlx::query(
        r#"UPDATE agents SET balance_cents = balance_cents - $2, last_expense_cents = $2, last_expense_reason = 'home construction funding', last_expense_at = NOW(), updated_at = NOW() WHERE id = $1"#,
    )
    .bind(&agent_id)
    .bind(actual_payment)
    .execute(&mut *tx)
    .await?;

    let new_funded = project.funded_cents + actual_payment;

    let updated = sqlx::query_as::<_, ConstructionProject>(
        r#"
        UPDATE construction_projects
        SET funded_cents = $2,
            status = CASE WHEN $2 >= cost_cents THEN 'funding' ELSE status END,
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, agent_id, location_name, status, cost_cents, funded_cents, progress,
                  company_id, last_progress_tick, location_id, created_at, updated_at, completed_at
        "#,
    )
    .bind(&project.id)
    .bind(new_funded)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    let msg = if new_funded >= project.cost_cents {
        format!("Fully funded! {}¢ paid. Ready to hire a builder.", actual_payment)
    } else {
        format!("Paid {}¢. Funded: {}¢ / {}¢.", actual_payment, new_funded, project.cost_cents)
    };

    Ok(Json(ApiResponse::from(updated).with_notification(NotificationPayload {
        message: msg,
        mode: NotificationMode::Instant,
        eta_seconds: None,
    })))
}

pub async fn hire_builder(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<HireBuilderRequest>,
) -> AppResult<Json<ApiResponse<ConstructionProject>>> {
    let mut tx = state.pool().begin().await?;

    let project = sqlx::query_as::<_, ConstructionProject>(
        r#"
        SELECT id, agent_id, location_name, status, cost_cents, funded_cents, progress,
               company_id, last_progress_tick, location_id, created_at, updated_at, completed_at
        FROM construction_projects
        WHERE agent_id = $1 AND status = 'funding'
        FOR UPDATE
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("no fully-funded project ready for a builder".to_string()))?;

    if project.company_id.is_some() {
        return Err(AppError::BadRequest("builder already hired".to_string()));
    }

    // Look up company
    let company = sqlx::query_as::<_, ConstructionCompany>(
        r#"SELECT id, name, progress_per_sim_hour, hiring_fee_cents, is_active, created_at FROM construction_companies WHERE id = $1 AND is_active = TRUE"#,
    )
    .bind(&payload.company_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("construction company not found or inactive".to_string()))?;

    // Check agent can afford hiring fee
    let balance = sqlx::query_scalar::<_, i64>(
        r#"SELECT balance_cents FROM agents WHERE id = $1 FOR UPDATE"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if balance < company.hiring_fee_cents {
        return Err(AppError::BadRequest(format!(
            "insufficient funds for hiring fee: need {}¢, have {}¢",
            company.hiring_fee_cents, balance
        )));
    }

    // Deduct hiring fee
    sqlx::query(
        r#"UPDATE agents SET balance_cents = balance_cents - $2, last_expense_cents = $2, last_expense_reason = 'builder hiring fee', last_expense_at = NOW(), updated_at = NOW() WHERE id = $1"#,
    )
    .bind(&agent_id)
    .bind(company.hiring_fee_cents)
    .execute(&mut *tx)
    .await?;

    let updated = sqlx::query_as::<_, ConstructionProject>(
        r#"
        UPDATE construction_projects
        SET company_id = $2,
            status = 'building',
            last_progress_tick = NOW(),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, agent_id, location_name, status, cost_cents, funded_cents, progress,
                  company_id, last_progress_tick, location_id, created_at, updated_at, completed_at
        "#,
    )
    .bind(&project.id)
    .bind(&company.id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(updated).with_notification(NotificationPayload {
        message: format!("{} hired! Building has begun. Check progress with check_home_project.", company.name),
        mode: NotificationMode::Instant,
        eta_seconds: None,
    })))
}

pub async fn check_home_project(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let mut tx = state.pool().begin().await?;

    let project = sqlx::query_as::<_, ConstructionProject>(
        r#"
        SELECT id, agent_id, location_name, status, cost_cents, funded_cents, progress,
               company_id, last_progress_tick, location_id, created_at, updated_at, completed_at
        FROM construction_projects
        WHERE agent_id = $1 AND status != 'complete'
        FOR UPDATE
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("no active home project".to_string()))?;

    // If building, advance progress based on sim-time elapsed
    let mut project = project;
    if project.status == "building" && project.company_id.is_some() && project.last_progress_tick.is_some() {
        let company = sqlx::query_as::<_, ConstructionCompany>(
            r#"SELECT id, name, progress_per_sim_hour, hiring_fee_cents, is_active, created_at FROM construction_companies WHERE id = $1"#,
        )
        .bind(project.company_id.as_ref().unwrap())
        .fetch_one(&mut *tx)
        .await?;

        // Compute sim-time elapsed since last tick
        let (_sim_time, time_scale, _paused, _epoch_start) = crate::routes::world::compute_sim_time(state.pool()).await;
        let last_tick = project.last_progress_tick.unwrap();

        // Sim-time elapsed = (wall_elapsed * time_scale) in hours
        let wall_elapsed_secs = (Utc::now() - last_tick).num_seconds().max(0) as f64;
        let sim_elapsed_hours = (wall_elapsed_secs * time_scale) / 3600.0;

        let progress_gain = (sim_elapsed_hours * company.progress_per_sim_hour as f64).round() as i32;
        let new_progress = (project.progress + progress_gain).min(100);

        if new_progress >= 100 {
            // Complete the project!
            let location_id = format!("home_{}", Uuid::new_v4().simple());

            // Create the home location
            sqlx::query(
                r#"INSERT INTO locations (id, name, description, map_x, map_y, kind) VALUES ($1, $2, $3, 544, 416, 'home')"#,
            )
            .bind(&location_id)
            .bind(&project.location_name)
            .bind(format!("{}'s home, built with love and hard-earned money.", project.location_name))
            .execute(&mut *tx)
            .await?;

            // Create a bed object at the home
            let bed_id = format!("bed_{}", Uuid::new_v4().simple());
            sqlx::query(
                r#"INSERT INTO world_objects (id, name, location_id, state, actions) VALUES ($1, 'Bed', $2, '{}'::jsonb, ARRAY['sleep'])"#,
            )
            .bind(&bed_id)
            .bind(&location_id)
            .execute(&mut *tx)
            .await?;

            // Assign owner + resident roles
            sqlx::query(
                r#"INSERT INTO location_roles (location_id, agent_id, role) VALUES ($1, $2, 'owner'), ($1, $2, 'resident') ON CONFLICT DO NOTHING"#,
            )
            .bind(&location_id)
            .bind(&agent_id)
            .execute(&mut *tx)
            .await?;

            // Update agent home_location_id
            sqlx::query(
                r#"UPDATE agents SET home_location_id = $2, updated_at = NOW() WHERE id = $1"#,
            )
            .bind(&agent_id)
            .bind(&location_id)
            .execute(&mut *tx)
            .await?;

            // Mark project complete
            project = sqlx::query_as::<_, ConstructionProject>(
                r#"
                UPDATE construction_projects
                SET progress = 100,
                    status = 'complete',
                    location_id = $2,
                    completed_at = NOW(),
                    updated_at = NOW()
                WHERE id = $1
                RETURNING id, agent_id, location_name, status, cost_cents, funded_cents, progress,
                          company_id, last_progress_tick, location_id, created_at, updated_at, completed_at
                "#,
            )
            .bind(&project.id)
            .bind(&location_id)
            .fetch_one(&mut *tx)
            .await?;

            tx.commit().await?;

            return Ok(Json(ApiResponse::from(json!({
                "project": project,
                "completed": true,
                "home_location_id": location_id,
                "message": "Your home is complete! You can move in and enjoy better sleep recovery."
            })).with_notification(NotificationPayload {
                message: "Your home is complete! Move in and enjoy better sleep.".to_string(),
                mode: NotificationMode::Instant,
                eta_seconds: None,
            })));
        } else {
            // Update progress
            project = sqlx::query_as::<_, ConstructionProject>(
                r#"
                UPDATE construction_projects
                SET progress = $2,
                    last_progress_tick = NOW(),
                    updated_at = NOW()
                WHERE id = $1
                RETURNING id, agent_id, location_name, status, cost_cents, funded_cents, progress,
                          company_id, last_progress_tick, location_id, created_at, updated_at, completed_at
                "#,
            )
            .bind(&project.id)
            .bind(new_progress)
            .fetch_one(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;

    let company_name = if let Some(ref cid) = project.company_id {
        sqlx::query_scalar::<_, String>(
            r#"SELECT name FROM construction_companies WHERE id = $1"#,
        )
        .bind(cid)
        .fetch_optional(state.pool())
        .await?
        .unwrap_or_else(|| "Unknown".to_string())
    } else {
        "Not hired yet".to_string()
    };

    Ok(Json(ApiResponse::from(json!({
        "project": project,
        "builder": company_name,
        "progress_percent": project.progress,
        "remaining_cost": (project.cost_cents - project.funded_cents).max(0),
    }))))
}

pub async fn list_construction_companies(
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<Vec<ConstructionCompany>>>> {
    let companies = sqlx::query_as::<_, ConstructionCompany>(
        r#"SELECT id, name, progress_per_sim_hour, hiring_fee_cents, is_active, created_at FROM construction_companies WHERE is_active = TRUE ORDER BY name"#,
    )
    .fetch_all(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(companies)))
}
