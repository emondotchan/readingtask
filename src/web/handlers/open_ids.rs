use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate as reading_task;
use reading_task::OpenIdRecord;

use crate::web::error::CommandError;
use crate::web::state::AppState;
use crate::web::utils::run_blocking_db;

pub async fn get_open_ids(
  State(state): State<AppState>,
) -> Result<Json<Vec<OpenIdRecord>>, CommandError> {
  let db = state.resolve_db()?;
  let ids = run_blocking_db(move || reading_task::get_all_open_id_records(&db)).await?;
  Ok(Json(ids))
}

pub async fn add_open_id(
  State(state): State<AppState>,
  Json(open_id): Json<OpenIdRecord>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  run_blocking_db(move || reading_task::add_open_id(&db, &open_id)).await?;
  Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_open_id(
  State(state): State<AppState>,
  Path(open_id): Path<String>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  run_blocking_db(move || reading_task::delete_open_id(&db, &open_id)).await?;
  Ok(StatusCode::NO_CONTENT)
}
