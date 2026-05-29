use std::net::SocketAddr;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{
    Router,
    routing::{delete, get, patch, post},
};
use dotenvy::dotenv;
use tower_http::cors::{Any, CorsLayer};
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

mod auth;
mod error;
mod heartbeat;
mod models;
mod routes;
mod state;
mod ws_events;

use auth::require_sim_key;
use error::AppResult;
use routes::actions::{
    action_accept_invitation, action_accept_join_request, action_board_post, action_check_balance,
    action_cook_food, action_buy_item, action_check_vitals, action_complete_intention, action_drop_item,
    action_check_shelf_stock, action_restock_shelf, action_receive_delivery, action_order_delivery, action_clean_shop,
    action_list_job_openings, action_apply_for_job, action_check_payroll, action_pay_employee,
    action_resign_job, action_hire_applicant, action_fire_employee, action_collect_city_wage,
    action_read_civic_board, action_file_complaint, action_nominate_for_hall_of_fame,
    action_mayor_set_city_wage, action_mayor_fire_city_employee, action_mayor_post_announcement,
    action_mayor_post_ordinance, action_mayor_resolve_complaint, action_mayor_veto_ordinance,
    action_mayor_approve_city_job, action_call_election, action_nominate_self,
    action_cast_vote, action_close_election,
    action_wash_up, action_shower, action_brush_teeth, action_get_ready,
    action_bathe, action_swim, action_groom,
    action_browse_shop,
    action_check_world_time,
    action_get_intention, action_get_inventory, action_get_transaction_log,
    action_join_conversation, action_leave_conversation, action_look_around, action_move_to,
    action_pay_agent, action_pick_up_item, action_request_money, action_respond_money_request,
    action_send_message, action_set_activity, action_set_intention, action_sleep,
    action_speak_to, action_transfer_item, action_use_item, action_wake_up, get_tool_manifest,
};
use routes::applications::{create_application, get_application, list_applications, approve_application, reject_application, get_admin_application};
use routes::construction::{start_home_project, fund_home_project, hire_builder, check_home_project, list_construction_companies};
use routes::banking::{
    action_check_bank_account, action_check_bank_balance_sheet, action_check_bank_rates,
    action_check_bank_trends, action_check_rate_policy_context, action_explain_bank_policy,
    action_deposit_money, action_repay_loan, action_set_bank_rates, action_take_loan,
    action_withdraw_money,
};
use routes::agents::{
    agent_health_check, clear_agent_activity, get_agent_by_id, list_agents, move_agent_with_header,
    update_agent_activity, update_agent_location,
};
use routes::board::{
    clear_board, create_board_post, delete_board_post, get_board_posts, get_public_board,
};
use routes::citizens::{
    citizen_action, claim_citizen_wake, close_citizen_wake, create_test_citizen_wake,
    respond_citizen_wake, wait_for_agent_interrupt, ws_citizen,
};
use routes::economy::update_economy;
use routes::events::{create_event, list_events};
use routes::intentions::{
    create_agent_intention, get_current_agent_intention, list_agent_intentions,
    list_current_intentions, update_agent_intention,
};
use routes::inventory::transfer_item_between_agents;
use routes::inventory::{
    add_item_to_agent_inventory, get_agent_inventory, remove_item_from_agent_inventory, use_item,
};
use routes::jobs::{
    get_job_by_id, list_agent_jobs, list_job_agents, list_jobs, remove_agent_job, upsert_agent_job,
};
use routes::locations::{action_check_location_roles, get_agent_locations, get_location_by_id, get_nearby_locations, list_locations};
use routes::objects::{list_objects_by_location, update_object_state};
use routes::pathfind::get_path;
use routes::pulse::get_town_pulse;
use routes::sleep::{start_sleep, wake_up};
use routes::tokens::{create_agent_token, list_agent_tokens, revoke_agent_token};
use routes::world::{get_world_time, update_world_time};
use routes::conversations::{
    get_conversation_detail, list_active_conversations,
};
use state::AppState;
use ws_events::ws_events;

