use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
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
}

#[derive(Debug, Serialize)]
pub struct LookAroundResponse {
    pub location: Location,
    pub nearby: Vec<AdjacentLocation>,
    pub objects: Vec<WorldObject>,
    pub agents_present: Vec<LookAroundAgent>,
    pub items_on_ground: Vec<InventoryItem>,
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
        SELECT id, name, state, current_activity
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

    Ok(Json(ApiResponse::from(LookAroundResponse {
        location,
        nearby,
        objects,
        agents_present,
        items_on_ground,
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

    // Credit shopkeeper (if one exists at this shop — same location prefix, e.g. harvey_oak_*)
    let location_prefix = item_location.split('_').take(2).collect::<Vec<_>>().join("_");
    let shopkeeper_id: Option<String> = sqlx::query_scalar(
        r#"
        SELECT id FROM agents
        WHERE occupation = 'Shopkeeper' AND is_active = TRUE
          AND current_location_id LIKE $1
        LIMIT 1
        "#,
    )
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

    // Shelf items (priced, on aisle)
    let shelf_items = sqlx::query_as::<_, InventoryItem>(
        r#"
        SELECT id, name, held_by, location_id, state, quantity, consumable_type, vital_value, price_cents
        FROM inventory_items
        WHERE location_id LIKE $1 AND price_cents IS NOT NULL AND held_by IS NULL
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

    // Shop balance (shopkeeper's balance)
    let shop_balance_cents = sqlx::query_scalar::<_, i64>(
        r#"SELECT balance_cents FROM agents WHERE id = $1"#,
    )
    .bind(&agent_id)
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

    // Get the aisle location for this shop
    let aisle_location = format!("{}_aisle", prefix);

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
    .bind(&aisle_location)
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
    .bind(&aisle_location)
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
    let counter_id = format!("{}_counter_{}", prefix, prefix.split('_').last().unwrap_or("harvey"));

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

pub async fn action_set_intention(
    State(state): State<AppState>,
    AgentId(agent_id): AgentId,
    Json(payload): Json<CreateAgentIntentionRequest>,
) -> AppResult<Json<ApiResponse<AgentIntention>>> {
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
        tool_check_vitals(),
        tool_set_intention(),
        tool_complete_intention(),
        tool_get_intention(),
    ];
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

    // Check for priced items at current location (shop shelves)
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

    if has_priced_items {
        tools.push(tool_buy_item());
    }

    // Shopkeeper tools — only for agents with occupation 'Shopkeeper' at a shop location
    let is_shopkeeper = agent_row.4 == "Shopkeeper";
    let at_shop = agent_row.1.starts_with("harvey_oak_");

    if is_shopkeeper && at_shop {
        // Always available for shopkeepers at their shop
        tools.push(tool_check_shelf_stock());
        tools.push(tool_clean_shop());

        // Conditional: restock when backroom items exist
        let location_prefix = agent_row.1.split('_').take(2).collect::<Vec<_>>().join("_");
        let has_backroom_items = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM inventory_items
                WHERE location_id LIKE $1 AND price_cents IS NULL AND held_by IS NULL
                  AND state->>'backroom' = 'true'
            )
            "#,
        )
        .bind(format!("{}%", location_prefix))
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
        .bind(format!("{}%", location_prefix))
        .fetch_one(state.pool())
        .await?;

        if has_pending_delivery {
            tools.push(tool_receive_delivery());
        } else {
            tools.push(tool_order_delivery());
        }
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
                            "consumable_type": {"type": "string", "description": "food, water, or stamina"},
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

fn tool_set_intention() -> WorldToolDefinition {
    WorldToolDefinition {
        name: "set_intention".to_string(),
        description: "Set your current intention — what you plan to do next. You can only have one active intention at a time. Use this to plan your next action (e.g., 'go to the cafe to eat', 'walk to the park').".to_string(),
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
        description: "Get your current active intention, if any. Returns null if you have no active intention.".to_string(),
        endpoint: "/actions/get_intention".to_string(),
        method: "POST".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}
