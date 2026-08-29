use axum::Json;
use axum::extract::State;

use crate as reading_task;
use reading_task::AppPaths;

use crate::web::bootstrap;
use crate::web::dto::{RuntimeStatus, SaveSqlitePathInput};
use crate::web::error::CommandError;
use crate::web::state::AppState;
use crate::web::utils::{
  build_runtime_status, normalize_sqlite_path, resource_error, run_blocking_fn,
};

pub async fn health() -> &'static str {
  "ok"
}

pub async fn get_runtime_status(
  State(state): State<AppState>,
) -> Result<Json<RuntimeStatus>, CommandError> {
  let runtime_paths = state.snapshot_paths()?;
  let db = state.snapshot_db()?;
  let status =
    run_blocking_fn(move || Ok(build_runtime_status(&runtime_paths, db.as_ref()))).await?;
  Ok(Json(status))
}

pub async fn set_sqlite_path(
  State(state): State<AppState>,
  Json(input): Json<SaveSqlitePathInput>,
) -> Result<Json<RuntimeStatus>, CommandError> {
  let runtime_paths = state.snapshot_paths()?;
  let (db_path, db) = run_blocking_fn(move || {
    let db_path = normalize_sqlite_path(&input.sqlite_path)?;
    let app_paths = AppPaths::new_with_db_path(db_path.clone());
    let db = reading_task::init_db_context(&app_paths).map_err(CommandError::from)?;
    bootstrap::save_sqlite_path(&runtime_paths.sqlite_settings_path, &db_path)
      .map_err(|error| resource_error(format!("保存 SQLite 路径失败: {error}")))?;
    Ok((db_path, db))
  })
  .await?;

  let updated_paths = state.replace_db(db_path, db.clone())?;
  let status = run_blocking_fn(move || Ok(build_runtime_status(&updated_paths, Some(&db)))).await?;
  Ok(Json(status))
}
