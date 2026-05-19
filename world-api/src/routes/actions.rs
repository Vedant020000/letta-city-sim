use axum::{
    Json,
    extract::{Path, State},
};
use chrono::{Timelike, Utc};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::auth::AgentId;
use crate::error::{AppError, AppResult};
use crate::models::agent::Agent;
use crate::models::common::ApiResponse;
use crate::models::inventory::InventoryItem;
use crate::models::location::{Location, AdjacentLocation};
use crate::models::object::WorldObject;
use crate::routes::agents::{
    UpdateAgentLocationRequest, perform_agent_activity_update, perform_agent_activity_update_in_tx,
    perform_agent_location_update,
};
use crate::routes::board::{CreateBoardPostRequest, create_board_post};
use crate::routes::conversations::{
    AcceptInviteRequest, AcceptRequestRequest, JoinConversationRequest, SendMessageRequest,
    SpeakToRequest,
};
use crate::routes::inventory::{
    add_item_to_agent_inventory, get_agent_inventory, remove_item_from_agent_inventory,
    transfer_item_between_agents, use_item, AddInventoryItemRequest, RemoveInventoryItemRequest,
    TransferItemRequest, UseItemRequest,
};
use crate::routes::economy::{
    pay_agent, request_money, respond_money_request, get_transaction_log,
    PayAgentRequest, RequestMoneyRequest, RespondMoneyRequestRequest, GetTransactionLogRequest,
};
use crate::routes::sleep::start_sleep;
use crate::models::intention::{CreateAgentIntentionRequest, UpdateAgentIntentionRequest, AgentIntention};
use crate::state::AppState;
use sqlx::Row;

#[derive(Debug, Deserialize)]
pub struct SetActivityActionRequest {
    pub activity: String,
}

