use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use log::Level;

use crate as reading_task;
use reading_task::FcRecord;

use crate::web::dto::UpsertFcInput;
use crate::web::error::CommandError;
use crate::web::state::AppState;
use crate::web::utils::{log_command, run_blocking_db};

pub async fn get_fcs(State(state): State<AppState>) -> Result<Json<Vec<FcRecord>>, CommandError> {
  let db = state.resolve_db()?;
  log_command(
    Level::Debug,
    "get_fcs",
    format!("db_path={}", db.db_path().display()),
  );
  let fcs = run_blocking_db(move || reading_task::get_all_fcs(&db))
    .await
    .inspect_err(|error| {
      log_command(Level::Error, "get_fcs", error.to_string());
    })?;
  log_command(
    Level::Debug,
    "get_fcs",
    format!("loaded {} fc records", fcs.len()),
  );
  Ok(Json(fcs))
}

pub async fn add_or_update_fc(
  State(state): State<AppState>,
  Json(input): Json<UpsertFcInput>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  run_blocking_db(move || {
    reading_task::add_or_update_fc(&db, input.previous_name.as_deref(), &input.fc)
  })
  .await?;
  Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_fc(
  State(state): State<AppState>,
  Path(name): Path<String>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  run_blocking_db(move || reading_task::delete_fc(&db, &name)).await?;
  Ok(StatusCode::NO_CONTENT)
}
