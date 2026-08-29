use axum::Json;
use axum::extract::{Path, State};

use crate as reading_task;
use reading_task::TaskItemResult;

use crate::web::error::CommandError;
use crate::web::state::AppState;
use crate::web::utils::run_blocking_db;

pub async fn get_task_results(
  State(state): State<AppState>,
  Path(task_id): Path<String>,
) -> Result<Json<Vec<TaskItemResult>>, CommandError> {
  let db = state.resolve_db()?;
  let results = run_blocking_db(move || reading_task::get_task_results(&db, &task_id)).await?;
  Ok(Json(results))
}

pub async fn retry_task_result(
  State(state): State<AppState>,
  Path((task_id, result_id)): Path<(String, i64)>,
) -> Result<Json<TaskItemResult>, CommandError> {
  let db = state.resolve_db()?;
  let result = reading_task::retry_task_result(&db, &task_id, result_id)
    .await
    .map_err(CommandError::from)?;
  Ok(Json(result))
}