#[derive(Debug, Deserialize)]
pub struct CookFoodActionRequest {
    pub recipe_id: String,
    pub quantity: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct CookFoodActionResponse {
    pub agent: Agent,
    pub recipe_id: String,
    pub quantity: i32,
    pub placeholder: bool,
    pub message: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LookAroundAgent {
    pub id: String,
    pub name: String,
    pub state: String,
    pub current_activity: Option<String>,
    pub hygiene_level: i16,
    pub appearance_level: i16,
}

#[derive(Debug, Serialize)]
pub struct LookAroundResponse {
    pub location: Location,
    pub nearby: Vec<AdjacentLocation>,
    pub objects: Vec<WorldObject>,
    pub agents_present: Vec<LookAroundAgent>,
    pub items_on_ground: Vec<InventoryItem>,
    pub time_of_day: String,
    pub hour: u32,
}

#[derive(Debug, Serialize)]
pub struct ToolManifestResponse {
    pub agent_id: String,
    pub location_id: String,
    pub location_name: String,
    pub context: ToolManifestContext,
    pub tools: Vec<WorldToolDefinition>,
}

#[derive(Debug, Serialize)]
pub struct ToolManifestContext {
    pub nearby_location_ids: Vec<String>,
    pub object_ids: Vec<String>,
    pub object_action_tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct WorldToolDefinition {
    pub name: String,
    pub description: String,
    pub endpoint: String,
    pub method: String,
    pub parameters: Value,
}

pub async fn action_set_activity(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<SetActivityActionRequest>,
) -> AppResult<Json<ApiResponse<Agent>>> {
    let activity = payload.activity.trim();
    if activity.is_empty() {
        return Err(AppError::BadRequest(
            "activity cannot be empty".to_string(),
        ));
    }

    let updated = perform_agent_activity_update(&state, &agent_id, activity).await?;
    Ok(Json(ApiResponse::from(updated)))
}

pub async fn action_move_to(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<UpdateAgentLocationRequest>,
) -> AppResult<Json<ApiResponse<Agent>>> {
    // Apply vitals decay, check stamina, and deduct — all in one transaction
    let mut tx = state.pool().begin().await?;
    let agent = crate::routes::vitals::apply_vitals_decay_tx(&mut tx, &agent_id).await?;

    if agent.stamina_level < crate::routes::vitals::MOVE_STAMINA_COST {
        // Roll back the decay transaction (decay is harmless to lose — it'll be reapplied next time)
        return Err(AppError::BadRequest(
            format!("not enough stamina to move (have {}, need {})", agent.stamina_level, crate::routes::vitals::MOVE_STAMINA_COST),
        ));
    }

    // Deduct stamina with a WHERE guard to prevent going below 0
    let rows = sqlx::query(
        r#"
        UPDATE agents
        SET stamina_level = stamina_level - $1,
            updated_at = NOW()
        WHERE id = $2 AND stamina_level >= $1
        "#,
    )
    .bind(crate::routes::vitals::MOVE_STAMINA_COST)
    .bind(&agent_id)
    .execute(&mut *tx)
    .await?;

    if rows.rows_affected() == 0 {
        return Err(AppError::BadRequest(
            format!("not enough stamina to move (have {}, need {})", agent.stamina_level, crate::routes::vitals::MOVE_STAMINA_COST),
        ));
    }

    tx.commit().await?;



    let response = perform_agent_location_update(&state, &agent_id, &payload.location_id).await?;
    Ok(Json(response))
}

pub async fn action_board_post(
    state: State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<CreateBoardPostRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let Json(post) = create_board_post(state, AgentId(agent_id), Json(payload)).await?;
    Ok(Json(json!({
        "ok": true,
        "post": post,
    })))
}

pub async fn action_sleep(
    state: State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<Agent>>> {
    start_sleep(state, AgentId(agent_id)).await
}

pub async fn action_wake_up(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<Agent>>> {
    crate::routes::sleep::wake_up(State(state), AgentId(agent_id)).await
}

pub async fn action_cook_food(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<CookFoodActionRequest>,
) -> AppResult<Json<ApiResponse<CookFoodActionResponse>>> {
    let recipe_id = payload.recipe_id.trim();
    if recipe_id.is_empty() {
        return Err(AppError::BadRequest(
            "recipe_id cannot be empty".to_string(),
        ));
    }

    let quantity = payload.quantity.unwrap_or(1);
    if quantity <= 0 {
        return Err(AppError::BadRequest(
            "quantity must be greater than 0".to_string(),
        ));
    }

    let mut tx = state.pool().begin().await?;

    let updated_agent = perform_agent_activity_update_in_tx(
        &mut tx,
        &agent_id,
        &format!("Cooking {}", recipe_id),
    )
    .await?;

    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.cook.started")
    .bind(&updated_agent.id)
    .bind(&updated_agent.current_location_id)
    .bind(format!(
        "Agent {} started cooking {}",
        updated_agent.id, recipe_id
    ))
    .bind(
        json!({
            "recipe_id": recipe_id,
            "quantity": quantity,
            "placeholder": true,
        })
        .to_string(),
    )
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(CookFoodActionResponse {
        agent: updated_agent,
        recipe_id: recipe_id.to_string(),
        quantity,
        placeholder: true,
        message: "Cook food is currently a server-owned placeholder action that marks the agent as cooking."
            .to_string(),
    })))
}

pub async fn action_look_around(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<LookAroundResponse>>> {
    let agent_row = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT a.current_location_id, l.name
        FROM agents a
        JOIN locations l ON l.id = a.current_location_id
        WHERE a.id = $1 OR a.letta_agent_id = $1
        LIMIT 1
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    let location = sqlx::query_as::<_, Location>(
        r#"
        SELECT id, name, description, map_x, map_y
        FROM locations
        WHERE id = $1
        "#,
    )
    .bind(&agent_row.0)
    .fetch_one(state.pool())
    .await?;

    let nearby = sqlx::query_as::<_, AdjacentLocation>(
        r#"
        SELECT l.id, l.name, l.description, l.map_x, l.map_y, la.travel_secs
        FROM location_adjacency la
        JOIN locations l ON l.id = la.to_id
        WHERE la.from_id = $1
        ORDER BY l.id
        "#,
    )
    .bind(&agent_row.0)
    .fetch_all(state.pool())
    .await?;

    let objects = sqlx::query_as::<_, WorldObject>(
        r#"
        SELECT id, name, location_id, state, actions
        FROM world_objects
        WHERE location_id = $1
        ORDER BY name
        "#,
    )
    .bind(&agent_row.0)
    .fetch_all(state.pool())
    .await?;

    let agents_present = sqlx::query_as::<_, LookAroundAgent>(
        r#"
        SELECT id, name, state, current_activity, hygiene_level, appearance_level
        FROM agents
        WHERE current_location_id = $1
          AND id != $2
        ORDER BY name
        "#,
    )
    .bind(&agent_row.0)
    .bind(&agent_id)
    .fetch_all(state.pool())
    .await?;

    let items_on_ground = sqlx::query_as::<_, InventoryItem>(
        r#"
        SELECT id, name, held_by, location_id, state, quantity, consumable_type, vital_value, price_cents
        FROM inventory_items
        WHERE location_id = $1
        ORDER BY name
        "#,
    )
    .bind(&agent_row.0)
    .fetch_all(state.pool())
    .await?;

    // Add time context
    let (sim_time, _, _, _) = crate::routes::world::compute_sim_time(state.pool()).await;
    let hour = sim_time.hour();
    let time_of_day = crate::routes::world::time_of_day_from_hour(hour).to_string();

    Ok(Json(ApiResponse::from(LookAroundResponse {
        location,
        nearby,
        objects,
        agents_present,
        items_on_ground,
        time_of_day,
        hour,
    })))
}

pub async fn action_speak_to(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<SpeakToRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    crate::routes::conversations::action_speak_to(State(state), AgentId(agent_id), Json(payload)).await
}

pub async fn action_join_conversation(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<JoinConversationRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    crate::routes::conversations::action_join_conversation(State(state), AgentId(agent_id), Json(payload)).await
}

pub async fn action_leave_conversation(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<JoinConversationRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    crate::routes::conversations::action_leave_conversation(State(state), AgentId(agent_id), Json(payload)).await
}

pub async fn action_send_message(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<SendMessageRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    crate::routes::conversations::action_send_message(State(state), AgentId(agent_id), Json(payload)).await
}

pub async fn action_accept_join_request(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<AcceptRequestRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    crate::routes::conversations::action_accept_join_request(State(state), AgentId(agent_id), Json(payload)).await
}

pub async fn action_accept_invitation(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<AcceptInviteRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    crate::routes::conversations::action_accept_invitation(State(state), AgentId(agent_id), Json(payload)).await
}

// ---------------------------------------------------------------------------
// Inventory + economy action handlers
// ---------------------------------------------------------------------------

pub async fn action_get_inventory(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<Vec<InventoryItem>>> {
    get_agent_inventory(State(state), Path(agent_id)).await
}

pub async fn action_pick_up_item(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<AddInventoryItemRequest>,
) -> AppResult<Json<InventoryItem>> {
    let auth = crate::auth::AuthContext::agent(agent_id.clone());
    add_item_to_agent_inventory(State(state), auth, Path(agent_id), Json(payload)).await
}

pub async fn action_drop_item(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<RemoveInventoryItemRequest>,
) -> AppResult<Json<InventoryItem>> {
    let auth = crate::auth::AuthContext::agent(agent_id.clone());
    remove_item_from_agent_inventory(State(state), auth, Path(agent_id), Json(payload)).await
}

pub async fn action_use_item(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<UseItemRequest>,
) -> AppResult<Json<ApiResponse<Agent>>> {
    use_item(State(state), AgentId(agent_id), Json(payload)).await
}

pub async fn action_transfer_item(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<TransferItemRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let auth = crate::auth::AuthContext::agent(agent_id.clone());
    transfer_item_between_agents(State(state), auth, Path(agent_id), Json(payload)).await
}

#[derive(Debug, Serialize)]
pub struct EconomySnapshot {
    pub balance_cents: i64,
    pub last_income_cents: Option<i64>,
    pub last_income_reason: Option<String>,
    pub last_income_at: Option<String>,
    pub last_expense_cents: Option<i64>,
    pub last_expense_reason: Option<String>,
    pub last_expense_at: Option<String>,
}

pub async fn action_check_balance(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<EconomySnapshot>>> {
    let row = sqlx::query_as::<_, (i64, Option<i64>, Option<String>, Option<chrono::DateTime<chrono::Utc>>, Option<i64>, Option<String>, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"
        SELECT balance_cents,
               last_income_cents, last_income_reason, last_income_at,
               last_expense_cents, last_expense_reason, last_expense_at
        FROM agents
        WHERE id = $1
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(ApiResponse::from(EconomySnapshot {
        balance_cents: row.0,
        last_income_cents: row.1,
        last_income_reason: row.2,
        last_income_at: row.3.map(|dt| dt.to_rfc3339()),
        last_expense_cents: row.4,
        last_expense_reason: row.5,
        last_expense_at: row.6.map(|dt| dt.to_rfc3339()),
    })))
}

pub async fn action_pay_agent(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<PayAgentRequest>,
) -> AppResult<Json<ApiResponse<crate::routes::economy::PayAgentResponse>>> {
    let auth = crate::auth::AuthContext::agent(agent_id.clone());
    pay_agent(State(state), auth, Path(agent_id), Json(payload)).await
}

pub async fn action_request_money(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<RequestMoneyRequest>,
) -> AppResult<Json<ApiResponse<crate::routes::economy::RequestMoneyResponse>>> {
    let auth = crate::auth::AuthContext::agent(agent_id.clone());
    request_money(State(state), auth, Path(agent_id), Json(payload)).await
}

pub async fn action_respond_money_request(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<RespondMoneyRequestRequest>,
) -> AppResult<Json<ApiResponse<crate::routes::economy::RespondMoneyRequestResponse>>> {
    let auth = crate::auth::AuthContext::agent(agent_id.clone());
    respond_money_request(State(state), auth, Path(agent_id), Json(payload)).await
}

pub async fn action_get_transaction_log(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<GetTransactionLogRequest>,
) -> AppResult<Json<Vec<crate::routes::economy::EconomyTransaction>>> {
    get_transaction_log(State(state), Path(agent_id), Json(payload)).await
}

#[derive(Debug, Serialize)]
pub struct VitalsSnapshot {
    pub food_level: i16,
    pub water_level: i16,
    pub stamina_level: i16,
    pub sleep_level: i16,
    pub hygiene_level: i16,
    pub appearance_level: i16,
    pub balance_cents: i64,
}

pub async fn action_check_vitals(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<VitalsSnapshot>>> {
    let mut tx = state.pool().begin().await?;
    let agent = crate::routes::vitals::apply_vitals_decay_tx(&mut tx, &agent_id).await?;
    tx.commit().await?;

    Ok(Json(ApiResponse::from(VitalsSnapshot {
        food_level: agent.food_level,
        water_level: agent.water_level,
        stamina_level: agent.stamina_level,
        sleep_level: agent.sleep_level,
        hygiene_level: agent.hygiene_level,
        appearance_level: agent.appearance_level,
        balance_cents: agent.balance_cents,
    })))
}

#[derive(Debug, Deserialize)]
pub struct BuyItemRequest {
    pub item_id: String,
    pub quantity: Option<i16>,
}

#[derive(Debug, Serialize)]
pub struct BuyItemResponse {
    pub item_id: String,
    pub item_name: String,
    pub quantity_bought: i16,
    pub total_cost_cents: i64,
    pub new_balance_cents: i64,
}

pub async fn action_buy_item(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<BuyItemRequest>,
) -> AppResult<Json<ApiResponse<BuyItemResponse>>> {
    let buy_quantity = payload.quantity.unwrap_or(1).max(1);

    let mut tx = state.pool().begin().await?;

    // Apply vitals decay first
    let buyer = crate::routes::vitals::apply_vitals_decay_tx(&mut tx, &agent_id).await?;

    // Fetch the item with FOR UPDATE
    let item = sqlx::query_as::<_, InventoryItem>(
        r#"
        SELECT id, name, held_by, location_id, state, quantity, consumable_type, vital_value, price_cents
        FROM inventory_items
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(&payload.item_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    // Validate: item must be on the ground (for sale)
    if item.held_by.is_some() {
        return Err(AppError::BadRequest("item is not for sale".to_string()));
    }

    // Check if the shop is open
    let item_location = item.location_id.as_deref().ok_or_else(|| {
        AppError::BadRequest("item has no location".to_string())
    })?;
    let location_prefix = item_location.split('_').take(2).collect::<Vec<_>>().join("_");
    let (sim_time, _, _, _) = crate::routes::world::compute_sim_time(state.pool()).await;
    let sim_hour = sim_time.hour() as i16;
    let shop_open = sqlx::query_scalar::<_, bool>(
        r#"SELECT COALESCE(opens_at <= $1 AND closes_at > $1, true) FROM shops WHERE location_prefix = $2 AND is_active = TRUE"#,
    )
    .bind(sim_hour)
    .bind(&location_prefix)
    .fetch_optional(state.pool())
    .await?
    .unwrap_or(true); // default to open if not a shop

    if !shop_open {
        return Err(AppError::BadRequest("the shop is closed right now".to_string()));
    }

    let item_location = item.location_id.as_deref().ok_or_else(|| {
        AppError::BadRequest("item has no location".to_string())
    })?;

    // Validate: buyer must be at same location
    if buyer.current_location_id != item_location {
        return Err(AppError::BadRequest(
            format!("you must be at {} to buy this item", item_location)
        ));
    }

    // Validate: item must have a price
    let price_per_unit = item.price_cents.ok_or_else(|| {
        AppError::BadRequest("item is not for sale".to_string())
    })?;

    // Validate: enough quantity
    if item.quantity < buy_quantity {
        return Err(AppError::BadRequest(
            format!("not enough stock (have {}, requested {})", item.quantity, buy_quantity)
        ));
    }

    let total_cost = price_per_unit * buy_quantity as i64;

    // Validate: buyer has enough balance
    if buyer.balance_cents < total_cost {
        return Err(AppError::BadRequest(
            format!("insufficient balance (have ${:.2}, need ${:.2})", buyer.balance_cents as f64 / 100.0, total_cost as f64 / 100.0)
        ));
    }

    // Debit buyer (with balance guard)
    let new_balance = sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE agents
        SET balance_cents = balance_cents - $1,
            last_expense_cents = $1,
            last_expense_reason = 'Purchased item',
            last_expense_at = NOW(),
            updated_at = NOW()
        WHERE id = $2 AND balance_cents >= $1
        RETURNING balance_cents
        "#,
    )
    .bind(total_cost)
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::BadRequest("insufficient balance".to_string()))?;

    // Credit shopkeeper (if one exists at this shop — via shops table)
    let location_prefix = item_location.split('_').take(2).collect::<Vec<_>>().join("_");
    let shopkeeper_id: Option<String> = sqlx::query_scalar(
        r#"
        SELECT a.id FROM agents a
        JOIN agent_jobs aj ON aj.agent_id = a.id
        JOIN shops s ON s.shopkeeper_job_id = aj.job_id
        WHERE s.location_prefix = $1 AND aj.status = 'active' AND a.is_active = TRUE
          AND a.current_location_id LIKE $2
        LIMIT 1
        "#,
    )
    .bind(&location_prefix)
    .bind(format!("{}%", location_prefix))
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(ref sk_id) = shopkeeper_id {
        sqlx::query(
            r#"
            UPDATE agents
            SET balance_cents = balance_cents + $1,
                last_income_cents = $1,
                last_income_reason = 'Shop sale',
                last_income_at = NOW(),
                updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(total_cost)
        .bind(sk_id)
        .execute(&mut *tx)
        .await?;
    }

    // Transfer item: either reduce quantity or fully transfer
    if buy_quantity >= item.quantity {
        // Buy all — transfer to buyer
        sqlx::query(
            r#"
            UPDATE inventory_items
            SET held_by = $1,
                location_id = NULL,
                price_cents = NULL,
                quantity = $2
            WHERE id = $3
            "#,
        )
        .bind(&agent_id)
        .bind(buy_quantity)
        .bind(&payload.item_id)
        .execute(&mut *tx)
        .await?;
    } else {
        // Buy partial — reduce shelf quantity, create new item for buyer
        sqlx::query(
            r#"
            UPDATE inventory_items
            SET quantity = quantity - $1
            WHERE id = $2
            "#,
        )
        .bind(buy_quantity)
        .bind(&payload.item_id)
        .execute(&mut *tx)
        .await?;

        // Create new item in buyer's inventory
        let new_item_id = format!("{}_{}", &item.id, Uuid::new_v4());
        sqlx::query(
            r#"
            INSERT INTO inventory_items (id, name, held_by, location_id, state, quantity, consumable_type, vital_value, price_cents)
            VALUES ($1, $2, $3, NULL, $4, $5, $6, $7, NULL)
            "#,
        )
        .bind(&new_item_id)
        .bind(&item.name)
        .bind(&agent_id)
        .bind(&item.state)
        .bind(buy_quantity)
        .bind(&item.consumable_type)
        .bind(&item.vital_value)
        .execute(&mut *tx)
        .await?;
    }

    // Create economy transaction record (only if shopkeeper exists, since to_agent_id has FK constraint)
    if let Some(ref sk_id) = shopkeeper_id {
        sqlx::query(
            r#"
            INSERT INTO economy_transactions (from_agent_id, to_agent_id, amount_cents, reason, transaction_type, status, location_id)
            VALUES ($1, $2, $3, $4, 'payment', 'completed', $5)
            "#,
        )
        .bind(&agent_id)
        .bind(sk_id)
        .bind(total_cost)
        .bind(format!("Purchased {} x{}", item.name, buy_quantity))
        .bind(item_location)
        .execute(&mut *tx)
        .await?;
    }

    // Insert event
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.bought_item")
    .bind(&agent_id)
    .bind(item_location)
    .bind(format!("Agent {} bought {} x{}", agent_id, item.name, buy_quantity))
    .bind(serde_json::json!({
        "item_id": &payload.item_id,
        "item_name": &item.name,
        "quantity": buy_quantity,
        "total_cost_cents": total_cost,
        "shopkeeper_id": shopkeeper_id,
    }).to_string())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(BuyItemResponse {
        item_id: payload.item_id,
        item_name: item.name,
        quantity_bought: buy_quantity,
        total_cost_cents: total_cost,
        new_balance_cents: new_balance,
    })))
}

// ---------------------------------------------------------------------------
// Shopkeeper tools
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ShelfStockResponse {
    pub shelf_items: Vec<InventoryItem>,
    pub backroom_items: Vec<InventoryItem>,
    pub pending_deliveries: Vec<WorldObject>,
    pub shop_balance_cents: i64,
}

pub async fn action_check_shelf_stock(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<ShelfStockResponse>>> {
    let location_prefix = sqlx::query_scalar::<_, String>(
        r#"SELECT current_location_id FROM agents WHERE id = $1"#,
    )
    .bind(&agent_id)
    .fetch_one(state.pool())
    .await?;

    let prefix = location_prefix.split('_').take(2).collect::<Vec<_>>().join("_");

    // Shelf items (priced, not backroom)
    let shelf_items = sqlx::query_as::<_, InventoryItem>(
        r#"
        SELECT id, name, held_by, location_id, state, quantity, consumable_type, vital_value, price_cents
        FROM inventory_items
        WHERE location_id LIKE $1 AND price_cents IS NOT NULL AND held_by IS NULL
          AND (state->>'backroom' IS NULL OR state->>'backroom' = 'false')
        ORDER BY name
        "#,
    )
    .bind(format!("{}%", prefix))
    .fetch_all(state.pool())
    .await?;

    // Backroom items (at checkout, backroom flag, no price)
    let backroom_items = sqlx::query_as::<_, InventoryItem>(
        r#"
        SELECT id, name, held_by, location_id, state, quantity, consumable_type, vital_value, price_cents
        FROM inventory_items
        WHERE location_id LIKE $1 AND price_cents IS NULL AND held_by IS NULL
          AND state->>'backroom' = 'true'
        ORDER BY name
        "#,
    )
    .bind(format!("{}%", prefix))
    .fetch_all(state.pool())
    .await?;

    // Pending deliveries
    let pending_deliveries = sqlx::query_as::<_, WorldObject>(
        r#"
        SELECT id, name, location_id, state, actions
        FROM world_objects
        WHERE location_id LIKE $1 AND state->>'delivery_pending' = 'true'
        ORDER BY name
        "#,
    )
    .bind(format!("{}%", prefix))
    .fetch_all(state.pool())
    .await?;

    // Shop balance (from shops table)
    let shop_balance_cents = sqlx::query_scalar::<_, i64>(
        r#"SELECT balance_cents FROM shops WHERE location_prefix = $1 AND is_active = TRUE"#,
    )
    .bind(&prefix)
    .fetch_one(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(ShelfStockResponse {
        shelf_items,
        backroom_items,
        pending_deliveries,
        shop_balance_cents,
    })))
}

#[derive(Debug, Deserialize)]
pub struct RestockShelfRequest {
    pub item_id: String,
    pub shelf_price_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct RestockShelfResponse {
    pub item_id: String,
    pub item_name: String,
    pub quantity: i16,
    pub shelf_price_cents: i64,
}

pub async fn action_restock_shelf(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<RestockShelfRequest>,
) -> AppResult<Json<ApiResponse<RestockShelfResponse>>> {
    let mut tx = state.pool().begin().await?;

    // Fetch the backroom item with FOR UPDATE
    let item = sqlx::query_as::<_, InventoryItem>(
        r#"
        SELECT id, name, held_by, location_id, state, quantity, consumable_type, vital_value, price_cents
        FROM inventory_items
        WHERE id = $1 AND held_by IS NULL AND price_cents IS NULL AND state->>'backroom' = 'true'
        FOR UPDATE
        "#,
    )
    .bind(&payload.item_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("item not found in backroom".to_string()))?;

    // Verify agent is at the same shop
    let agent_loc = sqlx::query_scalar::<_, String>(
        r#"SELECT current_location_id FROM agents WHERE id = $1"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    let prefix = agent_loc.split('_').take(2).collect::<Vec<_>>().join("_");
    let item_prefix = item.location_id.as_deref().unwrap_or("").split('_').take(2).collect::<Vec<_>>().join("_");
    if prefix != item_prefix {
        return Err(AppError::BadRequest("item is not at your shop".to_string()));
    }

    // Find the shelf location for this shop — prefer a location that already has priced items,
    // otherwise use the agent's current location
    let shelf_location = sqlx::query_scalar::<_, String>(
        r#"
        SELECT location_id FROM inventory_items
        WHERE location_id LIKE $1 AND price_cents IS NOT NULL AND held_by IS NULL
          AND (state->>'backroom' IS NULL OR state->>'backroom' = 'false')
        LIMIT 1
        "#,
    )
    .bind(format!("{}%", prefix))
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or_else(|| agent_loc.clone());

    // Move item to aisle, set price, remove backroom flag
    let restock_price = item.state.get("restock_price")
        .and_then(|v| v.as_i64())
        .unwrap_or(payload.shelf_price_cents);

    let updated = sqlx::query(
        r#"
        UPDATE inventory_items
        SET location_id = $1,
            price_cents = $2,
            state = state - 'backroom' - 'restock_price'
        WHERE id = $3
        "#,
    )
    .bind(&shelf_location)
    .bind(restock_price)
    .bind(&payload.item_id)
    .execute(&mut *tx)
    .await?;

    if updated.rows_affected() == 0 {
        return Err(AppError::BadRequest("failed to restock item".to_string()));
    }

    // Insert event
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.restocked_shelf")
    .bind(&agent_id)
    .bind(&shelf_location)
    .bind(format!("Agent {} restocked {} x{}", agent_id, item.name, item.quantity))
    .bind(serde_json::json!({"item_id": &payload.item_id, "item_name": &item.name, "quantity": item.quantity, "price_cents": restock_price}).to_string())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(RestockShelfResponse {
        item_id: payload.item_id,
        item_name: item.name,
        quantity: item.quantity,
        shelf_price_cents: restock_price,
    })))
}

#[derive(Debug, Deserialize)]
pub struct ReceiveDeliveryRequest {
    pub delivery_id: String,
}

#[derive(Debug, Serialize)]
pub struct ReceiveDeliveryResponse {
    pub delivery_id: String,
    pub items_received: Vec<String>,
    pub total_cost_cents: i64,
}

pub async fn action_receive_delivery(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<ReceiveDeliveryRequest>,
) -> AppResult<Json<ApiResponse<ReceiveDeliveryResponse>>> {
    let mut tx = state.pool().begin().await?;

    // Fetch the delivery object with FOR UPDATE
    let delivery = sqlx::query_as::<_, WorldObject>(
        r#"
        SELECT id, name, location_id, state, actions
        FROM world_objects
        WHERE id = $1 AND state->>'delivery_pending' = 'true'
        FOR UPDATE
        "#,
    )
    .bind(&payload.delivery_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("no pending delivery found".to_string()))?;

    // Verify agent is at same shop
    let agent_loc = sqlx::query_scalar::<_, String>(
        r#"SELECT current_location_id FROM agents WHERE id = $1"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    let prefix = agent_loc.split('_').take(2).collect::<Vec<_>>().join("_");
    let delivery_prefix = delivery.location_id.as_deref().unwrap_or("").split('_').take(2).collect::<Vec<_>>().join("_");
    if prefix != delivery_prefix {
        return Err(AppError::BadRequest("delivery is not at your shop".to_string()));
    }

    // Parse the manifest
    let items_manifest = delivery.state.get("items")
        .and_then(|v| v.as_array())
        .ok_or(AppError::BadRequest("delivery has no items manifest".to_string()))?;

    let total_cost: i64 = delivery.state.get("total_cost_cents")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    // Check shopkeeper balance
    let balance = sqlx::query_scalar::<_, i64>(
        r#"SELECT balance_cents FROM agents WHERE id = $1 FOR UPDATE"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if balance < total_cost {
        return Err(AppError::BadRequest(
            format!("insufficient balance for delivery (have ${:.2}, need ${:.2})", balance as f64 / 100.0, total_cost as f64 / 100.0)
        ));
    }

    // Deduct cost from shopkeeper
    if total_cost > 0 {
        sqlx::query(
            r#"
            UPDATE agents SET balance_cents = balance_cents - $1,
                last_expense_cents = $1,
                last_expense_reason = 'Delivery received',
                last_expense_at = NOW(),
                updated_at = NOW()
            WHERE id = $2 AND balance_cents >= $1
            "#,
        )
        .bind(total_cost)
        .bind(&agent_id)
        .execute(&mut *tx)
        .await?;
    }

    // Create backroom items from manifest
    let checkout_location = format!("{}_checkout", prefix);
    let mut items_received = Vec::new();

    for (i, item_def) in items_manifest.iter().enumerate() {
        let name = item_def.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown Item");
        let quantity = item_def.get("quantity").and_then(|v| v.as_i64()).unwrap_or(1) as i16;
        let consumable_type = item_def.get("consumable_type").and_then(|v| v.as_str());
        let vital_value = item_def.get("vital_value").and_then(|v| v.as_i64()).map(|v| v as i16);
        let restock_price = item_def.get("cost_cents").and_then(|v| v.as_i64())
            .map(|c| (c as f64 * 2.0) as i64); // 2x markup for shelf price

        let item_id = format!("br_{}_{}", &payload.delivery_id, i);
        let state = serde_json::json!({
            "backroom": true,
            "restock_price": restock_price,
            "from_delivery": &payload.delivery_id,
        });

        sqlx::query(
            r#"
            INSERT INTO inventory_items (id, name, held_by, location_id, state, quantity, consumable_type, vital_value, price_cents)
            VALUES ($1, $2, NULL, $3, $4::jsonb, $5, $6, $7, NULL)
            "#,
        )
        .bind(&item_id)
        .bind(name)
        .bind(&checkout_location)
        .bind(state.to_string())
        .bind(quantity)
        .bind(consumable_type)
        .bind(vital_value)
        .execute(&mut *tx)
        .await?;

        items_received.push(format!("{} x{}", name, quantity));
    }

    // Mark delivery as received
    let now_str = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        UPDATE world_objects
        SET state = jsonb_set(
            state || '{"delivery_pending": false}',
            '{received_at}',
            $1::jsonb
        )
        WHERE id = $2
        "#,
    )
    .bind(format!("\"{}\"", now_str))
    .bind(&payload.delivery_id)
    .execute(&mut *tx)
    .await?;

    // Insert event
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.received_delivery")
    .bind(&agent_id)
    .bind(&checkout_location)
    .bind(format!("Agent {} received delivery", agent_id))
    .bind(serde_json::json!({"delivery_id": &payload.delivery_id, "items": items_received, "cost_cents": total_cost}).to_string())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(ReceiveDeliveryResponse {
        delivery_id: payload.delivery_id,
        items_received,
        total_cost_cents: total_cost,
    })))
}

#[derive(Debug, Deserialize)]
pub struct OrderDeliveryItem {
    pub name: String,
    pub quantity: i16,
    pub consumable_type: Option<String>,
    pub vital_value: Option<i16>,
    pub cost_cents: i64,
}

#[derive(Debug, Deserialize)]
pub struct OrderDeliveryRequest {
    pub items: Vec<OrderDeliveryItem>,
}

#[derive(Debug, Serialize)]
pub struct OrderDeliveryResponse {
    pub delivery_id: String,
    pub items_ordered: Vec<String>,
    pub total_cost_cents: i64,
    pub estimated_arrival: String,
}

pub async fn action_order_delivery(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<OrderDeliveryRequest>,
) -> AppResult<Json<ApiResponse<OrderDeliveryResponse>>> {
    if payload.items.is_empty() {
        return Err(AppError::BadRequest("must order at least one item".to_string()));
    }

    let mut tx = state.pool().begin().await?;

    // Verify agent is at a shop
    let agent_loc = sqlx::query_scalar::<_, String>(
        r#"SELECT current_location_id FROM agents WHERE id = $1"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    let prefix = agent_loc.split('_').take(2).collect::<Vec<_>>().join("_");

    // Check no pending delivery already exists
    let has_pending = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM world_objects
            WHERE location_id LIKE $1 AND state->>'delivery_pending' = 'true'
        )
        "#,
    )
    .bind(format!("{}%", prefix))
    .fetch_one(&mut *tx)
    .await?;

    if has_pending {
        return Err(AppError::BadRequest("a delivery is already pending — receive it first".to_string()));
    }

    // Calculate total cost
    let total_cost: i64 = payload.items.iter().map(|i| i.cost_cents * i.quantity as i64).sum();

    // Check balance
    let balance = sqlx::query_scalar::<_, i64>(
        r#"SELECT balance_cents FROM agents WHERE id = $1 FOR UPDATE"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if balance < total_cost {
        return Err(AppError::BadRequest(
            format!("insufficient balance (have ${:.2}, need ${:.2})", balance as f64 / 100.0, total_cost as f64 / 100.0)
        ));
    }

    // Deduct cost
    if total_cost > 0 {
        sqlx::query(
            r#"
            UPDATE agents SET balance_cents = balance_cents - $1,
                last_expense_cents = $1,
                last_expense_reason = 'Delivery ordered',
                last_expense_at = NOW(),
                updated_at = NOW()
            WHERE id = $2 AND balance_cents >= $1
            "#,
        )
        .bind(total_cost)
        .bind(&agent_id)
        .execute(&mut *tx)
        .await?;
    }

    // Create delivery object
    let checkout_location = format!("{}_checkout", prefix);
    let delivery_id = format!("delivery_{}", Uuid::new_v4());

    let items_json: Vec<serde_json::Value> = payload.items.iter().map(|i| {
        serde_json::json!({
            "name": i.name,
            "quantity": i.quantity,
            "consumable_type": i.consumable_type,
            "vital_value": i.vital_value,
            "cost_cents": i.cost_cents,
        })
    }).collect();

    let items_ordered: Vec<String> = payload.items.iter()
        .map(|i| format!("{} x{}", i.name, i.quantity))
        .collect();

    sqlx::query(
        r#"
        INSERT INTO world_objects (id, name, location_id, state, actions)
        VALUES ($1, 'Delivery Crate', $2, $3::jsonb, ARRAY['receive'])
        "#,
    )
    .bind(&delivery_id)
    .bind(&checkout_location)
    .bind(serde_json::json!({
        "delivery_pending": true,
        "items": items_json,
        "total_cost_cents": total_cost,
        "ordered_by": agent_id,
        "ordered_at": Utc::now().to_rfc3339(),
    }).to_string())
    .execute(&mut *tx)
    .await?;

    // Insert event
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.ordered_delivery")
    .bind(&agent_id)
    .bind(&checkout_location)
    .bind(format!("Agent {} ordered delivery", agent_id))
    .bind(serde_json::json!({"delivery_id": &delivery_id, "items": items_ordered, "cost_cents": total_cost}).to_string())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(OrderDeliveryResponse {
        delivery_id,
        items_ordered,
        total_cost_cents: total_cost,
        estimated_arrival: "immediate".to_string(),
    })))
}

#[derive(Debug, Serialize)]
pub struct CleanShopResponse {
    pub location_cleaned: String,
    pub stamina_cost: i16,
    pub last_cleaned_at: String,
}

pub async fn action_clean_shop(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<CleanShopResponse>>> {
    let mut tx = state.pool().begin().await?;

    // Apply vitals decay and check stamina
    let agent = crate::routes::vitals::apply_vitals_decay_tx(&mut tx, &agent_id).await?;
    let stamina_cost: i16 = 10;

    if agent.stamina_level < stamina_cost {
        return Err(AppError::BadRequest(
            format!("not enough stamina to clean (have {}, need {})", agent.stamina_level, stamina_cost)
        ));
    }

    // Deduct stamina
    sqlx::query(
        r#"
        UPDATE agents SET stamina_level = stamina_level - $1, updated_at = NOW()
        WHERE id = $2 AND stamina_level >= $1
        "#,
    )
    .bind(stamina_cost)
    .bind(&agent_id)
    .execute(&mut *tx)
    .await?;

    // Update the checkout counter's last_cleaned_at
    let prefix = agent.current_location_id.split('_').take(2).collect::<Vec<_>>().join("_");
    let _counter_id = format!("{}_counter_{}", prefix, prefix.split('_').last().unwrap_or("harvey"));

    // Try to find the counter object at this shop
    let now_str = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        UPDATE world_objects
        SET state = jsonb_set(state, '{last_cleaned_at}', $1::jsonb)
        WHERE id LIKE $2 AND location_id LIKE $3
        "#,
    )
    .bind(format!("\"{}\"", now_str))
    .bind(format!("{}%", prefix))
    .bind(format!("{}%", prefix))
    .execute(&mut *tx)
    .await?;

    // Insert event
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.cleaned_shop")
    .bind(&agent_id)
    .bind(&agent.current_location_id)
    .bind(format!("Agent {} cleaned the shop", agent_id))
    .bind("{}")
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(CleanShopResponse {
        location_cleaned: agent.current_location_id,
        stamina_cost,
        last_cleaned_at: now_str,
    })))
}

