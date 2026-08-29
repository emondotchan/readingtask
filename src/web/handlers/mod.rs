pub mod courses;
pub mod daily;
pub mod fcs;
pub mod monthly;
pub mod open_ids;
pub mod quick_run;
pub mod results;
pub mod shops;
pub mod system;

use axum::Router;
use axum::routing::{delete, get, post};

use crate::web::state::AppState;

pub fn build_api_router(state: AppState) -> Router<AppState> {
  Router::new()
    .route("/health", get(system::health))
    .route("/runtime-status", get(system::get_runtime_status))
    .route("/sqlite-path", post(system::set_sqlite_path))
    .route("/run-reading-task", post(quick_run::run_reading_task))
    .route(
      "/open-ids",
      get(open_ids::get_open_ids).post(open_ids::add_open_id),
    )
    .route("/open-ids/{open_id}", delete(open_ids::delete_open_id))
    .route(
      "/shops",
      get(shops::get_shops).delete(shops::delete_all_shops),
    )
    .route("/shops/import", post(shops::import_shops))
    .route("/shops/shop-types", post(shops::update_shop_types))
    .route("/fcs", get(fcs::get_fcs).post(fcs::add_or_update_fc))
    .route("/fcs/{name}", delete(fcs::delete_fc))
    .route(
      "/courses",
      get(courses::get_courses)
        .post(courses::add_or_update_course)
        .delete(courses::delete_course),
    )
    .route("/shop-count", get(shops::get_shop_count))
    .route(
      "/monthly-tasks",
      get(monthly::get_monthly_tasks).post(monthly::create_monthly_task),
    )
    .route(
      "/monthly-tasks/preview",
      post(monthly::preview_monthly_task_plan),
    )
    .route("/monthly-tasks/{id}", delete(monthly::delete_monthly_task))
    .route(
      "/monthly-tasks/{id}/reschedule",
      post(monthly::reschedule_monthly_task_plans),
    )
    .route(
      "/daily-tasks/{task_id}",
      get(daily::get_daily_task).post(daily::save_daily_task),
    )
    .route(
      "/daily-tasks/{task_id}/pending",
      get(daily::get_pending_daily_task),
    )
    .route(
      "/daily-tasks/{task_id}/all",
      get(daily::get_task_daily_tasks),
    )
    .route("/daily-tasks/batch-run", post(daily::batch_run_daily_tasks))
    .route(
      "/daily-tasks/run-status",
      get(daily::get_daily_task_run_status),
    )
    .route("/daily-tasks/{task_id}/run", post(daily::run_daily_task))
    .route(
      "/daily-tasks/{task_id}/pause",
      post(daily::pause_daily_task),
    )
    .route("/tasks/{task_id}/results", get(results::get_task_results))
    .route(
      "/tasks/{task_id}/results/{result_id}/retry",
      post(results::retry_task_result),
    )
    .with_state(state)
}
