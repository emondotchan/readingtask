use axum::Json;
use axum::extract::State;

use crate as reading_task;
use reading_task::{TaskRunRequest, TaskRunSummary};

use crate::web::dto::RunTaskInput;
use crate::web::error::CommandError;
use crate::web::state::AppState;

pub async fn run_reading_task(
  State(state): State<AppState>,
  Json(request): Json<RunTaskInput>,
) -> Result<Json<TaskRunSummary>, CommandError> {
  let db = state.resolve_db()?;
  let run_date = request.run_date.clone();
  let task_request: TaskRunRequest = request.into();

  let summary = reading_task::run_task_with_progress(&db, task_request, |_| {}, Some(&run_date))
    .await
    .map_err(CommandError::from)?;

  Ok(Json(summary))
}