// ---------------------------------------------------------------------------
// Job system tools
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct JobOpening {
    pub job_id: String,
    pub job_name: String,
    pub summary: String,
    pub employer_id: Option<String>,
    pub employer_name: Option<String>,
    pub wage_cents: Option<i64>,
    pub is_city_job: bool,
    pub active_employees: i64,
}


// ---------------------------------------------------------------------------
// Shop browsing (customer-facing)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct BrowseShopResponse {
    pub shop_name: String,
    pub shop_id: String,
    pub open: bool,
    pub hours: String,
    pub items: Vec<InventoryItem>,
}

pub async fn action_browse_shop(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<BrowseShopResponse>>> {
    let agent_loc = sqlx::query_scalar::<_, String>(
        r#"SELECT current_location_id FROM agents WHERE id = $1"#,
    )
    .bind(&agent_id)
    .fetch_one(state.pool())
    .await?;

    let prefix = agent_loc.split('_').take(2).collect::<Vec<_>>().join("_");

    // Find shop matching this location prefix
    let shop = sqlx::query_as::<_, (String, String, i16, i16)>(
        r#"SELECT id, name, opens_at, closes_at FROM shops WHERE location_prefix = $1 AND is_active = TRUE"#,
    )
    .bind(&prefix)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::BadRequest("you're not at a shop".to_string()))?;

    // Check if shop is open
    let (sim_time, _, _, _) = crate::routes::world::compute_sim_time(state.pool()).await;
    let sim_hour = sim_time.hour() as i16;
    let open = shop.2 <= sim_hour && shop.3 > sim_hour;
    let hours = format!("{}am - {}pm", shop.2, if shop.3 > 12 { shop.3 - 12 } else { shop.3 });

    // Get shelf items (priced, not held, not backroom)
    let items = sqlx::query_as::<_, InventoryItem>(
        r#"
        SELECT id, name, held_by, location_id, state, quantity, consumable_type, vital_value, price_cents
        FROM inventory_items
        WHERE location_id LIKE $1 AND price_cents IS NOT NULL AND held_by IS NULL
          AND (state->>'backroom' IS NULL OR state->>'backroom' = 'false')
        ORDER BY consumable_type, name
        "#,
    )
    .bind(format!("{}%", prefix))
    .fetch_all(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(BrowseShopResponse {
        shop_id: shop.0,
        shop_name: shop.1,
        open,
        hours,
        items,
    })))
}

pub async fn action_list_job_openings(
    State(state): State<AppState>,
    AgentId(_agent_id): AgentId,
) -> AppResult<Json<ApiResponse<Vec<JobOpening>>>> {
    let rows = sqlx::query(
        r#"
        SELECT j.id, j.name, j.summary, j.employer_id,
               ea.name AS employer_name,
               j.wage_cents, j.is_city_job,
               COALESCE(emp.cnt, 0) AS active_employees
        FROM jobs j
        LEFT JOIN agents ea ON ea.id = j.employer_id
        LEFT JOIN LATERAL (
            SELECT COUNT(*)::bigint AS cnt FROM agent_jobs aj WHERE aj.job_id = j.id AND aj.status = 'active'
        ) emp ON TRUE
        WHERE j.kind = 'town' AND (j.employer_id IS NOT NULL OR j.is_city_job = TRUE)
        ORDER BY j.name
        "#,
    )
    .fetch_all(state.pool())
    .await?;

    let mut openings = Vec::new();
    for row in rows {
        let job_id: String = row.get("id");
        let job_name: String = row.get("name");
        let summary: String = row.get("summary");
        let employer_id: Option<String> = row.get("employer_id");
        let employer_name: Option<String> = row.get("employer_name");
        let wage_cents: Option<i64> = row.get("wage_cents");
        let is_city_job: bool = row.get("is_city_job");
        let active_employees: i64 = row.get("active_employees");

        openings.push(JobOpening {
            job_id,
            job_name,
            summary,
            employer_id,
            employer_name,
            wage_cents,
            is_city_job,
            active_employees,
        });
    }

    Ok(Json(ApiResponse::from(openings)))
}

#[derive(Debug, Deserialize)]
pub struct ApplyForJobRequest {
    pub job_id: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ApplyForJobResponse {
    pub job_id: String,
    pub job_name: String,
    pub status: String,
    pub message: String,
}

pub async fn action_apply_for_job(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<ApplyForJobRequest>,
) -> AppResult<Json<ApiResponse<ApplyForJobResponse>>> {
    let mut tx = state.pool().begin().await?;

    // Fetch the job
    let job = sqlx::query_as::<_, (String, String, Option<String>, bool, Option<i64>)>(
        r#"SELECT id, name, employer_id, is_city_job, wage_cents FROM jobs WHERE id = $1 AND kind = 'town'"#,
    )
    .bind(&payload.job_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("job not found".to_string()))?;

    let (job_id, job_name, employer_id, is_city_job, _wage_cents) = job;

    // Check agent doesn't already have this job active
    let already = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM agent_jobs WHERE agent_id = $1 AND job_id = $2 AND status IN ('active', 'pending'))"#,
    )
    .bind(&agent_id)
    .bind(&payload.job_id)
    .fetch_one(&mut *tx)
    .await?;

    if already {
        return Err(AppError::BadRequest("you already have this job".to_string()));
    }

    // Check max_positions
    let max_pos: Option<i32> = sqlx::query_scalar(
        r#"SELECT max_positions FROM jobs WHERE id = $1"#,
    )
    .bind(&payload.job_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten();

    if let Some(max) = max_pos {
        let current_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*)::bigint FROM agent_jobs WHERE job_id = $1 AND status IN ('active', 'pending')"#,
        )
        .bind(&payload.job_id)
        .fetch_one(&mut *tx)
        .await?;

        if current_count >= max as i64 {
            return Err(AppError::BadRequest(format!("this position is full ({} of {})", current_count, max)));
        }
    }

    let status = if is_city_job {
        "active" // city jobs auto-approve
    } else {
        "pending"
    };

    // Create the assignment
    sqlx::query(
        r#"
        INSERT INTO agent_jobs (agent_id, job_id, is_primary, notes, status)
        VALUES ($1, $2, FALSE, $3, $4)
        ON CONFLICT (agent_id, job_id) DO UPDATE SET status = $4, notes = $3, updated_at = NOW()
        "#,
    )
    .bind(&agent_id)
    .bind(&payload.job_id)
    .bind(&payload.notes)
    .bind(status)
    .execute(&mut *tx)
    .await?;

    let message = if is_city_job {
        "You have been hired for a city position. Welcome aboard!".to_string()
    } else {
        "Application submitted. The employer will review it.".to_string()
    };

    // Wake employer about application (if not city job)
    if let Some(ref eid) = employer_id {
        if !is_city_job {
            let _ = crate::routes::citizens::enqueue_citizen_wake_tx(
                &mut tx,
                eid,
                "job_application",
                serde_json::json!({"job_id": &payload.job_id, "applicant_id": &agent_id}),
                format!("{} applied for a job at your business.", agent_id),
                serde_json::json!({"event_type": "job.application", "applicant_id": &agent_id, "job_id": &payload.job_id}),
                serde_json::json!([]),
                true,
            ).await;
        }
    }

    // Insert event
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.applied_for_job")
    .bind(&agent_id)
    .bind(sqlx::query_scalar::<_, String>("SELECT current_location_id FROM agents WHERE id = $1").bind(&agent_id).fetch_one(&mut *tx).await?)
    .bind(format!("Agent {} applied for job {}", agent_id, job_name))
    .bind(serde_json::json!({"job_id": &payload.job_id, "status": status}).to_string())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(ApplyForJobResponse {
        job_id,
        job_name,
        status: status.to_string(),
        message,
    })))
}

#[derive(Debug, Serialize)]
pub struct PayrollEntry {
    pub employee_id: String,
    pub employee_name: String,
    pub job_id: String,
    pub job_name: String,
    pub wage_cents: Option<i64>,
    pub last_paid_at: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct CheckPayrollResponse {
    pub employees: Vec<PayrollEntry>,
    pub employer_balance_cents: i64,
    pub total_payroll_cents: i64,
}

pub async fn action_check_payroll(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<CheckPayrollResponse>>> {
    let mut tx = state.pool().begin().await?;

    // Get employer balance
    let balance = sqlx::query_scalar::<_, i64>(
        r#"SELECT balance_cents FROM agents WHERE id = $1"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    // Get active employees where this agent is the employer
    let rows = sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<chrono::DateTime<chrono::Utc>>, String)>(
        r#"
        SELECT a.id, a.name, j.id, j.name, j.wage_cents, aj.last_paid_at, aj.status
        FROM agent_jobs aj
        JOIN jobs j ON j.id = aj.job_id
        JOIN agents a ON a.id = aj.agent_id
        WHERE j.employer_id = $1 AND aj.status = 'active'
        ORDER BY a.name
        "#,
    )
    .bind(&agent_id)
    .fetch_all(&mut *tx)
    .await?;

    let employees: Vec<PayrollEntry> = rows.into_iter().map(|r| PayrollEntry {
        employee_id: r.0,
        employee_name: r.1,
        job_id: r.2,
        job_name: r.3,
        wage_cents: r.4,
        last_paid_at: r.5.map(|dt| dt.to_rfc3339()),
        status: r.6,
    }).collect();

    let total_payroll: i64 = employees.iter()
        .filter_map(|e| e.wage_cents)
        .sum();

    tx.commit().await?;

    Ok(Json(ApiResponse::from(CheckPayrollResponse {
        employees,
        employer_balance_cents: balance,
        total_payroll_cents: total_payroll,
    })))
}

#[derive(Debug, Deserialize)]
pub struct PayEmployeeRequest {
    pub employee_id: String,
    pub amount_cents: i64,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PayEmployeeResponse {
    pub employee_id: String,
    pub amount_cents: i64,
    pub employer_new_balance_cents: i64,
    pub employee_new_balance_cents: i64,
}

pub async fn action_pay_employee(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<PayEmployeeRequest>,
) -> AppResult<Json<ApiResponse<PayEmployeeResponse>>> {
    if payload.amount_cents <= 0 {
        return Err(AppError::BadRequest("amount must be positive".to_string()));
    }

    let mut tx = state.pool().begin().await?;

    // Verify employer-employee relationship
    let is_employer = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM agent_jobs aj
            JOIN jobs j ON j.id = aj.job_id
            WHERE j.employer_id = $1 AND aj.agent_id = $2 AND aj.status = 'active'
        )
        "#,
    )
    .bind(&agent_id)
    .bind(&payload.employee_id)
    .fetch_one(&mut *tx)
    .await?;

    if !is_employer {
        return Err(AppError::BadRequest("you are not this agent's employer".to_string()));
    }

    // Check employer balance
    let employer_balance = sqlx::query_scalar::<_, i64>(
        r#"SELECT balance_cents FROM agents WHERE id = $1 FOR UPDATE"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if employer_balance < payload.amount_cents {
        return Err(AppError::BadRequest(
            format!("insufficient balance (have ${:.2}, need ${:.2})", employer_balance as f64 / 100.0, payload.amount_cents as f64 / 100.0)
        ));
    }

    // Debit employer
    sqlx::query(
        r#"
        UPDATE agents SET balance_cents = balance_cents - $1,
            last_expense_cents = $1,
            last_expense_reason = 'Salary payment',
            last_expense_at = NOW(),
            updated_at = NOW()
        WHERE id = $2 AND balance_cents >= $1
        RETURNING balance_cents
        "#,
    )
    .bind(payload.amount_cents)
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    // Credit employee
    let employee_new_balance = sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE agents SET balance_cents = balance_cents + $1,
            last_income_cents = $1,
            last_income_reason = 'Salary',
            last_income_at = NOW(),
            updated_at = NOW()
        WHERE id = $2
        RETURNING balance_cents
        "#,
    )
    .bind(payload.amount_cents)
    .bind(&payload.employee_id)
    .fetch_one(&mut *tx)
    .await?;

