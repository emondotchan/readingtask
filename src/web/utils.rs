use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;

use log::Level;

use crate as reading_task;
use reading_task::DbContext;

use super::bootstrap::RuntimePaths;
use super::dto::RuntimeStatus;
use super::error::CommandError;

pub fn log_command(level: Level, command: &str, message: impl AsRef<str>) {
  log::log!(
    target: &format!("reading_task::web::{command}"),
    level,
    "{}",
    message.as_ref()
  );
}

pub fn validation_error(message: impl Into<String>) -> CommandError {
  CommandError {
    category: "validation".to_string(),
    message: message.into(),
  }
}

pub fn resource_error(message: impl Into<String>) -> CommandError {
  CommandError {
    category: "resource".to_string(),
    message: message.into(),
  }
}

pub async fn run_blocking_db<F, T>(f: F) -> Result<T, CommandError>
where
  F: FnOnce() -> Result<T, reading_task::AppError> + Send + 'static,
  T: Send + 'static,
{
  tokio::task::spawn_blocking(f)
    .await
    .map_err(|e| resource_error(format!("后台任务调度失败: {e}")))?
    .map_err(CommandError::from)
}

pub async fn run_blocking_fn<F, T>(f: F) -> Result<T, CommandError>
where
  F: FnOnce() -> Result<T, CommandError> + Send + 'static,
  T: Send + 'static,
{
  tokio::task::spawn_blocking(f)
    .await
    .map_err(|e| resource_error(format!("后台任务调度失败: {e}")))?
}

pub fn normalize_sqlite_path(path: &str) -> Result<PathBuf, CommandError> {
  let trimmed = path.trim();
  if trimmed.is_empty() {
    return Err(validation_error("SQLite 存储文件路径不能为空"));
  }

  let db_path = if let Some(suffix) = trimmed.strip_prefix("~/") {
    std::env::var_os("HOME")
      .map(PathBuf::from)
      .unwrap_or_else(|| PathBuf::from("~"))
      .join(suffix)
  } else {
    PathBuf::from(trimmed)
  };

  if db_path.is_dir() {
    return Err(validation_error(
      "SQLite 存储文件路径必须是文件，不能是目录",
    ));
  }

  if let Some(parent) = db_path.parent()
    && !parent.as_os_str().is_empty()
  {
    fs::create_dir_all(parent)
      .map_err(|error| resource_error(format!("无法创建 SQLite 目录: {error}")))?;
  }

  Ok(db_path)
}

pub fn build_runtime_status(paths: &RuntimePaths, db: Option<&DbContext>) -> RuntimeStatus {
  let sqlite_configured = db.is_some();
  let (open_ids_ready, shop_ready, fc_ready, course_ready) = if let Some(db) = db {
    let open_ids_ready = reading_task::get_all_open_id_records(db)
      .map(|items| !items.is_empty())
      .unwrap_or(false);
    let shop_ready = reading_task::get_all_shops(db)
      .map(|items| !items.is_empty())
      .unwrap_or(false);
    let fc_ready = reading_task::get_all_fcs(db)
      .map(|items| !items.is_empty())
      .unwrap_or(false);
    let course_ready = reading_task::get_all_courses(db)
      .map(|items| !items.is_empty())
      .unwrap_or(false);
    (open_ids_ready, shop_ready, fc_ready, course_ready)
  } else {
    (false, false, false, false)
  };

  RuntimeStatus {
    sqlite_path: paths
      .db_path
      .as_ref()
      .map(|path| path.to_string_lossy().to_string()),
    sqlite_configured,
    open_ids_ready,
    shop_ready,
    province_ready: sqlite_configured,
    fc_ready,
    course_ready,
  }
}

pub fn default_bind_addr() -> SocketAddr {
  SocketAddr::from(([0, 0, 0, 0], 10086))
}
