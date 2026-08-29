use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use log::Level;

use crate as reading_task;
use reading_task::{DailyTask, MonthlyTask, MonthlyTaskPlanPreview};

use crate::web::dto::ReschedulePlansInput;
use crate::web::error::CommandError;
use crate::web::state::AppState;
use crate::web::utils::{log_command, run_blocking_db};

pub async fn get_monthly_tasks(
  State(state): State<AppState>,
) -> Result<Json<Vec<MonthlyTask>>, CommandError> {
  let db = state.resolve_db()?;
  log_command(
    Level::Debug,
    "get_monthly_tasks",
    format!("db_path={}", db.db_path().display()),
  );
  let tasks = run_blocking_db(move || reading_task::get_all_monthly_tasks(&db))
    .await
    .inspect_err(|error| {
      log_command(Level::Error, "get_monthly_tasks", error.to_string());
    })?;
  log_command(
    Level::Debug,
    "get_monthly_tasks",
    format!("loaded {} monthly tasks", tasks.len()),
  );
  Ok(Json(tasks))
}

pub async fn preview_monthly_task_plan(
  State(state): State<AppState>,
  Json(task): Json<MonthlyTask>,
) -> Result<Json<MonthlyTaskPlanPreview>, CommandError> {
  let db = state.resolve_db()?;
  let preview =
    run_blocking_db(move || reading_task::preview_monthly_task_plan(&db, &task)).await?;
  Ok(Json(preview))
}

pub async fn create_monthly_task(
  State(state): State<AppState>,
  Json(task): Json<MonthlyTask>,
) -> Result<Json<MonthlyTaskPlanPreview>, CommandError> {
  let db = state.resolve_db()?;
  let plan =
    run_blocking_db(move || reading_task::create_monthly_task_with_plan(&db, &task)).await?;
  Ok(Json(plan))
}

pub async fn delete_monthly_task(
  State(state): State<AppState>,
  Path(id): Path<String>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  run_blocking_db(move || reading_task::delete_monthly_task(&db, &id)).await?;
  Ok(StatusCode::NO_CONTENT)
}

pub async fn reschedule_monthly_task_plans(
  State(state): State<AppState>,
  Path(id): Path<String>,
  Json(input): Json<Option<ReschedulePlansInput>>,
) -> Result<Json<Vec<DailyTask>>, CommandError> {
  let db = state.resolve_db()?;
  let start_date = input
    .and_then(|i| i.start_date)
    .filter(|s| !s.trim().is_empty())
    .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
  let updated =
    run_blocking_db(move || reading_task::reschedule_unfinished_daily_tasks(&db, &id, &start_date))
      .await?;
  Ok(Json(updated))
}