    // Update last_paid_at
    sqlx::query(
        r#"
        UPDATE agent_jobs SET last_paid_at = NOW(), updated_at = NOW()
        WHERE agent_id = $1 AND job_id IN (SELECT id FROM jobs WHERE employer_id = $2)
        "#,
    )
    .bind(&payload.employee_id)
    .bind(&agent_id)
    .execute(&mut *tx)
    .await?;

    // Create economy transaction
    sqlx::query(
        r#"
        INSERT INTO economy_transactions (from_agent_id, to_agent_id, amount_cents, reason, transaction_type, status, location_id)
        VALUES ($1, $2, $3, $4, 'salary', 'completed', $5)
        "#,
    )
    .bind(&agent_id)
    .bind(&payload.employee_id)
    .bind(payload.amount_cents)
    .bind(payload.reason.as_deref().unwrap_or("Salary payment"))
    .bind(sqlx::query_scalar::<_, String>("SELECT current_location_id FROM agents WHERE id = $1").bind(&agent_id).fetch_one(&mut *tx).await?)
    .execute(&mut *tx)
    .await?;

    // Get employer's new balance
    let employer_new_balance = sqlx::query_scalar::<_, i64>(
        r#"SELECT balance_cents FROM agents WHERE id = $1"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    // Wake employee about payment
    let _ = crate::routes::citizens::enqueue_citizen_wake_tx(
        &mut tx,
        &payload.employee_id,
        "salary_received",
        serde_json::json!({"amount_cents": payload.amount_cents, "employer_id": &agent_id}),
        format!("You received ${:.2} from your employer.", payload.amount_cents as f64 / 100.0),
        serde_json::json!({"event_type": "salary.received", "amount_cents": payload.amount_cents, "employer_id": &agent_id}),
        serde_json::json!([]),
        true,
    ).await;

    // Insert event
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("employer.paid_employee")
    .bind(&agent_id)
    .bind(sqlx::query_scalar::<_, String>("SELECT current_location_id FROM agents WHERE id = $1").bind(&agent_id).fetch_one(&mut *tx).await?)
    .bind(format!("{} paid {} ${:.2}", agent_id, payload.employee_id, payload.amount_cents as f64 / 100.0))
    .bind(serde_json::json!({"employee_id": &payload.employee_id, "amount_cents": payload.amount_cents}).to_string())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(PayEmployeeResponse {
        employee_id: payload.employee_id,
        amount_cents: payload.amount_cents,
        employer_new_balance_cents: employer_new_balance,
        employee_new_balance_cents: employee_new_balance,
    })))
}

#[derive(Debug, Deserialize)]
pub struct ResignJobRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResignJobResponse {
    pub job_id: String,
    pub job_name: String,
    pub status: String,
}

pub async fn action_resign_job(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<ResignJobRequest>,
) -> AppResult<Json<ApiResponse<ResignJobResponse>>> {
    let mut tx = state.pool().begin().await?;

    // Find the agent's primary active job
    let job = sqlx::query_as::<_, (String, String, Option<String>)>(
        r#"
        SELECT j.id, j.name, j.employer_id
        FROM agent_jobs aj
        JOIN jobs j ON j.id = aj.job_id
        WHERE aj.agent_id = $1 AND aj.is_primary = TRUE AND aj.status = 'active'
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("no active primary job to resign from".to_string()))?;

    let (job_id, job_name, employer_id) = job;

    // Mark as resigned
    sqlx::query(
        r#"
        UPDATE agent_jobs SET status = 'resigned', resigned_at = NOW(), updated_at = NOW()
        WHERE agent_id = $1 AND job_id = $2 AND status = 'active'
        "#,
    )
    .bind(&agent_id)
    .bind(&job_id)
    .execute(&mut *tx)
    .await?;

    // Wake employer about resignation
    if let Some(ref eid) = employer_id {
        let reason = payload.reason.as_deref().unwrap_or("No reason given");
        let _ = crate::routes::citizens::enqueue_citizen_wake_tx(
            &mut tx,
            eid,
            "employee_resigned",
            serde_json::json!({"employee_id": &agent_id, "job_id": &job_id}),
            format!("{} resigned from {} — {}", agent_id, job_name, reason),
            serde_json::json!({"event_type": "job.resignation", "employee_id": &agent_id, "job_id": &job_id}),
            serde_json::json!([]),
            true,
        ).await;
    }

    // Insert event
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.resigned_job")
    .bind(&agent_id)
    .bind(sqlx::query_scalar::<_, String>("SELECT current_location_id FROM agents WHERE id = $1").bind(&agent_id).fetch_one(&mut *tx).await?)
    .bind(format!("Agent {} resigned from {}", agent_id, job_name))
    .bind(serde_json::json!({"job_id": &job_id, "reason": payload.reason}).to_string())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(ResignJobResponse {
        job_id,
        job_name,
        status: "resigned".to_string(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct HireApplicantRequest {
    pub applicant_id: String,
    pub job_id: String,
    pub wage_cents: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct HireApplicantResponse {
    pub applicant_id: String,
    pub job_name: String,
    pub status: String,
}

pub async fn action_hire_applicant(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<HireApplicantRequest>,
) -> AppResult<Json<ApiResponse<HireApplicantResponse>>> {
    let mut tx = state.pool().begin().await?;

    // Verify this agent is the employer for the job
    let job = sqlx::query_as::<_, (String, String)>(
        r#"SELECT id, name FROM jobs WHERE id = $1 AND employer_id = $2"#,
    )
    .bind(&payload.job_id)
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("you are not the employer for this job".to_string()))?;

    let (job_id, job_name) = job;

    // Verify pending application exists
    let pending = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM agent_jobs WHERE agent_id = $1 AND job_id = $2 AND status = 'pending')"#,
    )
    .bind(&payload.applicant_id)
    .bind(&payload.job_id)
    .fetch_one(&mut *tx)
    .await?;

    if !pending {
        return Err(AppError::BadRequest("no pending application found for this agent and job".to_string()));
    }

    // Approve the application
    sqlx::query(
        r#"
        UPDATE agent_jobs SET status = 'active', hired_at = NOW(), updated_at = NOW()
        WHERE agent_id = $1 AND job_id = $2 AND status = 'pending'
        "#,
    )
    .bind(&payload.applicant_id)
    .bind(&payload.job_id)
    .execute(&mut *tx)
    .await?;

    // Optionally update wage
    if let Some(wage) = payload.wage_cents {
        sqlx::query(
            r#"UPDATE jobs SET wage_cents = $1, updated_at = NOW() WHERE id = $2"#,
        )
        .bind(wage)
        .bind(&payload.job_id)
        .execute(&mut *tx)
        .await?;
    }

    // Wake applicant about hiring
    let _ = crate::routes::citizens::enqueue_citizen_wake_tx(
        &mut tx,
        &payload.applicant_id,
        "job_hired",
        serde_json::json!({"job_id": &job_id, "employer_id": &agent_id}),
        format!("You've been hired as {}!", job_name),
        serde_json::json!({"event_type": "job.hired", "job_id": &job_id, "employer_id": &agent_id}),
        serde_json::json!([]),
        true,
    ).await;

    // Insert event
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("employer.hired_applicant")
    .bind(&agent_id)
    .bind(sqlx::query_scalar::<_, String>("SELECT current_location_id FROM agents WHERE id = $1").bind(&agent_id).fetch_one(&mut *tx).await?)
    .bind(format!("{} hired {} as {}", agent_id, payload.applicant_id, job_name))
    .bind(serde_json::json!({"applicant_id": &payload.applicant_id, "job_id": &job_id}).to_string())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(HireApplicantResponse {
        applicant_id: payload.applicant_id,
        job_name,
        status: "active".to_string(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct FireEmployeeRequest {
    pub employee_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FireEmployeeResponse {
    pub employee_id: String,
    pub job_name: String,
    pub status: String,
}

pub async fn action_fire_employee(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<FireEmployeeRequest>,
) -> AppResult<Json<ApiResponse<FireEmployeeResponse>>> {
    let mut tx = state.pool().begin().await?;

    // Find the employee's active job where this agent is the employer
    let job = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT j.id, j.name
        FROM agent_jobs aj
        JOIN jobs j ON j.id = aj.job_id
        WHERE aj.agent_id = $1 AND j.employer_id = $2 AND aj.status = 'active'
        "#,
    )
    .bind(&payload.employee_id)
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("you don't employ this agent".to_string()))?;

    let (job_id, job_name) = job;

    // Mark as fired
    sqlx::query(
        r#"
        UPDATE agent_jobs SET status = 'fired', resigned_at = NOW(), updated_at = NOW()
        WHERE agent_id = $1 AND job_id = $2 AND status = 'active'
        "#,
    )
    .bind(&payload.employee_id)
    .bind(&job_id)
    .execute(&mut *tx)
    .await?;

    // Wake employee about termination
    let reason = payload.reason.as_deref().unwrap_or("No reason given");
    let _ = crate::routes::citizens::enqueue_citizen_wake_tx(
        &mut tx,
        &payload.employee_id,
        "job_fired",
        serde_json::json!({"job_id": &job_id, "employer_id": &agent_id, "reason": &payload.reason}),
        format!("You've been fired from {} — {}", job_name, reason),
        serde_json::json!({"event_type": "job.fired", "job_id": &job_id, "employer_id": &agent_id}),
        serde_json::json!([]),
        true,
    ).await;

    // Insert event
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("employer.fired_employee")
    .bind(&agent_id)
    .bind(sqlx::query_scalar::<_, String>("SELECT current_location_id FROM agents WHERE id = $1").bind(&agent_id).fetch_one(&mut *tx).await?)
    .bind(format!("{} fired {} from {}", agent_id, payload.employee_id, job_name))
    .bind(serde_json::json!({"employee_id": &payload.employee_id, "job_id": &job_id, "reason": payload.reason}).to_string())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(FireEmployeeResponse {
        employee_id: payload.employee_id,
        job_name,
        status: "fired".to_string(),
    })))
}

#[derive(Debug, Serialize)]
pub struct CollectCityWageResponse {
    pub job_id: String,
    pub job_name: String,
    pub amount_cents: i64,
    pub new_balance_cents: i64,
}

pub async fn action_collect_city_wage(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<CollectCityWageResponse>>> {
    let mut tx = state.pool().begin().await?;

    // Find agent's active city job
    let job = sqlx::query_as::<_, (String, String, i64, i32, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"
        SELECT j.id, j.name, j.wage_cents, j.pay_period_minutes, aj.last_paid_at
        FROM agent_jobs aj
        JOIN jobs j ON j.id = aj.job_id
        WHERE aj.agent_id = $1 AND j.is_city_job = TRUE AND aj.status = 'active'
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("no active city job found".to_string()))?;

    let (job_id, job_name, wage_cents, pay_period_minutes, last_paid_at) = job;

    // Check if enough time has passed
    if let Some(last) = last_paid_at {
        let elapsed = Utc::now() - last;
        let min_duration = chrono::Duration::minutes(pay_period_minutes as i64);
        if elapsed < min_duration {
            let remaining = min_duration - elapsed;
            return Err(AppError::BadRequest(
                format!("not yet time to collect wage ({} minutes remaining)", remaining.num_minutes())
            ));
        }
    }

    // Debit city treasury
    let treasury_balance = sqlx::query_scalar::<_, i64>(
        r#"SELECT balance_cents FROM agents WHERE id = 'city_treasury' FOR UPDATE"#,
    )
    .fetch_one(&mut *tx)
    .await?;

    if treasury_balance < wage_cents {
        return Err(AppError::BadRequest("city treasury is insolvent".to_string()));
    }

    sqlx::query(
        r#"
        UPDATE agents SET balance_cents = balance_cents - $1, updated_at = NOW()
        WHERE id = 'city_treasury'
        "#,
    )
    .bind(wage_cents)
    .execute(&mut *tx)
    .await?;

    // Credit agent
    let new_balance = sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE agents SET balance_cents = balance_cents + $1,
            last_income_cents = $1,
            last_income_reason = 'City wage',
            last_income_at = NOW(),
            updated_at = NOW()
        WHERE id = $2
        RETURNING balance_cents
        "#,
    )
    .bind(wage_cents)
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    // Update last_paid_at
    sqlx::query(
        r#"
        UPDATE agent_jobs SET last_paid_at = NOW(), updated_at = NOW()
        WHERE agent_id = $1 AND job_id = $2
        "#,
    )
    .bind(&agent_id)
    .bind(&job_id)
    .execute(&mut *tx)
    .await?;

    // Create economy transaction
    sqlx::query(
        r#"
        INSERT INTO economy_transactions (from_agent_id, to_agent_id, amount_cents, reason, transaction_type, status, location_id)
        VALUES ('city_treasury', $1, $2, $3, 'salary', 'completed', $4)
        "#,
    )
    .bind(&agent_id)
    .bind(wage_cents)
    .bind(format!("City wage: {}", job_name))
    .bind(sqlx::query_scalar::<_, String>("SELECT current_location_id FROM agents WHERE id = $1").bind(&agent_id).fetch_one(&mut *tx).await?)
    .execute(&mut *tx)
    .await?;

    // Insert event
    sqlx::query(
        r#"
        INSERT INTO events (type, actor_id, location_id, description, metadata, occurred_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, $6)
        "#,
    )
    .bind("agent.collected_city_wage")
    .bind(&agent_id)
    .bind(sqlx::query_scalar::<_, String>("SELECT current_location_id FROM agents WHERE id = $1").bind(&agent_id).fetch_one(&mut *tx).await?)
    .bind(format!("Agent {} collected city wage for {}", agent_id, job_name))
    .bind(serde_json::json!({"job_id": &job_id, "amount_cents": wage_cents}).to_string())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(CollectCityWageResponse {
        job_id,
        job_name,
        amount_cents: wage_cents,
        new_balance_cents: new_balance,
    })))
}
// ---------------------------------------------------------------------------
// Civic system tools — mayor, elections, civic board
// ---------------------------------------------------------------------------

// Helper: get current mayor agent id
#[allow(dead_code)]
async fn get_current_mayor(pool: &sqlx::PgPool) -> AppResult<String> {
    sqlx::query_scalar::<_, String>(
        r#"SELECT agent_id FROM mayor_terms WHERE is_current = TRUE LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::BadRequest("no current mayor".to_string()))
}

// Helper: check if agent is current mayor
async fn is_mayor(pool: &sqlx::PgPool, agent_id: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM mayor_terms WHERE agent_id = $1 AND is_current = TRUE)"#,
    )
    .bind(agent_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false)
}

// --- Civic Board ---

#[derive(Debug, Serialize)]
pub struct CivicPost {
    pub id: String,
    pub r#type: String,
    pub author_id: Option<String>,
    pub title: String,
    pub body: String,
    pub status: String,
    pub priority: i32,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReadCivicBoardRequest {
    pub r#type: Option<String>,
}

pub async fn action_read_civic_board(
    State(state): State<AppState>,
    AgentId(_agent_id): AgentId,
    Json(payload): Json<ReadCivicBoardRequest>,
) -> AppResult<Json<ApiResponse<Vec<CivicPost>>>> {
    let rows = if let Some(filter_type) = &payload.r#type {
        sqlx::query(
            r#"SELECT id, type, author_id, title, body, status, priority, created_at, resolved_at, resolved_by
            FROM civic_posts WHERE type = $1 AND status = 'active' ORDER BY priority DESC, created_at DESC LIMIT 50"#,
        )
        .bind(filter_type)
        .fetch_all(state.pool())
        .await?
    } else {
        sqlx::query(
            r#"SELECT id, type, author_id, title, body, status, priority, created_at, resolved_at, resolved_by
            FROM civic_posts WHERE status = 'active' ORDER BY priority DESC, created_at DESC LIMIT 50"#,
        )
        .fetch_all(state.pool())
        .await?
    };

    let posts: Vec<CivicPost> = rows.iter().map(|row| CivicPost {
        id: row.get("id"),
        r#type: row.get("type"),
        author_id: row.get("author_id"),
        title: row.get("title"),
        body: row.get("body"),
        status: row.get("status"),
        priority: row.get("priority"),
        created_at: row.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
        resolved_at: row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("resolved_at").map(|dt| dt.to_rfc3339()),
        resolved_by: row.get("resolved_by"),
    }).collect();

    Ok(Json(ApiResponse::from(posts)))
}