#[tokio::main]
async fn main() -> AppResult<()> {
    setup_tracing();
    dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")?;
    let max_connections: u32 = std::env::var("DB_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3001);

    let state = AppState::new(&database_url, max_connections).await?;

    // Start the simulation heartbeat — drives vitals alerts and idle wakes
    heartbeat::spawn_heartbeat(state.clone());

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/board", get(get_public_board))
        .route("/board/posts", get(get_board_posts))
        .route("/board/posts", patch(create_board_post))
        .route("/board/posts/:post_id", delete(delete_board_post))
        .route("/board/clear", delete(clear_board))
        .route("/events", get(list_events))
        .route("/events", post(create_event))
        .route("/intentions/current", get(list_current_intentions))
        .route("/ws/events", get(ws_events))
        .route("/ws/citizen", get(ws_citizen))
        .route("/locations", get(list_locations))
        .route("/world/time", get(get_world_time).patch(update_world_time))
        .route("/v1/citizen/action", post(citizen_action))
        .route("/v1/citizen/wakes/claim", post(claim_citizen_wake))
        .route("/v1/citizen/wakes/:wake_event_id/respond", post(respond_citizen_wake))
        .route("/v1/agents/:id/wait", get(wait_for_agent_interrupt))
        .route("/actions/set_activity", post(action_set_activity))
        .route("/actions/move_to", post(action_move_to))
        .route("/actions/board_post", post(action_board_post))
        .route("/actions/sleep", post(action_sleep))
        .route("/actions/wake_up", post(action_wake_up))
        .route("/actions/cook_food", post(action_cook_food))
        .route("/actions/look_around", post(action_look_around))
        .route("/actions/speak_to", post(action_speak_to))
        .route("/actions/join_conversation", post(action_join_conversation))
        .route("/actions/leave_conversation", post(action_leave_conversation))
        .route("/actions/send_message", post(action_send_message))
        .route("/actions/accept_join_request", post(action_accept_join_request))
        .route("/actions/accept_invitation", post(action_accept_invitation))
        .route("/actions/get_inventory", post(action_get_inventory))
        .route("/actions/pick_up_item", post(action_pick_up_item))
        .route("/actions/drop_item", post(action_drop_item))
        .route("/actions/use_item", post(action_use_item))
        .route("/actions/transfer_item", post(action_transfer_item))
        .route("/actions/check_balance", post(action_check_balance))
        .route("/actions/pay_agent", post(action_pay_agent))
        .route("/actions/request_money", post(action_request_money))
        .route("/actions/respond_money_request", post(action_respond_money_request))
        .route("/actions/get_transaction_log", post(action_get_transaction_log))
        .route("/actions/check_bank_rates", post(action_check_bank_rates))
        .route("/actions/check_bank_account", post(action_check_bank_account))
        .route("/actions/deposit_money", post(action_deposit_money))
        .route("/actions/withdraw_money", post(action_withdraw_money))
        .route("/actions/take_loan", post(action_take_loan))
        .route("/actions/repay_loan", post(action_repay_loan))
        .route("/actions/set_bank_rates", post(action_set_bank_rates))
        .route("/actions/check_bank_balance_sheet", post(action_check_bank_balance_sheet))
        .route("/actions/check_bank_trends", post(action_check_bank_trends))
        .route("/actions/check_rate_policy_context", post(action_check_rate_policy_context))
        .route("/actions/explain_bank_policy", post(action_explain_bank_policy))
        .route("/actions/check_vitals", post(action_check_vitals))
        .route("/actions/buy_item", post(action_buy_item))
        .route("/actions/check_shelf_stock", post(action_check_shelf_stock))
        .route("/actions/restock_shelf", post(action_restock_shelf))
        .route("/actions/receive_delivery", post(action_receive_delivery))
        .route("/actions/order_delivery", post(action_order_delivery))
        .route("/actions/clean_shop", post(action_clean_shop))
        .route("/actions/list_job_openings", post(action_list_job_openings))
        .route("/actions/apply_for_job", post(action_apply_for_job))
        .route("/actions/check_payroll", post(action_check_payroll))
        .route("/actions/pay_employee", post(action_pay_employee))
        .route("/actions/resign_job", post(action_resign_job))
        .route("/actions/hire_applicant", post(action_hire_applicant))
        .route("/actions/fire_employee", post(action_fire_employee))
        .route("/actions/collect_city_wage", post(action_collect_city_wage))
        .route("/actions/read_civic_board", post(action_read_civic_board))
        .route("/actions/file_complaint", post(action_file_complaint))
        .route("/actions/nominate_for_hall_of_fame", post(action_nominate_for_hall_of_fame))
        .route("/actions/mayor_set_city_wage", post(action_mayor_set_city_wage))
        .route("/actions/mayor_fire_city_employee", post(action_mayor_fire_city_employee))
        .route("/actions/mayor_post_announcement", post(action_mayor_post_announcement))
        .route("/actions/mayor_post_ordinance", post(action_mayor_post_ordinance))
        .route("/actions/mayor_resolve_complaint", post(action_mayor_resolve_complaint))
        .route("/actions/mayor_veto_ordinance", post(action_mayor_veto_ordinance))
        .route("/actions/mayor_approve_city_job", post(action_mayor_approve_city_job))
        .route("/actions/call_election", post(action_call_election))
        .route("/actions/nominate_self", post(action_nominate_self))
        .route("/actions/cast_vote", post(action_cast_vote))
        .route("/actions/close_election", post(action_close_election))
        .route("/actions/wash_up", post(action_wash_up))
        .route("/actions/shower", post(action_shower))
        .route("/actions/brush_teeth", post(action_brush_teeth))
        .route("/actions/get_ready", post(action_get_ready))
        .route("/actions/bathe", post(action_bathe))
        .route("/actions/swim", post(action_swim))
        .route("/actions/groom", post(action_groom))
        .route("/actions/browse_shop", post(action_browse_shop))
        .route("/actions/check_world_time", post(action_check_world_time))
        .route("/actions/set_intention", post(action_set_intention))
        .route("/actions/complete_intention", post(action_complete_intention))
        .route("/actions/get_intention", post(action_get_intention))
        .route("/actions/check_location_roles", post(action_check_location_roles))
        .route("/conversations", get(list_active_conversations))
        .route("/conversations/:id", get(get_conversation_detail))
        .route("/locations/:id", get(get_location_by_id))
        .route("/locations/:id/nearby", get(get_nearby_locations))
        .route(
            "/locations/:location_id/objects",
            get(list_objects_by_location),
        )
        .route("/objects/:id", patch(update_object_state))
        .route("/pathfind", get(get_path))
        .route("/town/pulse", get(get_town_pulse))
        .route("/applications", post(create_application))
        .route("/applications/:id", get(get_application))
        .route("/agents", get(list_agents))
        .route("/agents/health", get(agent_health_check))
        .route("/agents/move", patch(move_agent_with_header))
        .route("/admin/applications", get(list_applications))
        .route("/admin/applications/:id", get(get_admin_application))
        .route("/admin/applications/:id/approve", post(approve_application))
        .route("/admin/applications/:id/reject", post(reject_application))
        .route("/construction/companies", get(list_construction_companies))
        .route("/actions/start_home_project", post(start_home_project))
        .route("/actions/fund_home_project", post(fund_home_project))
        .route("/actions/hire_builder", post(hire_builder))
        .route("/actions/check_home_project", post(check_home_project))
        .route("/admin/agents/:id/tokens", get(list_agent_tokens))
        .route("/admin/agents/:id/tokens", post(create_agent_token))
        .route(
            "/admin/agents/:id/citizen-wakes/test",
            post(create_test_citizen_wake),
        )
        .route(
            "/admin/agents/:id/citizen-wakes/:event_id/close",
            post(close_citizen_wake),
        )
        .route("/admin/agent-tokens/:id", delete(revoke_agent_token))
        .route("/jobs", get(list_jobs))
        .route("/jobs/:id", get(get_job_by_id))
        .route("/jobs/:id/agents", get(list_job_agents))
        .route("/agents/:id", get(get_agent_by_id))
        .route("/agents/:id/tool-manifest", get(get_tool_manifest))
        .route("/agents/:id/locations", get(get_agent_locations))
        .route("/agents/:id/intentions", get(list_agent_intentions))
        .route("/agents/:id/intentions", post(create_agent_intention))
        .route("/agents/:id/jobs", get(list_agent_jobs))
        .route("/agents/:id/jobs/:job_id", patch(upsert_agent_job))
        .route("/agents/:id/jobs/:job_id", delete(remove_agent_job))
        .route(
            "/agents/:id/intentions/current",
            get(get_current_agent_intention),
        )
        .route(
            "/agents/:id/intentions/:intention_id",
            patch(update_agent_intention),
        )
        .route("/agents/:id/location", patch(update_agent_location))
        .route("/agents/:id/activity", patch(update_agent_activity))
        .route("/agents/:id/activity", delete(clear_agent_activity))
        .route("/agents/sleep", post(start_sleep))
        .route("/agents/sleep", delete(wake_up))
        .route("/agents/use-item", post(use_item))
        .route("/agents/:id/economy", patch(update_economy))
        .route("/inventory/:id", get(get_agent_inventory))
        .route("/inventory/:id/add", patch(add_item_to_agent_inventory))
        .route(
            "/inventory/:id/remove",
            patch(remove_item_from_agent_inventory),
        )
        .route(
            "/agents/:id/inventory/transfer",
            patch(transfer_item_between_agents),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_sim_key,
        ))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Starting World API on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

fn setup_tracing() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
}