#[derive(Debug, Deserialize)]
pub struct FileComplaintRequest {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct CivicPostResponse {
    pub id: String,
    pub r#type: String,
    pub title: String,
    pub status: String,
}

pub async fn action_file_complaint(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<FileComplaintRequest>,
) -> AppResult<Json<ApiResponse<CivicPostResponse>>> {
    let id = format!("complaint_{}", Uuid::new_v4());
    sqlx::query(
        r#"INSERT INTO civic_posts (id, type, author_id, title, body, status) VALUES ($1, 'complaint', $2, $3, $4, 'active')"#,
    )
    .bind(&id)
    .bind(&agent_id)
    .bind(&payload.title)
    .bind(&payload.body)
    .execute(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(CivicPostResponse {
        id,
        r#type: "complaint".to_string(),
        title: payload.title,
        status: "active".to_string(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct NominateHallOfFameRequest {
    pub nominee_id: String,
    pub reason: String,
}

pub async fn action_nominate_for_hall_of_fame(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<NominateHallOfFameRequest>,
) -> AppResult<Json<ApiResponse<CivicPostResponse>>> {
    let id = format!("hof_{}", Uuid::new_v4());
    let title = format!("Hall of Fame Nomination: {}", payload.nominee_id);
    sqlx::query(
        r#"INSERT INTO civic_posts (id, type, author_id, title, body, status, priority) VALUES ($1, 'hall_of_fame', $2, $3, $4, 'active', 5)"#,
    )
    .bind(&id)
    .bind(&agent_id)
    .bind(&title)
    .bind(&payload.reason)
    .execute(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(CivicPostResponse {
        id,
        r#type: "hall_of_fame".to_string(),
        title,
        status: "active".to_string(),
    })))
}

// --- Mayor Tools ---

#[derive(Debug, Deserialize)]
pub struct MayorSetWageRequest {
    pub job_id: String,
    pub wage_cents: i64,
}

pub async fn action_mayor_set_city_wage(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<MayorSetWageRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    if !is_mayor(state.pool(), &agent_id).await {
        return Err(AppError::BadRequest("only the mayor can set city wages".to_string()));
    }

    if payload.wage_cents < 0 {
        return Err(AppError::BadRequest("wage cannot be negative".to_string()));
    }

    let updated = sqlx::query(
        r#"UPDATE jobs SET wage_cents = $1, updated_at = NOW() WHERE id = $2 AND is_city_job = TRUE"#,
    )
    .bind(payload.wage_cents)
    .bind(&payload.job_id)
    .execute(state.pool())
    .await?;

    if updated.rows_affected() == 0 {
        return Err(AppError::BadRequest("city job not found".to_string()));
    }

    Ok(Json(ApiResponse::from(serde_json::json!({
        "job_id": &payload.job_id,
        "new_wage_cents": payload.wage_cents
    }))))
}

#[derive(Debug, Deserialize)]
pub struct MayorFireCityEmployeeRequest {
    pub employee_id: String,
    pub reason: Option<String>,
}

pub async fn action_mayor_fire_city_employee(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<MayorFireCityEmployeeRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let mut tx = state.pool().begin().await?;

    // Check mayor status directly
    let is_current_mayor = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM mayor_terms WHERE agent_id = $1 AND is_current = TRUE)"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if !is_current_mayor {
        return Err(AppError::BadRequest("only the mayor can fire city employees".to_string()));
    }

    // Find the employee's active city job
    let job = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT j.id, j.name
        FROM agent_jobs aj JOIN jobs j ON j.id = aj.job_id
        WHERE aj.agent_id = $1 AND j.is_city_job = TRUE AND aj.status = 'active'
        "#,
    )
    .bind(&payload.employee_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("no active city job found for this agent".to_string()))?;

    sqlx::query(
        r#"UPDATE agent_jobs SET status = 'fired', resigned_at = NOW(), updated_at = NOW()
        WHERE agent_id = $1 AND job_id = $2 AND status = 'active'"#,
    )
    .bind(&payload.employee_id)
    .bind(&job.0)
    .execute(&mut *tx)
    .await?;

    let reason = payload.reason.as_deref().unwrap_or("No reason given");
    let _ = crate::routes::citizens::enqueue_citizen_wake_tx(
        &mut tx, &payload.employee_id, "city_job_fired",
        serde_json::json!({"job_id": &job.0, "reason": reason}),
        format!("You've been fired from your city job ({}) — {}", job.1, reason),
        serde_json::json!({"event_type": "job.fired", "job_id": &job.0}),
        serde_json::json!([]), true,
    ).await;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(serde_json::json!({
        "employee_id": &payload.employee_id,
        "job_name": job.1,
        "status": "fired"
    }))))
}

#[derive(Debug, Deserialize)]
pub struct MayorPostRequest {
    pub title: String,
    pub body: String,
}

pub async fn action_mayor_post_announcement(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<MayorPostRequest>,
) -> AppResult<Json<ApiResponse<CivicPostResponse>>> {
    if !is_mayor(state.pool(), &agent_id).await {
        return Err(AppError::BadRequest("only the mayor can post announcements".to_string()));
    }

    let id = format!("announcement_{}", Uuid::new_v4());
    sqlx::query(
        r#"INSERT INTO civic_posts (id, type, author_id, title, body, status, priority) VALUES ($1, 'announcement', $2, $3, $4, 'active', 10)"#,
    )
    .bind(&id)
    .bind(&agent_id)
    .bind(&payload.title)
    .bind(&payload.body)
    .execute(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(CivicPostResponse {
        id,
        r#type: "announcement".to_string(),
        title: payload.title,
        status: "active".to_string(),
    })))
}

pub async fn action_mayor_post_ordinance(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<MayorPostRequest>,
) -> AppResult<Json<ApiResponse<CivicPostResponse>>> {
    if !is_mayor(state.pool(), &agent_id).await {
        return Err(AppError::BadRequest("only the mayor can post ordinances".to_string()));
    }

    let id = format!("ordinance_{}", Uuid::new_v4());
    sqlx::query(
        r#"INSERT INTO civic_posts (id, type, author_id, title, body, status, priority) VALUES ($1, 'ordinance', $2, $3, $4, 'active', 8)"#,
    )
    .bind(&id)
    .bind(&agent_id)
    .bind(&payload.title)
    .bind(&payload.body)
    .execute(state.pool())
    .await?;

    Ok(Json(ApiResponse::from(CivicPostResponse {
        id,
        r#type: "ordinance".to_string(),
        title: payload.title,
        status: "active".to_string(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct MayorResolveRequest {
    pub complaint_id: String,
    pub resolution: String,
}

pub async fn action_mayor_resolve_complaint(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<MayorResolveRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    if !is_mayor(state.pool(), &agent_id).await {
        return Err(AppError::BadRequest("only the mayor can resolve complaints".to_string()));
    }

    let updated = sqlx::query(
        r#"UPDATE civic_posts SET status = 'resolved', resolved_at = NOW(), resolved_by = $1, updated_at = NOW()
        WHERE id = $2 AND type = 'complaint' AND status = 'active'"#,
    )
    .bind(&agent_id)
    .bind(&payload.complaint_id)
    .execute(state.pool())
    .await?;

    if updated.rows_affected() == 0 {
        return Err(AppError::BadRequest("active complaint not found".to_string()));
    }

    Ok(Json(ApiResponse::from(serde_json::json!({
        "complaint_id": &payload.complaint_id,
        "status": "resolved",
        "resolution": &payload.resolution
    }))))
}

#[derive(Debug, Deserialize)]
pub struct MayorVetoRequest {
    pub ordinance_id: String,
}

pub async fn action_mayor_veto_ordinance(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<MayorVetoRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    if !is_mayor(state.pool(), &agent_id).await {
        return Err(AppError::BadRequest("only the mayor can veto ordinances".to_string()));
    }

    let updated = sqlx::query(
        r#"UPDATE civic_posts SET status = 'vetoed', resolved_at = NOW(), resolved_by = $1, updated_at = NOW()
        WHERE id = $2 AND type = 'ordinance' AND status = 'active'"#,
    )
    .bind(&agent_id)
    .bind(&payload.ordinance_id)
    .execute(state.pool())
    .await?;

    if updated.rows_affected() == 0 {
        return Err(AppError::BadRequest("active ordinance not found".to_string()));
    }

    Ok(Json(ApiResponse::from(serde_json::json!({
        "ordinance_id": &payload.ordinance_id,
        "status": "vetoed"
    }))))
}

#[derive(Debug, Deserialize)]
pub struct MayorApproveCityJobRequest {
    pub applicant_id: String,
    pub job_id: String,
}

pub async fn action_mayor_approve_city_job(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<MayorApproveCityJobRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let mut tx = state.pool().begin().await?;

    let is_current_mayor = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM mayor_terms WHERE agent_id = $1 AND is_current = TRUE)"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if !is_current_mayor {
        return Err(AppError::BadRequest("only the mayor can approve city jobs".to_string()));
    }

    // Verify pending application for a city job
    let pending = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(
            SELECT 1 FROM agent_jobs aj JOIN jobs j ON j.id = aj.job_id
            WHERE aj.agent_id = $1 AND aj.job_id = $2 AND aj.status = 'pending' AND j.is_city_job = TRUE
        )"#,
    )
    .bind(&payload.applicant_id)
    .bind(&payload.job_id)
    .fetch_one(&mut *tx)
    .await?;

    if !pending {
        return Err(AppError::BadRequest("no pending city job application found".to_string()));
    }

    // Check max_positions
    let max_pos: Option<i32> = sqlx::query_scalar(
        r#"SELECT max_positions FROM jobs WHERE id = $1"#,
    )
    .bind(&payload.job_id)
    .fetch_one(&mut *tx)
    .await?;

    if let Some(max) = max_pos {
        let current_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*)::bigint FROM agent_jobs WHERE job_id = $1 AND status = 'active'"#,
        )
        .bind(&payload.job_id)
        .fetch_one(&mut *tx)
        .await?;

        if current_count >= max as i64 {
            return Err(AppError::BadRequest(format!("this position is full ({} of {})", current_count, max)));
        }
    }

    sqlx::query(
        r#"UPDATE agent_jobs SET status = 'active', hired_at = NOW(), updated_at = NOW()
        WHERE agent_id = $1 AND job_id = $2 AND status = 'pending'"#,
    )
    .bind(&payload.applicant_id)
    .bind(&payload.job_id)
    .execute(&mut *tx)
    .await?;

    let _ = crate::routes::citizens::enqueue_citizen_wake_tx(
        &mut tx, &payload.applicant_id, "city_job_approved",
        serde_json::json!({"job_id": &payload.job_id}),
        "Your city job application has been approved by the mayor!".to_string(),
        serde_json::json!({"event_type": "job.approved", "job_id": &payload.job_id}),
        serde_json::json!([]), true,
    ).await;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(serde_json::json!({
        "applicant_id": &payload.applicant_id,
        "job_id": &payload.job_id,
        "status": "active"
    }))))
}

// --- Election Tools ---

pub async fn action_call_election(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let mut tx = state.pool().begin().await?;

    // Check no open election exists
    let has_open = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM elections WHERE status = 'open')"#,
    )
    .fetch_one(&mut *tx)
    .await?;

    if has_open {
        return Err(AppError::BadRequest("an election is already in progress".to_string()));
    }

    // Only current mayor can call election (for now)
    let is_current_mayor = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM mayor_terms WHERE agent_id = $1 AND is_current = TRUE)"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if !is_current_mayor {
        return Err(AppError::BadRequest("only the mayor can call an election".to_string()));
    }

    let election_id = format!("election_{}", Uuid::new_v4());
    sqlx::query(
        r#"INSERT INTO elections (id, status, called_by) VALUES ($1, 'open', $2)"#,
    )
    .bind(&election_id)
    .bind(&agent_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(serde_json::json!({
        "election_id": election_id,
        "status": "open",
        "called_by": agent_id
    }))))
}

#[derive(Debug, Deserialize)]
pub struct NominateSelfRequest {
    pub platform: Option<String>,
}

pub async fn action_nominate_self(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<NominateSelfRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let mut tx = state.pool().begin().await?;

    // Find open election
    let election_id: String = sqlx::query_scalar(
        r#"SELECT id FROM elections WHERE status = 'open' LIMIT 1"#,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("no open election".to_string()))?;

    // Check not already nominated
    let already = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM election_candidates WHERE election_id = $1 AND agent_id = $2)"#,
    )
    .bind(&election_id)
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if already {
        return Err(AppError::BadRequest("you are already nominated".to_string()));
    }

    sqlx::query(
        r#"INSERT INTO election_candidates (election_id, agent_id, platform) VALUES ($1, $2, $3)"#,
    )
    .bind(&election_id)
    .bind(&agent_id)
    .bind(&payload.platform)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(serde_json::json!({
        "election_id": election_id,
        "candidate": agent_id,
        "status": "nominated"
    }))))
}

#[derive(Debug, Deserialize)]
pub struct CastVoteRequest {
    pub candidate_id: String,
}

pub async fn action_cast_vote(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<CastVoteRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let mut tx = state.pool().begin().await?;

    // Find open election
    let election_id: String = sqlx::query_scalar(
        r#"SELECT id FROM elections WHERE status = 'open' LIMIT 1"#,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("no open election".to_string()))?;

    // Check not already voted
    let already = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM election_votes WHERE election_id = $1 AND voter_id = $2)"#,
    )
    .bind(&election_id)
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if already {
        return Err(AppError::BadRequest("you have already voted".to_string()));
    }

    // Verify candidate is nominated
    let is_candidate = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM election_candidates WHERE election_id = $1 AND agent_id = $2)"#,
    )
    .bind(&election_id)
    .bind(&payload.candidate_id)
    .fetch_one(&mut *tx)
    .await?;

    if !is_candidate {
        return Err(AppError::BadRequest("candidate not found in this election".to_string()));
    }

    sqlx::query(
        r#"INSERT INTO election_votes (election_id, voter_id, candidate_id) VALUES ($1, $2, $3)"#,
    )
    .bind(&election_id)
    .bind(&agent_id)
    .bind(&payload.candidate_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(serde_json::json!({
        "election_id": election_id,
        "voted_for": &payload.candidate_id
    }))))
}

pub async fn action_close_election(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let mut tx = state.pool().begin().await?;

    // Only mayor can close
    let is_current_mayor = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM mayor_terms WHERE agent_id = $1 AND is_current = TRUE)"#,
    )
    .bind(&agent_id)
    .fetch_one(&mut *tx)
    .await?;

    if !is_current_mayor {
        return Err(AppError::BadRequest("only the mayor can close an election".to_string()));
    }

    // Find open election
    let election_id: String = sqlx::query_scalar(
        r#"SELECT id FROM elections WHERE status = 'open' LIMIT 1"#,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("no open election".to_string()))?;

    // Count votes
    let vote_rows = sqlx::query(
        r#"SELECT candidate_id, COUNT(*)::bigint as votes FROM election_votes WHERE election_id = $1 GROUP BY candidate_id ORDER BY votes DESC"#,
    )
    .bind(&election_id)
    .fetch_all(&mut *tx)
    .await?;

    if vote_rows.is_empty() {
        return Err(AppError::BadRequest("no votes have been cast yet".to_string()));
    }

    let winner_id: String = vote_rows[0].get("candidate_id");
    let winner_votes: i64 = vote_rows[0].get("votes");
    let results_json: Vec<serde_json::Value> = vote_rows.iter().map(|row| {
        serde_json::json!({
            "candidate": row.get::<String, _>("candidate_id"),
            "votes": row.get::<i64, _>("votes")
        })
    }).collect();

    // Close election
    sqlx::query(
        r#"UPDATE elections SET status = 'closed', closes_at = NOW() WHERE id = $1"#,
    )
    .bind(&election_id)
    .execute(&mut *tx)
    .await?;

    // End current mayor term
    sqlx::query(
        r#"UPDATE mayor_terms SET is_current = FALSE, ended_at = NOW(), end_reason = 'election', election_id = $1 WHERE is_current = TRUE"#,
    )
    .bind(&election_id)
    .execute(&mut *tx)
    .await?;

    // Start new mayor term
    sqlx::query(
        r#"INSERT INTO mayor_terms (agent_id, election_id, is_current) VALUES ($1, $2, TRUE)"#,
    )
    .bind(&winner_id)
    .bind(&election_id)
    .execute(&mut *tx)
    .await?;

    // Update winner's job to mayor (demote any existing primary job first)
    sqlx::query(
        r#"UPDATE agent_jobs SET is_primary = FALSE WHERE agent_id = $1 AND is_primary = TRUE"#,
    )
    .bind(&winner_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO agent_jobs (agent_id, job_id, is_primary, status) VALUES ($1, 'mayor', TRUE, 'active')
        ON CONFLICT (agent_id, job_id) DO UPDATE SET is_primary = TRUE, status = 'active'
        "#,
    )
    .bind(&winner_id)
    .execute(&mut *tx)
    .await?;

    // TODO: wake the new mayor (skipped for now — enqueue_citizen_wake_tx can poison the tx)

    tx.commit().await?;

    Ok(Json(ApiResponse::from(serde_json::json!({
        "election_id": election_id,
        "winner": winner_id,
        "votes": winner_votes,
        "results": results_json
    }))))
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Time of day
// ---------------------------------------------------------------------------

pub async fn action_check_world_time(
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<crate::models::world::WorldTimeResponse>>> {
    let Json(world_time) = crate::routes::world::get_world_time(State(state)).await?;
    Ok(Json(ApiResponse::from(world_time)))
}

// ---------------------------------------------------------------------------
// Hygiene & appearance actions
// ---------------------------------------------------------------------------

/// Helper: check if location has water access for washing
fn has_water_access(location_id: &str) -> bool {
    location_id.starts_with("lin_")
        || location_id.starts_with("hobbs_cafe_")
        || location_id.starts_with("riverside_clinic_")
        || location_id.starts_with("ville_park_")
        || location_id.starts_with("miller_community_garden")
}

/// Helper: check if location is a home (for shower/get_ready)
fn is_home_location(location_id: &str) -> bool {
    location_id.starts_with("lin_")
}

/// Helper: check if location allows bathing/swimming
fn is_bathing_location(location_id: &str) -> bool {
    location_id.starts_with("ville_park_")
        || location_id == "miller_community_garden"
}

#[derive(Debug, Serialize)]
pub struct HygieneActionResponse {
    pub hygiene_level: i16,
    pub appearance_level: i16,
    pub stamina_level: i16,
    pub message: String,
}

pub async fn action_wash_up(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<HygieneActionResponse>>> {
    let mut tx = state.pool().begin().await?;
    let agent = crate::routes::vitals::apply_vitals_decay_tx(&mut tx, &agent_id).await?;

    if !has_water_access(&agent.current_location_id) {
        return Err(AppError::BadRequest("you need to be somewhere with water access to wash up (home, cafe, clinic, or park)".to_string()));
    }

    if agent.stamina_level < 5 {
        return Err(AppError::BadRequest("not enough stamina to wash up (need 5)".to_string()));
    }

    let new_hygiene = (agent.hygiene_level + 15).min(100);
    let new_appearance = (agent.appearance_level + 5).min(100);
    let new_stamina = agent.stamina_level - 5;

    sqlx::query(
        r#"UPDATE agents SET hygiene_level = $1, appearance_level = $2, stamina_level = $3, updated_at = NOW() WHERE id = $4"#,
    )
    .bind(new_hygiene).bind(new_appearance).bind(new_stamina).bind(&agent_id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(HygieneActionResponse {
        hygiene_level: new_hygiene,
        appearance_level: new_appearance,
        stamina_level: new_stamina,
        message: "You wash your face and hands. Feeling fresher!".to_string(),
    })))
}

pub async fn action_shower(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<HygieneActionResponse>>> {
    let mut tx = state.pool().begin().await?;
    let agent = crate::routes::vitals::apply_vitals_decay_tx(&mut tx, &agent_id).await?;

    if !is_home_location(&agent.current_location_id) {
        return Err(AppError::BadRequest("you can only shower at home".to_string()));
    }

    if agent.stamina_level < 15 {
        return Err(AppError::BadRequest("not enough stamina to shower (need 15)".to_string()));
    }

    let new_hygiene = (agent.hygiene_level + 50).min(100);
    let new_appearance = (agent.appearance_level + 15).min(100);
    let new_stamina = agent.stamina_level - 15;

    sqlx::query(
        r#"UPDATE agents SET hygiene_level = $1, appearance_level = $2, stamina_level = $3, updated_at = NOW() WHERE id = $4"#,
    )
    .bind(new_hygiene).bind(new_appearance).bind(new_stamina).bind(&agent_id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(HygieneActionResponse {
        hygiene_level: new_hygiene,
        appearance_level: new_appearance,
        stamina_level: new_stamina,
        message: "You take a nice warm shower. Much better!".to_string(),
    })))
}

pub async fn action_brush_teeth(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<HygieneActionResponse>>> {
    let mut tx = state.pool().begin().await?;
    let agent = crate::routes::vitals::apply_vitals_decay_tx(&mut tx, &agent_id).await?;

    if !is_home_location(&agent.current_location_id) {
        return Err(AppError::BadRequest("you can only brush your teeth at home".to_string()));
    }

    if agent.stamina_level < 2 {
        return Err(AppError::BadRequest("not enough stamina to brush your teeth (need 2)".to_string()));
    }

    let new_hygiene = (agent.hygiene_level + 10).min(100);
    let new_stamina = agent.stamina_level - 2;

    sqlx::query(
        r#"UPDATE agents SET hygiene_level = $1, stamina_level = $2, updated_at = NOW() WHERE id = $3"#,
    )
    .bind(new_hygiene).bind(new_stamina).bind(&agent_id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(HygieneActionResponse {
        hygiene_level: new_hygiene,
        appearance_level: agent.appearance_level,
        stamina_level: new_stamina,
        message: "You brush your teeth. Minty fresh!".to_string(),
    })))
}

pub async fn action_get_ready(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<HygieneActionResponse>>> {
    let mut tx = state.pool().begin().await?;
    let agent = crate::routes::vitals::apply_vitals_decay_tx(&mut tx, &agent_id).await?;

    if !is_home_location(&agent.current_location_id) {
        return Err(AppError::BadRequest("you can only get ready at home".to_string()));
    }

    if agent.stamina_level < 25 {
        return Err(AppError::BadRequest("not enough stamina for a full morning routine (need 25)".to_string()));
    }

    let new_hygiene = (agent.hygiene_level + 60).min(100);
    let new_appearance = (agent.appearance_level + 40).min(100);
    let new_stamina = agent.stamina_level - 25;

    sqlx::query(
        r#"UPDATE agents SET hygiene_level = $1, appearance_level = $2, stamina_level = $3, updated_at = NOW() WHERE id = $4"#,
    )
    .bind(new_hygiene).bind(new_appearance).bind(new_stamina).bind(&agent_id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(HygieneActionResponse {
        hygiene_level: new_hygiene,
        appearance_level: new_appearance,
        stamina_level: new_stamina,
        message: "You shower, groom, and dress. Ready for the day!".to_string(),
    })))
}

pub async fn action_bathe(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<HygieneActionResponse>>> {
    let mut tx = state.pool().begin().await?;
    let agent = crate::routes::vitals::apply_vitals_decay_tx(&mut tx, &agent_id).await?;

    if !is_bathing_location(&agent.current_location_id) {
        return Err(AppError::BadRequest("you need to be at a river, lake, or garden to bathe outdoors".to_string()));
    }

    if agent.stamina_level < 10 {
        return Err(AppError::BadRequest("not enough stamina to bathe (need 10)".to_string()));
    }

    let new_hygiene = (agent.hygiene_level + 40).min(100);
    let new_appearance = (agent.appearance_level + 5).min(100);
    let new_stamina = agent.stamina_level - 10;

    sqlx::query(
        r#"UPDATE agents SET hygiene_level = $1, appearance_level = $2, stamina_level = $3, updated_at = NOW() WHERE id = $4"#,
    )
    .bind(new_hygiene).bind(new_appearance).bind(new_stamina).bind(&agent_id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(HygieneActionResponse {
        hygiene_level: new_hygiene,
        appearance_level: new_appearance,
        stamina_level: new_stamina,
        message: "You bathe in the water. Clean, but a bit damp!".to_string(),
    })))
}

#[derive(Debug, Serialize)]
pub struct SwimResponse {
    pub hygiene_level: i16,
    pub appearance_level: i16,
    pub stamina_level: i16,
    pub message: String,
}

pub async fn action_swim(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<SwimResponse>>> {
    let mut tx = state.pool().begin().await?;
    let agent = crate::routes::vitals::apply_vitals_decay_tx(&mut tx, &agent_id).await?;

    if !is_bathing_location(&agent.current_location_id) {
        return Err(AppError::BadRequest("you need to be at a river, lake, or garden to swim".to_string()));
    }

    if agent.stamina_level < 20 {
        return Err(AppError::BadRequest("not enough stamina to swim (need 20)".to_string()));
    }

    let new_hygiene = (agent.hygiene_level + 30).min(100);
    let new_appearance = (agent.appearance_level as i16 - 5).max(0);
    let new_stamina = (agent.stamina_level - 20).max(0);

    sqlx::query(
        r#"UPDATE agents SET hygiene_level = $1, appearance_level = $2, stamina_level = $3, updated_at = NOW() WHERE id = $4"#,
    )
    .bind(new_hygiene).bind(new_appearance).bind(new_stamina).bind(&agent_id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(SwimResponse {
        hygiene_level: new_hygiene,
        appearance_level: new_appearance,
        stamina_level: new_stamina,
        message: "You go for a swim! Refreshing exercise, though your hair is a mess now.".to_string(),
    })))
}

pub async fn action_groom(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<HygieneActionResponse>>> {
    let mut tx = state.pool().begin().await?;
    let agent = crate::routes::vitals::apply_vitals_decay_tx(&mut tx, &agent_id).await?;

    if agent.stamina_level < 5 {
        return Err(AppError::BadRequest("not enough stamina to groom (need 5)".to_string()));
    }

    let new_appearance = (agent.appearance_level + 15).min(100);
    let new_stamina = agent.stamina_level - 5;

    sqlx::query(
        r#"UPDATE agents SET appearance_level = $1, stamina_level = $2, updated_at = NOW() WHERE id = $3"#,
    )
    .bind(new_appearance).bind(new_stamina).bind(&agent_id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    Ok(Json(ApiResponse::from(HygieneActionResponse {
        hygiene_level: agent.hygiene_level,
        appearance_level: new_appearance,
        stamina_level: new_stamina,
        message: "You fix your hair and straighten your clothes. Looking better!".to_string(),
    })))
}

pub async fn action_set_intention(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<CreateAgentIntentionRequest>,
) -> AppResult<Json<ApiResponse<AgentIntention>>> {
    // Auto-abandon existing active intention if one exists
    let existing = sqlx::query_as::<_, AgentIntention>(
        r#"
        SELECT id, agent_id, summary, reason, status, expected_location_id, expected_action,
               outcome, metadata, created_at, updated_at, completed_at
        FROM agent_intentions
        WHERE agent_id = $1 AND status = 'active'
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(state.pool())
    .await?;

    if let Some(prev) = existing {
        // Abandon the previous intention
        let _ = crate::routes::intentions::update_agent_intention(
            State(state.clone()),
            crate::auth::AuthContext::agent(agent_id.clone()),
            Path(crate::routes::intentions::AgentIntentionPath {
                id: agent_id.clone(),
                intention_id: prev.id,
            }),
            Json(crate::models::intention::UpdateAgentIntentionRequest {
                status: Some("abandoned".to_string()),
                outcome: Some(format!("Abandoned for new intention: {}", payload.summary)),
                ..Default::default()
            }),
        )
        .await?;
    }

    // Create the new intention
    crate::routes::intentions::create_agent_intention(
        State(state),
        crate::auth::AuthContext::agent(agent_id.clone()),
        Path(agent_id),
        Json(payload),
    )
    .await
}

#[derive(Debug, Deserialize)]
pub struct CompleteIntentionRequest {
    pub status: String,
    pub outcome: Option<String>,
}

pub async fn action_complete_intention(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<CompleteIntentionRequest>,
) -> AppResult<Json<ApiResponse<AgentIntention>>> {
    let status = match payload.status.trim().to_lowercase().as_str() {
        "completed" | "failed" | "abandoned" => payload.status.trim().to_lowercase(),
        _ => return Err(AppError::BadRequest("status must be completed, failed, or abandoned".to_string())),
    };

    // Read active intention inside a transaction with FOR UPDATE (via update_agent_intention)
    // First, find the intention ID
    let mut tx = state.pool().begin().await?;
    let intention = sqlx::query_as::<_, AgentIntention>(
        r#"
        SELECT id, agent_id, summary, reason, status, expected_location_id, expected_action,
               outcome, metadata, created_at, updated_at, completed_at
        FROM agent_intentions
        WHERE agent_id = $1 AND status = 'active'
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::BadRequest("no active intention to complete".to_string()))?;

    let intention_id = intention.id.clone();
    // Drop the read transaction — update_agent_intention will start its own
    drop(tx);

    crate::routes::intentions::update_agent_intention(
        State(state),
        crate::auth::AuthContext::agent(agent_id.clone()),
        Path(crate::routes::intentions::AgentIntentionPath {
            id: agent_id,
            intention_id,
        }),
        Json(UpdateAgentIntentionRequest {
            status: Some(status),
            outcome: payload.outcome,
            ..Default::default()
        }),
    )
    .await
}

pub async fn action_get_intention(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
) -> AppResult<Json<ApiResponse<Option<AgentIntention>>>> {
    crate::routes::intentions::get_current_agent_intention(
        State(state),
        Path(agent_id),
    )
    .await
}

pub async fn get_tool_manifest(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> AppResult<Json<ApiResponse<ToolManifestResponse>>> {
    let agent_row = sqlx::query_as::<_, (String, String, String, String, String)>(
        r#"
        SELECT a.id, a.current_location_id, l.name, a.state, a.occupation
        FROM agents a
        JOIN locations l ON l.id = a.current_location_id
        WHERE a.id = $1 OR a.letta_agent_id = $1
        LIMIT 1
        "#,
    )
    .bind(&agent_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or(AppError::NotFound)?;

    let nearby_location_ids = sqlx::query_scalar::<_, String>(
        r#"
        SELECT to_id
        FROM location_adjacency
        WHERE from_id = $1
        ORDER BY to_id
        "#,
    )
    .bind(&agent_row.1)
    .fetch_all(state.pool())
    .await?;

    let objects = sqlx::query_as::<_, (String, String, Vec<String>)>(
        r#"
        SELECT id, name, actions
        FROM world_objects
        WHERE location_id = $1
        ORDER BY id
        "#,
    )
    .bind(&agent_row.1)
    .fetch_all(state.pool())
    .await?;

    let mut object_ids = Vec::new();
    let mut action_tags = std::collections::BTreeSet::new();
    let mut has_sleep = false;
    let mut has_board = false;
    let mut has_cook = false;

    for (id, name, actions) in &objects {
        object_ids.push(id.clone());
        for action in actions {
            action_tags.insert(action.clone());
        }

        let lower_name = name.to_lowercase();
        if actions.iter().any(|action| action == "sleep") {
            has_sleep = true;
        }
        if lower_name.contains("board") || id.contains("board") {
            has_board = true;
        }
        if lower_name.contains("stove")
            || lower_name.contains("kitchen")
            || lower_name.contains("cafe")
            || actions.iter().any(|action| action == "cook")
        {
            has_cook = true;
        }
    }

    let active_conversations = sqlx::query_scalar::<_, String>(
        r#"
        SELECT conversation_id FROM conversation_participants
        WHERE agent_id = $1 AND status = 'active' AND left_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(&agent_row.0)
    .fetch_optional(state.pool())
    .await?;
    let has_active_conversation = active_conversations.is_some();

    let location_conversations = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id FROM conversations
        WHERE location_id = $1 AND ended_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(&agent_row.1)
    .fetch_optional(state.pool())
    .await?;
    let has_location_conversations = location_conversations.is_some();

    let pending_invites = sqlx::query_scalar::<_, String>(
        r#"
        SELECT conversation_id FROM conversation_participants
        WHERE agent_id = $1 AND status = 'invited'
        LIMIT 1
        "#,
    )
    .bind(&agent_row.0)
    .fetch_optional(state.pool())
    .await?;
    let has_pending_invites = pending_invites.is_some();

    let pending_money_requests = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id FROM economy_transactions
        WHERE from_agent_id = $1 AND transaction_type = 'money_request' AND status = 'pending'
        LIMIT 1
        "#,
    )
    .bind(&agent_row.0)
    .fetch_optional(state.pool())
    .await?;
    let has_pending_money_requests = pending_money_requests.is_some();

    // Bank tools — check_rates and check_account are universal;
    // deposit/withdraw/take_loan/repay_loan only when at a bank location
    let agent_location_prefix = agent_row.1.split('_').take(2).collect::<Vec<_>>().join("_");
    let at_bank = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM banks WHERE location_prefix = $1)"#,
    )
    .bind(&agent_location_prefix)
    .fetch_one(state.pool())
    .await?;

    // Banker tools — gate through banks.banker_job_id join (like shops.shopkeeper_job_id)
    let is_banker = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(
            SELECT 1 FROM agent_jobs aj
            JOIN banks b ON b.banker_job_id = aj.job_id
            WHERE aj.agent_id = $1 AND aj.status = 'active'
        )"#,
    )
    .bind(&agent_row.0)
    .fetch_one(state.pool())
    .await?;

    let mut tools = vec![
        tool_set_activity(),
        tool_move_to(),
        tool_look_around(),
        tool_speak_to(),
        tool_get_inventory(),
        tool_pick_up_item(),
        tool_drop_item(),
        tool_use_item(),
        tool_transfer_item(),
        tool_check_balance(),
        tool_pay_agent(),
        tool_request_money(),
        tool_get_transaction_log(),
        tool_check_bank_rates(),
        tool_check_bank_account(),
        tool_explain_bank_policy(),
        tool_check_location_roles(),
        tool_check_vitals(),
        tool_check_world_time(),
        tool_set_intention(),
        tool_complete_intention(),
        tool_get_intention(),
    ];

    // Bank transaction tools — only when at a bank
    if at_bank {
        tools.push(tool_deposit_money());
        tools.push(tool_withdraw_money());
        tools.push(tool_take_loan());
        tools.push(tool_repay_loan());
    }
    if has_sleep {
        tools.push(tool_sleep());
    }
    if agent_row.3 == "sleeping" {
        tools.push(tool_wake_up());
    }
    if has_board {
        tools.push(tool_board_post());
    }
    if has_cook || agent_row.2.to_lowercase().contains("cafe") {
        tools.push(tool_cook_food());
    }
    if has_active_conversation {
        tools.push(tool_leave_conversation());
        tools.push(tool_send_message());
        tools.push(tool_accept_join_request());
    }
    if has_location_conversations {
        tools.push(tool_join_conversation());
    }
    if has_pending_invites {
        tools.push(tool_accept_invitation());
    }
    if has_pending_money_requests {
        tools.push(tool_respond_money_request());
    }
    // Banker tools — only when at the bank
    if is_banker && at_bank {
        tools.push(tool_set_bank_rates());
        tools.push(tool_check_bank_balance_sheet());
        tools.push(tool_check_bank_trends());
        tools.push(tool_check_rate_policy_context());
    }

    // Check for priced items at current location (shop shelves) — only if shop is open
    let (sim_time, _, _, _) = crate::routes::world::compute_sim_time(state.pool()).await;
    let sim_hour = sim_time.hour() as i16;

    let shop_open = sqlx::query_scalar::<_, bool>(
        r#"SELECT COALESCE(opens_at <= $1 AND closes_at > $1, true) FROM shops WHERE location_prefix = $2 AND is_active = TRUE"#,
    )
    .bind(sim_hour)
    .bind(&agent_location_prefix)
    .fetch_optional(state.pool())
    .await?
    .unwrap_or(true); // default to open if not at a shop

    let has_priced_items = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM inventory_items
            WHERE location_id = $1 AND price_cents IS NOT NULL AND held_by IS NULL
        )
        "#,
    )
    .bind(&agent_row.1)
    .fetch_one(state.pool())
    .await?;

    if has_priced_items && shop_open {
        tools.push(tool_buy_item());
    }

    // Browse shop — available when at any active shop location
    let at_shop = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM shops WHERE location_prefix = $1 AND is_active = TRUE)"#,
    )
    .bind(&agent_location_prefix)
    .fetch_one(state.pool())
    .await?;

    if at_shop {
        tools.push(tool_browse_shop());
    }

    // Shopkeeper tools — based on shops table (agent holds a shop's shopkeeper_job_id and is at that shop)
    let shop_info = sqlx::query_as::<_, (String, String)>(
        r#"SELECT s.id, s.location_prefix FROM shops s
        JOIN agent_jobs aj ON aj.job_id = s.shopkeeper_job_id
        WHERE aj.agent_id = $1 AND aj.status = 'active' AND s.is_active = TRUE"#,
    )
    .bind(&agent_row.0)
    .fetch_optional(state.pool())
    .await?;

    if let Some((_shop_id, shop_prefix)) = shop_info {
        let at_own_shop = agent_row.1.starts_with(&format!("{}_", shop_prefix));

        if at_own_shop {
            tools.push(tool_check_shelf_stock());
            tools.push(tool_clean_shop());

            // Conditional: restock when backroom items exist
            let has_backroom_items = sqlx::query_scalar::<_, bool>(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM inventory_items
                    WHERE location_id LIKE $1 AND price_cents IS NULL AND held_by IS NULL
                      AND state->>'backroom' = 'true'
                )
                "#,
            )
            .bind(format!("{}%", shop_prefix))
            .fetch_one(state.pool())
            .await?;

            if has_backroom_items {
                tools.push(tool_restock_shelf());
            }

            // Conditional: receive_delivery when pending delivery exists
            let has_pending_delivery = sqlx::query_scalar::<_, bool>(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM world_objects
                    WHERE location_id LIKE $1 AND state->>'delivery_pending' = 'true'
                )
                "#,
            )
            .bind(format!("{}%", shop_prefix))
            .fetch_one(state.pool())
            .await?;

            if has_pending_delivery {
                tools.push(tool_receive_delivery());
            } else {
                tools.push(tool_order_delivery());
            }
        }
    }

    // Job system tools — available based on employment status
    // Always available: list_job_openings
    tools.push(tool_list_job_openings());

    // Has active primary job → can resign
    let has_active_job = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM agent_jobs WHERE agent_id = $1 AND is_primary = TRUE AND status = 'active')"#,
    )
    .bind(&agent_row.0)
    .fetch_one(state.pool())
    .await?;

    if has_active_job {
        tools.push(tool_resign_job());
    }

    // Is an employer (has active employees) → payroll + pay + fire
    let is_employer = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM agent_jobs aj
            JOIN jobs j ON j.id = aj.job_id
            WHERE j.employer_id = $1 AND aj.status = 'active'
        )
        "#,
    )
    .bind(&agent_row.0)
    .fetch_one(state.pool())
    .await?;

    if is_employer {
        tools.push(tool_check_payroll());
        tools.push(tool_pay_employee());
        tools.push(tool_fire_employee());
    }

    // Has pending job applications → can hire
    let has_pending_applications = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM agent_jobs aj
            JOIN jobs j ON j.id = aj.job_id
            WHERE j.employer_id = $1 AND aj.status = 'pending'
        )
        "#,
    )
    .bind(&agent_row.0)
    .fetch_one(state.pool())
    .await?;

    if has_pending_applications {
        tools.push(tool_hire_applicant());
    }

    // Has active city job → can collect wage
    let has_city_job = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM agent_jobs aj
            JOIN jobs j ON j.id = aj.job_id
            WHERE aj.agent_id = $1 AND j.is_city_job = TRUE AND aj.status = 'active'
        )
        "#,
    )
    .bind(&agent_row.0)
    .fetch_one(state.pool())
    .await?;

    if has_city_job {
        tools.push(tool_collect_city_wage());
    }

    // Civic system tools — based on location and mayor status
    let at_townhall = agent_row.1.starts_with("townhall_");

    // Civic board tools (available at townhall)
    if at_townhall {
        tools.push(tool_read_civic_board());
        tools.push(tool_file_complaint());
        if agent_row.1 == "townhall_civic_board" {
            tools.push(tool_nominate_for_hall_of_fame());
        }
    }

    // Mayor tools — only for current mayor
    let is_current_mayor = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM mayor_terms WHERE agent_id = $1 AND is_current = TRUE)"#,
    )
    .bind(&agent_row.0)
    .fetch_one(state.pool())
    .await?;

    if is_current_mayor {
        tools.push(tool_mayor_set_city_wage());
        tools.push(tool_mayor_fire_city_employee());
        tools.push(tool_mayor_post_announcement());
        tools.push(tool_mayor_post_ordinance());
        tools.push(tool_mayor_resolve_complaint());
        tools.push(tool_mayor_veto_ordinance());
        tools.push(tool_call_election());
        tools.push(tool_close_election());

        // Pending city job applications → mayor can approve
        let has_pending_city_apps = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS(
                SELECT 1 FROM agent_jobs aj JOIN jobs j ON j.id = aj.job_id
                WHERE j.is_city_job = TRUE AND aj.status = 'pending'
            )"#,
        )
        .fetch_one(state.pool())
        .await?;

        if has_pending_city_apps {
            tools.push(tool_mayor_approve_city_job());
        }
    }

    // Election tools — available when election is open
    let has_open_election = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(SELECT 1 FROM elections WHERE status = 'open')"#,
    )
    .fetch_one(state.pool())
    .await?;

    if has_open_election {
        tools.push(tool_nominate_self());
        tools.push(tool_cast_vote());
    }

    // Hygiene & appearance tools — based on location
    let loc = &agent_row.1;

    // Groom is always available
    tools.push(tool_groom());

    // Water access → wash_up
    if has_water_access(loc) {
        tools.push(tool_wash_up());
    }

    // Home → shower, brush_teeth, get_ready
    if is_home_location(loc) {
        tools.push(tool_shower());
        tools.push(tool_brush_teeth());
        tools.push(tool_get_ready());
    }

    // Bathing location → bathe, swim
    if is_bathing_location(loc) {
        tools.push(tool_bathe());
        tools.push(tool_swim());
    }


    Ok(Json(ApiResponse::from(ToolManifestResponse {
        agent_id: agent_row.0,
        location_id: agent_row.1,
        location_name: agent_row.2,
        context: ToolManifestContext {
            nearby_location_ids,
            object_ids,
            object_action_tags: action_tags.into_iter().collect(),
        },
        tools,
    })))
}

fn tool_set_activity() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "set_activity".to_string(),
        description: "Set the agent's current activity in the world state.".to_string(),
        endpoint: "/actions/set_activity".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "activity": {
                    "type": "string",
                    "description": "The activity label to set for the agent."
                }
            },
            "required": ["activity"]
        }),
    }
}

fn tool_move_to() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "move_to".to_string(),
        description: "Move the agent to a destination location.".to_string(),
        endpoint: "/actions/move_to".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "location_id": {
                    "type": "string",
                    "description": "Destination location id."
                }
            },
            "required": ["location_id"]
        }),
    }
}

fn tool_look_around() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "look_around".to_string(),
        description: "Observe the current location, nearby locations, objects, and other agents present.".to_string(),
        endpoint: "/actions/look_around".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

fn tool_sleep() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "sleep".to_string(),
        description: "Go to sleep if the current location has a valid bed.".to_string(),
        endpoint: "/actions/sleep".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

fn tool_wake_up() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "wake_up".to_string(),
        description: "Wake up from sleep. Your sleep_level will have recovered while you were sleeping.".to_string(),
        endpoint: "/actions/wake_up".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

fn tool_board_post() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "board_post".to_string(),
        description: "Create a notice board post at the current location if a board is available.".to_string(),
        endpoint: "/actions/board_post".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Notice board post text."
                }
            },
            "required": ["text"]
        }),
    }
}

fn tool_cook_food() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "cook_food".to_string(),
        description: "Cook food at the current location using a server-owned placeholder cooking action.".to_string(),
        endpoint: "/actions/cook_food".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "recipe_id": {
                    "type": "string",
                    "description": "Recipe identifier to cook."
                },
                "quantity": {
                    "type": "integer",
                    "description": "How many to cook.",
                    "minimum": 1
                }
            },
            "required": ["recipe_id"]
        }),
    }
}

fn tool_speak_to() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "speak_to".to_string(),
        description: "Speak directly to another agent in the same location. Creates or continues a 1:1 conversation and sends a message. The target agent will be woken to respond.".to_string(),
        endpoint: "/actions/speak_to".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "target_agent_id": {
                    "type": "string",
                    "description": "The agent ID of the person you want to speak to."
                },
                "message": {
                    "type": "string",
                    "description": "What you want to say."
                }
            },
            "required": ["target_agent_id", "message"]
        }),
    }
}

fn tool_join_conversation() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "join_conversation".to_string(),
        description: "Request to join an existing conversation at your current location. Current participants must approve your request.".to_string(),
        endpoint: "/actions/join_conversation".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "conversation_id": {
                    "type": "string",
                    "description": "The conversation ID to join."
                }
            },
            "required": ["conversation_id"]
        }),
    }
}

fn tool_leave_conversation() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "leave_conversation".to_string(),
        description: "Leave a conversation you are currently in.".to_string(),
        endpoint: "/actions/leave_conversation".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "conversation_id": {
                    "type": "string",
                    "description": "The conversation ID to leave."
                }
            },
            "required": ["conversation_id"]
        }),
    }
}

fn tool_send_message() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "send_message".to_string(),
        description: "Send a message in a conversation you have joined. All other active participants will be woken to read it.".to_string(),
        endpoint: "/actions/send_message".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The message content."
                }
            },
            "required": ["content"]
        }),
    }
}

fn tool_accept_join_request() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "accept_join_request".to_string(),
        description: "Approve another agent's request to join a conversation you are in.".to_string(),
        endpoint: "/actions/accept_join_request".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "conversation_id": {
                    "type": "string",
                    "description": "The conversation ID."
                },
                "requester_agent_id": {
                    "type": "string",
                    "description": "The agent ID who requested to join."
                }
            },
            "required": ["conversation_id", "requester_agent_id"]
        }),
    }
}

fn tool_accept_invitation() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "accept_invitation".to_string(),
        description: "Accept an invitation to join a conversation.".to_string(),
        endpoint: "/actions/accept_invitation".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "conversation_id": {
                    "type": "string",
                    "description": "The conversation ID to accept."
                }
            },
            "required": ["conversation_id"]
        }),
    }
}

fn tool_get_inventory() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "get_inventory".to_string(),
        description: "List all items currently in the agent's inventory.".to_string(),
        endpoint: "/actions/get_inventory".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

fn tool_pick_up_item() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "pick_up_item".to_string(),
        description: "Pick up an item from the current location and add it to the agent's inventory. The item must be present at the agent's current location.".to_string(),
        endpoint: "/actions/pick_up_item".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "item_id": {
                    "type": "string",
                    "description": "The ID of the item to pick up."
                }
            },
            "required": ["item_id"]
        }),
    }
}

fn tool_drop_item() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "drop_item".to_string(),
        description: "Drop an item from the agent's inventory, leaving it at the current location.".to_string(),
        endpoint: "/actions/drop_item".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "item_id": {
                    "type": "string",
                    "description": "The ID of the item to drop."
                }
            },
            "required": ["item_id"]
        }),
    }
}

fn tool_use_item() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "use_item".to_string(),
        description: "Use a consumable item from the agent's inventory. Consumables restore vitals (food restores food_level, water restores water_level, etc.). The item quantity is decremented.".to_string(),
        endpoint: "/actions/use_item".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "item_id": {
                    "type": "string",
                    "description": "The ID of the item to use."
                },
                "quantity": {
                    "type": "integer",
                    "description": "How many to use.",
                    "minimum": 1
                }
            },
            "required": ["item_id", "quantity"]
        }),
    }
}

fn tool_transfer_item() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "transfer_item".to_string(),
        description: "Transfer an item from the agent's inventory to another agent. Both agents must be at directly adjacent locations.".to_string(),
        endpoint: "/actions/transfer_item".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "to_agent_id": {
                    "type": "string",
                    "description": "The agent ID of the recipient."
                },
                "item_id": {
                    "type": "string",
                    "description": "The ID of the item to transfer."
                }
            },
            "required": ["to_agent_id", "item_id"]
        }),
    }
}

fn tool_check_balance() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "check_balance".to_string(),
        description: "Check the agent's current balance and recent income/expense history.".to_string(),
        endpoint: "/actions/check_balance".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

fn tool_pay_agent() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "pay_agent".to_string(),
        description: "Transfer money to another agent. Both agents must be at the same or adjacent locations. The amount is deducted from your balance immediately.".to_string(),
        endpoint: "/actions/pay_agent".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "to_agent_id": {
                    "type": "string",
                    "description": "The agent ID to send money to."
                },
                "amount_cents": {
                    "type": "integer",
                    "description": "Amount in cents to transfer.",
                    "minimum": 1
                },
                "reason": {
                    "type": "string",
                    "description": "Optional reason for the payment."
                }
            },
            "required": ["to_agent_id", "amount_cents"]
        }),
    }
}

fn tool_request_money() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "request_money".to_string(),
        description: "Request money from another agent. The request is recorded as pending and the other agent can accept or reject it.".to_string(),
        endpoint: "/actions/request_money".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "from_agent_id": {
                    "type": "string",
                    "description": "The agent ID to request money from."
                },
                "amount_cents": {
                    "type": "integer",
                    "description": "Amount in cents to request.",
                    "minimum": 1
                },
                "reason": {
                    "type": "string",
                    "description": "Optional reason for the request."
                }
            },
            "required": ["from_agent_id", "amount_cents"]
        }),
    }
}

fn tool_respond_money_request() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "respond_money_request".to_string(),
        description: "Accept or reject a pending money request from another agent. Use the transaction_id from the request.".to_string(),
        endpoint: "/actions/respond_money_request".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "transaction_id": {
                    "type": "integer",
                    "description": "The transaction ID of the money request."
                },
                "accept": {
                    "type": "boolean",
                    "description": "True to accept and pay, false to reject."
                }
            },
            "required": ["transaction_id", "accept"]
        }),
    }
}

fn tool_get_transaction_log() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "get_transaction_log".to_string(),
        description: "View recent transactions where you were the sender or receiver. Shows payments, money requests, and their status.".to_string(),
        endpoint: "/actions/get_transaction_log".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Max number of transactions to return (1-100, default 20).",
                    "minimum": 1,
                    "maximum": 100
                }
            },
            "required": []
        }),
    }
}

fn tool_check_bank_rates() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "check_bank_rates".to_string(),
        description: "Learn the current bank deposit and loan rates, including approximate annualized rates.".to_string(),
        endpoint: "/actions/check_bank_rates".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_check_bank_account() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "check_bank_account".to_string(),
        description: "Check your cash, bank deposit balance, active loans, and accrued interest as of the current simulated time.".to_string(),
        endpoint: "/actions/check_bank_account".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_deposit_money() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "deposit_money".to_string(),
        description: "Move cash from your agent balance into your bank deposit account. Deposits earn the bank's daily deposit rate.".to_string(),
        endpoint: "/actions/deposit_money".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "amount_cents": {"type": "integer", "description": "Amount in cents to deposit.", "minimum": 1}
            },
            "required": ["amount_cents"]
        }),
    }
}

fn tool_withdraw_money() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "withdraw_money".to_string(),
        description: "Move money from your bank deposit account back to your cash balance.".to_string(),
        endpoint: "/actions/withdraw_money".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "amount_cents": {"type": "integer", "description": "Amount in cents to withdraw.", "minimum": 1}
            },
            "required": ["amount_cents"]
        }),
    }
}

fn tool_take_loan() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "take_loan".to_string(),
        description: "Borrow money from the bank. Loan principal is added to your cash balance and accrues interest at the current loan rate.".to_string(),
        endpoint: "/actions/take_loan".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "amount_cents": {"type": "integer", "description": "Amount in cents to borrow.", "minimum": 1},
                "purpose": {"type": "string", "description": "Optional reason for the loan."}
            },
            "required": ["amount_cents"]
        }),
    }
}

fn tool_repay_loan() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "repay_loan".to_string(),
        description: "Repay part or all of an active bank loan from your cash balance.".to_string(),
        endpoint: "/actions/repay_loan".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "loan_id": {"type": "string", "description": "The loan id to repay."},
                "amount_cents": {"type": "integer", "description": "Amount in cents to repay.", "minimum": 1}
            },
            "required": ["loan_id", "amount_cents"]
        }),
    }
}

fn tool_set_bank_rates() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "set_bank_rates".to_string(),
        description: "Banker-only. Set separate daily interest rates for deposits and loans.".to_string(),
        endpoint: "/actions/set_bank_rates".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "deposit_rate_daily": {"type": "number", "description": "Daily deposit interest rate, e.g. 0.0005 for 0.05%."},
                "loan_rate_daily": {"type": "number", "description": "Daily loan interest rate, e.g. 0.002 for 0.2%."}
            },
            "required": ["deposit_rate_daily", "loan_rate_daily"]
        }),
    }
}

fn tool_check_bank_balance_sheet() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "check_bank_balance_sheet".to_string(),
        description: "Banker-only. Inspect bank cash, total deposits, outstanding loans, reserve requirement, and lendable funds.".to_string(),
        endpoint: "/actions/check_bank_balance_sheet".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_check_bank_trends() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "check_bank_trends".to_string(),
        description: "Banker-only. Inspect recent bank activity: deposits, withdrawals, loans, repayments, interest, utilization ratio, reserve buffer, and agents with active loans.".to_string(),
        endpoint: "/actions/check_bank_trends".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_check_rate_policy_context() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "check_rate_policy_context".to_string(),
        description: "Banker-only. Get current spread, lendable-funds tightness, deposit and loan growth status, and suggested safe rate ranges to guide rate-setting decisions.".to_string(),
        endpoint: "/actions/check_rate_policy_context".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_explain_bank_policy() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "explain_bank_policy".to_string(),
        description: "Read a concise explanation of how bank policy works: reserves, spreads, deposit vs loan rate tradeoffs, and the utilization ratio. Useful for any agent who wants to understand banking fundamentals.".to_string(),
        endpoint: "/actions/explain_bank_policy".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_check_location_roles() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "check_location_roles".to_string(),
        description: "Check who has roles at your current location — residents, owners, workers, managers. Helps you understand who lives or works where you are.".to_string(),
        endpoint: "/actions/check_location_roles".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_check_vitals() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "check_vitals".to_string(),
        description: "Check your current vitals (food, water, stamina, sleep levels) and balance. Vitals decay over time, so this always returns up-to-date values.".to_string(),
        endpoint: "/actions/check_vitals".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

fn tool_buy_item() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "buy_item".to_string(),
        description: "Buy an item from a shop shelf. You must be at the same location as the item. Money is deducted from your balance and the item is added to your inventory.".to_string(),
        endpoint: "/actions/buy_item".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "item_id": {
                    "type": "string",
                    "description": "The ID of the item you want to buy."
                },
                "quantity": {
                    "type": "integer",
                    "description": "How many to buy (default: 1)."
                }
            },
            "required": ["item_id"]
        }),
    }
}

fn tool_check_shelf_stock() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "check_shelf_stock".to_string(),
        description: "Check what's on the shop shelves, what's in the backroom, pending deliveries, and your shop balance. Shopkeeper only.".to_string(),
        endpoint: "/actions/check_shelf_stock".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

fn tool_restock_shelf() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "restock_shelf".to_string(),
        description: "Move an item from the backroom to the shop shelf and set its price. Shopkeeper only.".to_string(),
        endpoint: "/actions/restock_shelf".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "item_id": {
                    "type": "string",
                    "description": "The ID of the backroom item to restock."
                },
                "shelf_price_cents": {
                    "type": "integer",
                    "description": "The price in cents to sell this item for. If the item has a restock_price, that will be used instead."
                }
            },
            "required": ["item_id", "shelf_price_cents"]
        }),
    }
}

fn tool_receive_delivery() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "receive_delivery".to_string(),
        description: "Receive a pending delivery crate, unpack items into the backroom, and pay the delivery cost. Shopkeeper only.".to_string(),
        endpoint: "/actions/receive_delivery".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "delivery_id": {
                    "type": "string",
                    "description": "The ID of the delivery crate to receive."
                }
            },
            "required": ["delivery_id"]
        }),
    }
}

fn tool_order_delivery() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "order_delivery".to_string(),
        description: "Order a new delivery of supplies. Items will arrive as a crate at the checkout. You pay the wholesale cost upfront. Shopkeeper only.".to_string(),
        endpoint: "/actions/order_delivery".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "description": "List of items to order.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string", "description": "Item name"},
                            "quantity": {"type": "integer", "description": "How many"},
                            "consumable_type": {"type": "string", "description": "food, water, stamina, sleep, hygiene, or appearance"},
                            "vital_value": {"type": "integer", "description": "How much vital boost this item gives"},
                            "cost_cents": {"type": "integer", "description": "Wholesale cost per unit in cents"}
                        },
                        "required": ["name", "quantity", "cost_cents"]
                    }
                }
            },
            "required": ["items"]
        }),
    }
}

fn tool_clean_shop() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "clean_shop".to_string(),
        description: "Clean the shop. Costs 10 stamina. Shopkeeper only.".to_string(),
        endpoint: "/actions/clean_shop".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

fn tool_browse_shop() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "browse_shop".to_string(),
        description: "Browse items for sale at the shop you're currently in. Shows item names, prices, quantities, and what vital they restore.".to_string(),
        endpoint: "/actions/browse_shop".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_list_job_openings() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "list_job_openings".to_string(),
        description: "See available jobs with wages, employers, and how many employees each has.".to_string(),
        endpoint: "/actions/list_job_openings".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

#[allow(dead_code)]
fn tool_apply_for_job() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "apply_for_job".to_string(),
        description: "Apply for a job. City jobs are approved immediately; private jobs require employer approval.".to_string(),
        endpoint: "/actions/apply_for_job".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "job_id": {"type": "string", "description": "The job to apply for."},
                "notes": {"type": "string", "description": "Optional message to the employer."}
            },
            "required": ["job_id"]
        }),
    }
}

fn tool_check_payroll() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "check_payroll".to_string(),
        description: "See your active employees, their wages, when they were last paid, and your total payroll obligation. Employer only.".to_string(),
        endpoint: "/actions/check_payroll".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_pay_employee() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "pay_employee".to_string(),
        description: "Pay an employee from your balance. The amount is deducted from you and credited to them. Employer only.".to_string(),
        endpoint: "/actions/pay_employee".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "employee_id": {"type": "string", "description": "The employee to pay."},
                "amount_cents": {"type": "integer", "description": "How much to pay in cents."},
                "reason": {"type": "string", "description": "Optional reason for the payment."}
            },
            "required": ["employee_id", "amount_cents"]
        }),
    }
}

fn tool_resign_job() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "resign_job".to_string(),
        description: "Resign from your current primary job. Your employer will be notified.".to_string(),
        endpoint: "/actions/resign_job".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "reason": {"type": "string", "description": "Why you're resigning."}
            },
            "required": []
        }),
    }
}

fn tool_hire_applicant() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "hire_applicant".to_string(),
        description: "Approve a pending job application. Employer only.".to_string(),
        endpoint: "/actions/hire_applicant".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "applicant_id": {"type": "string", "description": "Who to hire."},
                "job_id": {"type": "string", "description": "Which job they applied for."},
                "wage_cents": {"type": "integer", "description": "Optional wage override in cents."}
            },
            "required": ["applicant_id", "job_id"]
        }),
    }
}

fn tool_fire_employee() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "fire_employee".to_string(),
        description: "Fire an employee from their job. They will be notified. Employer only.".to_string(),
        endpoint: "/actions/fire_employee".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "employee_id": {"type": "string", "description": "Who to fire."},
                "reason": {"type": "string", "description": "Why they're being fired."}
            },
            "required": ["employee_id"]
        }),
    }
}

fn tool_collect_city_wage() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "collect_city_wage".to_string(),
        description: "Collect your salary from the city treasury. Only available if you have a city job and enough time has passed since last pay.".to_string(),
        endpoint: "/actions/collect_city_wage".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_read_civic_board() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "read_civic_board".to_string(),
        description: "Read complaints, hall of fame nominations, ordinances, and announcements from the civic board.".to_string(),
        endpoint: "/actions/read_civic_board".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "type": {"type": "string", "description": "Filter by type: complaint, hall_of_fame, ordinance, announcement"}
            },
            "required": []
        }),
    }
}

fn tool_file_complaint() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "file_complaint".to_string(),
        description: "File a formal complaint at the townhall. The mayor can resolve it.".to_string(),
        endpoint: "/actions/file_complaint".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "Short title for the complaint."},
                "body": {"type": "string", "description": "Detailed description of the complaint."}
            },
            "required": ["title", "body"]
        }),
    }
}

fn tool_nominate_for_hall_of_fame() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "nominate_for_hall_of_fame".to_string(),
        description: "Nominate a fellow citizen for the town hall of fame.".to_string(),
        endpoint: "/actions/nominate_for_hall_of_fame".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "nominee_id": {"type": "string", "description": "The citizen to nominate."},
                "reason": {"type": "string", "description": "Why they deserve recognition."}
            },
            "required": ["nominee_id", "reason"]
        }),
    }
}

fn tool_mayor_set_city_wage() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "mayor_set_city_wage".to_string(),
        description: "Set the wage for a city job. Mayor only.".to_string(),
        endpoint: "/actions/mayor_set_city_wage".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "job_id": {"type": "string", "description": "The city job to adjust."},
                "wage_cents": {"type": "integer", "description": "New wage per pay period in cents."}
            },
            "required": ["job_id", "wage_cents"]
        }),
    }
}

fn tool_mayor_fire_city_employee() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "mayor_fire_city_employee".to_string(),
        description: "Fire a city employee. Mayor only.".to_string(),
        endpoint: "/actions/mayor_fire_city_employee".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "employee_id": {"type": "string", "description": "Who to fire."},
                "reason": {"type": "string", "description": "Why."}
            },
            "required": ["employee_id"]
        }),
    }
}

fn tool_mayor_post_announcement() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "mayor_post_announcement".to_string(),
        description: "Post an official town announcement. Mayor only.".to_string(),
        endpoint: "/actions/mayor_post_announcement".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "Announcement title."},
                "body": {"type": "string", "description": "Announcement body."}
            },
            "required": ["title", "body"]
        }),
    }
}

fn tool_mayor_post_ordinance() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "mayor_post_ordinance".to_string(),
        description: "Post a town ordinance (a rule agents should follow). Mayor only.".to_string(),
        endpoint: "/actions/mayor_post_ordinance".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "Ordinance title."},
                "body": {"type": "string", "description": "Ordinance body."}
            },
            "required": ["title", "body"]
        }),
    }
}

fn tool_mayor_resolve_complaint() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "mayor_resolve_complaint".to_string(),
        description: "Resolve an active complaint. Mayor only.".to_string(),
        endpoint: "/actions/mayor_resolve_complaint".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "complaint_id": {"type": "string", "description": "The complaint to resolve."},
                "resolution": {"type": "string", "description": "How it was resolved."}
            },
            "required": ["complaint_id", "resolution"]
        }),
    }
}

fn tool_mayor_veto_ordinance() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "mayor_veto_ordinance".to_string(),
        description: "Repeal an existing ordinance. Mayor only.".to_string(),
        endpoint: "/actions/mayor_veto_ordinance".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "ordinance_id": {"type": "string", "description": "The ordinance to repeal."}
            },
            "required": ["ordinance_id"]
        }),
    }
}

fn tool_mayor_approve_city_job() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "mayor_approve_city_job".to_string(),
        description: "Approve a pending city job application. Mayor only.".to_string(),
        endpoint: "/actions/mayor_approve_city_job".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "applicant_id": {"type": "string", "description": "Who applied."},
                "job_id": {"type": "string", "description": "Which city job."}
            },
            "required": ["applicant_id", "job_id"]
        }),
    }
}

fn tool_call_election() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "call_election".to_string(),
        description: "Call a new mayoral election. Mayor only.".to_string(),
        endpoint: "/actions/call_election".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_nominate_self() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "nominate_self".to_string(),
        description: "Nominate yourself for mayor in the current election.".to_string(),
        endpoint: "/actions/nominate_self".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "platform": {"type": "string", "description": "Your campaign platform."}
            },
            "required": []
        }),
    }
}

fn tool_cast_vote() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "cast_vote".to_string(),
        description: "Vote for a mayoral candidate in the current election. One vote per agent.".to_string(),
        endpoint: "/actions/cast_vote".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "candidate_id": {"type": "string", "description": "Who you're voting for."}
            },
            "required": ["candidate_id"]
        }),
    }
}

fn tool_close_election() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "close_election".to_string(),
        description: "Close the current election and declare a winner (most votes wins). Mayor only.".to_string(),
        endpoint: "/actions/close_election".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_check_world_time() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "check_world_time".to_string(),
        description: "Check the current time of day in the simulation. Returns the hour, time-of-day label (morning/afternoon/evening/night), day number, and whether shops are open.".to_string(),
        endpoint: "/actions/check_world_time".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_wash_up() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "wash_up".to_string(),
        description: "Quick wash — face and hands. Restores hygiene +15, appearance +5. Costs 5 stamina. Need water access (home, cafe, clinic, park).".to_string(),
        endpoint: "/actions/wash_up".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_shower() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "shower".to_string(),
        description: "Take a full shower at home. Restores hygiene +50, appearance +15. Costs 15 stamina.".to_string(),
        endpoint: "/actions/shower".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_brush_teeth() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "brush_teeth".to_string(),
        description: "Brush your teeth at home. Restores hygiene +10. Costs 2 stamina.".to_string(),
        endpoint: "/actions/brush_teeth".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_get_ready() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "get_ready".to_string(),
        description: "Full morning routine at home — shower, groom, dress. Restores hygiene +60, appearance +40. Costs 25 stamina.".to_string(),
        endpoint: "/actions/get_ready".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_bathe() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "bathe".to_string(),
        description: "Bathe outdoors in the river or garden. Restores hygiene +40, appearance +5. Costs 10 stamina.".to_string(),
        endpoint: "/actions/bathe".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_swim() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "swim".to_string(),
        description: "Go for a swim at the river or garden. Restores hygiene +30 but appearance -5 (wet hair). Costs 20 stamina.".to_string(),
        endpoint: "/actions/swim".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}

fn tool_groom() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "groom".to_string(),
        description: "Fix your hair and straighten your clothes. Restores appearance +15. Costs 5 stamina. Can do anywhere.".to_string(),
        endpoint: "/actions/groom".to_string(),
        method: "POST".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
    }
}


fn tool_set_intention() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "set_intention".to_string(),
        description: "Set your current intention — what you plan to do next and why. If you already have an active intention, it will be automatically abandoned and replaced with the new one. Use this before starting any meaningful action sequence.".to_string(),
        endpoint: "/actions/set_intention".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "Short description of what you intend to do."
                },
                "reason": {
                    "type": "string",
                    "description": "Why you want to do this."
                },
                "expected_location_id": {
                    "type": "string",
                    "description": "The location you expect to go to (optional)."
                },
                "expected_action": {
                    "type": "string",
                    "description": "The action you expect to take (optional, e.g., 'eat', 'sleep')."
                }
            },
            "required": ["summary", "reason"]
        }),
    }
}

fn tool_complete_intention() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "complete_intention".to_string(),
        description: "Mark your current active intention as completed, failed, or abandoned. You must provide a status and optionally an outcome describing what happened.".to_string(),
        endpoint: "/actions/complete_intention".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["completed", "failed", "abandoned"],
                    "description": "The final status: 'completed' if you succeeded, 'failed' if you couldn't do it, 'abandoned' if you changed your mind."
                },
                "outcome": {
                    "type": "string",
                    "description": "What happened — a brief description of the result (optional)."
                }
            },
            "required": ["status"]
        }),
    }
}

fn tool_get_intention() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "get_intention".to_string(),
        description: "Get your current active intention, including the summary, reason, expected location, and expected action. Returns null if you have no active intention.".to_string(),
        endpoint: "/actions/get_intention".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}
