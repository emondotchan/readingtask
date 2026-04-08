use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use reading_task::{
  AppPaths, FcRecord, MonthlyTask, MonthlyTaskPlanPreview, OpenIdRecord, ShopRecord,
  TaskRunRequest, TaskRunSummary,
};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::bootstrap::{self, RuntimePaths};
use crate::error::CommandError;

fn log_command(level: &str, command: &str, message: impl AsRef<str>) {
  eprintln!(
    "[reading_task::tauri::{command}][{level}] {}",
    message.as_ref()
  );
}

fn validation_error(message: impl Into<String>) -> CommandError {
  CommandError {
    category: "validation".to_string(),
    message: message.into(),
  }
}

fn resource_error(message: impl Into<String>) -> CommandError {
  CommandError {
    category: "resource".to_string(),
    message: message.into(),
  }
}

#[derive(Debug)]
pub struct RuntimeState {
  paths: Mutex<RuntimePaths>,
}

impl RuntimeState {
  pub fn new(paths: RuntimePaths) -> Self {
    Self {
      paths: Mutex::new(paths),
    }
  }

  fn snapshot(&self) -> Result<RuntimePaths, CommandError> {
    self
      .paths
      .lock()
      .map(|paths| paths.clone())
      .map_err(|_| resource_error("运行时路径锁已损坏"))
  }

  fn update_db_path(&self, db_path: PathBuf) -> Result<RuntimePaths, CommandError> {
    let mut paths = self
      .paths
      .lock()
      .map_err(|_| resource_error("运行时路径锁已损坏"))?;
    paths.db_path = Some(db_path);
    Ok(paths.clone())
  }
}

fn normalize_sqlite_path(path: &str) -> Result<PathBuf, CommandError> {
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
    return Err(validation_error("SQLite 存储文件路径必须是文件，不能是目录"));
  }

  if let Some(parent) = db_path.parent() {
    if !parent.as_os_str().is_empty() {
      fs::create_dir_all(parent)
        .map_err(|error| resource_error(format!("无法创建 SQLite 目录: {error}")))?;
    }
  }

  Ok(db_path)
}

fn build_runtime_status(paths: &RuntimePaths) -> RuntimeStatus {
  let sqlite_configured = paths.db_path.is_some();
  let (open_ids_ready, shop_ready, fc_ready) = if let Some(db_path) = &paths.db_path {
    let app_paths = AppPaths::new_with_db_path(db_path.clone());
    let open_ids_ready = reading_task::get_all_open_id_records(&app_paths)
      .map(|items| !items.is_empty())
      .unwrap_or(false);
    let shop_ready = reading_task::get_all_shops(&app_paths)
      .map(|items| !items.is_empty())
      .unwrap_or(false);
    let fc_ready = reading_task::get_all_fcs(&app_paths)
      .map(|items| !items.is_empty())
      .unwrap_or(false);
    (open_ids_ready, shop_ready, fc_ready)
  } else {
    (false, false, false)
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
  }
}

fn resolve_paths(app: &tauri::AppHandle) -> Result<AppPaths, CommandError> {
  let runtime_state = app.state::<RuntimeState>();
  let runtime_paths = runtime_state.snapshot()?;
  let db_path = runtime_paths
    .db_path
    .ok_or_else(|| resource_error("请先在首页配置 SQLite 存储文件路径"))?;

  Ok(AppPaths::new_with_db_path(db_path))
}

#[derive(Default)]
pub struct TaskPauseRegistry {
  flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl TaskPauseRegistry {
  fn register(&self, task_id: &str) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    if let Ok(mut flags) = self.flags.lock() {
      flags.insert(task_id.to_string(), Arc::clone(&flag));
    }
    flag
  }

  fn pause(&self, task_id: &str) -> bool {
    if let Ok(flags) = self.flags.lock() {
      if let Some(flag) = flags.get(task_id) {
        flag.store(true, Ordering::SeqCst);
        return true;
      }
    }

    false
  }

  fn clear(&self, task_id: &str) {
    if let Ok(mut flags) = self.flags.lock() {
      flags.remove(task_id);
    }
  }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
  pub sqlite_path: Option<String>,
  pub sqlite_configured: bool,
  pub open_ids_ready: bool,
  pub shop_ready: bool,
  pub province_ready: bool,
  pub fc_ready: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunTaskInput {
  pub s_course_id: String,
  pub s_manager_id: String,
  pub fc: String,
  pub count: usize,
  pub shopcodes: Vec<String>,
  pub run_date: String,
}

impl From<RunTaskInput> for TaskRunRequest {
  fn from(input: RunTaskInput) -> Self {
    Self {
      s_course_id: input.s_course_id,
      s_manager_id: input.s_manager_id,
      fc: input.fc,
      count: input.count,
      shopcodes: input.shopcodes,
    }
  }
}

#[tauri::command]
pub async fn get_runtime_status(app: tauri::AppHandle) -> Result<RuntimeStatus, CommandError> {
  let runtime_state = app.state::<RuntimeState>();
  let runtime_paths = runtime_state.snapshot()?;
  Ok(build_runtime_status(&runtime_paths))
}

#[tauri::command]
pub async fn set_sqlite_path(
  app: tauri::AppHandle,
  sqlite_path: String,
) -> Result<RuntimeStatus, CommandError> {
  let db_path = normalize_sqlite_path(&sqlite_path)?;
  let runtime_state = app.state::<RuntimeState>();
  let runtime_paths = runtime_state.snapshot()?;
  let app_paths = AppPaths::new_with_db_path(db_path.clone());
  reading_task::init_db(&app_paths).map_err(CommandError::from)?;

  bootstrap::save_sqlite_path(&runtime_paths.sqlite_settings_path, &db_path)
    .map_err(|error| resource_error(format!("保存 SQLite 路径失败: {error}")))?;

  let updated_paths = runtime_state.update_db_path(db_path)?;
  Ok(build_runtime_status(&updated_paths))
}

#[tauri::command]
pub async fn run_reading_task(
  app: tauri::AppHandle,
  request: RunTaskInput,
) -> Result<TaskRunSummary, CommandError> {
  let paths = resolve_paths(&app)?;
  let run_date = request.run_date.clone();
  let task_request: TaskRunRequest = request.into();

  let handle = app.clone();
  let summary = reading_task::run_task_with_progress(
    &paths,
    task_request,
    move |progress| {
      let _ = handle.emit("reading-task://progress", progress);
    },
    Some(&run_date),
  )
  .await
  .map_err(CommandError::from)?;

  Ok(summary)
}

#[tauri::command]
pub async fn get_open_ids(app: tauri::AppHandle) -> Result<Vec<OpenIdRecord>, CommandError> {
  let paths = resolve_paths(&app)?;
  let ids = reading_task::get_all_open_id_records(&paths).map_err(CommandError::from)?;
  Ok(ids)
}

#[tauri::command]
pub async fn add_open_id(app: tauri::AppHandle, open_id: OpenIdRecord) -> Result<(), CommandError> {
  let paths = resolve_paths(&app)?;
  reading_task::add_open_id(&paths, &open_id).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn delete_open_id(app: tauri::AppHandle, open_id: String) -> Result<(), CommandError> {
  let paths = resolve_paths(&app)?;
  reading_task::delete_open_id(&paths, &open_id).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn import_open_ids_csv(
  app: tauri::AppHandle,
  csv_text: String,
) -> Result<usize, CommandError> {
  let paths = resolve_paths(&app)?;
  reading_task::import_open_ids_csv(&paths, &csv_text).map_err(CommandError::from)
}

#[tauri::command]
pub async fn get_shops(app: tauri::AppHandle) -> Result<Vec<ShopRecord>, CommandError> {
  let paths = resolve_paths(&app)?;
  let shops = reading_task::get_all_shops(&paths).map_err(CommandError::from)?;
  Ok(shops)
}

#[tauri::command]
pub async fn add_or_update_shop(
  app: tauri::AppHandle,
  shop: ShopRecord,
) -> Result<(), CommandError> {
  let paths = resolve_paths(&app)?;
  reading_task::add_or_update_shop(&paths, &shop).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn delete_shop(app: tauri::AppHandle, shop_code: String) -> Result<(), CommandError> {
  let paths = resolve_paths(&app)?;
  reading_task::delete_shop(&paths, &shop_code).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn get_fcs(app: tauri::AppHandle) -> Result<Vec<FcRecord>, CommandError> {
  let paths = resolve_paths(&app)?;
  log_command(
    "INFO",
    "get_fcs",
    format!("db_path={}", paths.db_path.display()),
  );
  let fcs = reading_task::get_all_fcs(&paths).map_err(|error| {
    log_command("ERROR", "get_fcs", error.to_string());
    CommandError::from(error)
  })?;
  log_command("INFO", "get_fcs", format!("loaded {} fc records", fcs.len()));
  Ok(fcs)
}

#[tauri::command]
pub async fn add_or_update_fc(app: tauri::AppHandle, fc: FcRecord) -> Result<(), CommandError> {
  let paths = resolve_paths(&app)?;
  reading_task::add_or_update_fc(&paths, &fc).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn delete_fc(app: tauri::AppHandle, name: String) -> Result<(), CommandError> {
  let paths = resolve_paths(&app)?;
  reading_task::delete_fc(&paths, &name).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn get_shop_count(
  app: tauri::AppHandle,
  fc_name: String,
  task_type: String,
) -> Result<usize, CommandError> {
  let paths = resolve_paths(&app)?;
  log_command("INFO", "get_shop_count", format!("fc_name={fc_name} task_type={task_type}"));
  let count = reading_task::get_shop_count_by_fc_and_type(&paths, &fc_name, &task_type)
    .map_err(CommandError::from)?;
  log_command("INFO", "get_shop_count", format!("count={count}"));
  Ok(count)
}

#[tauri::command]
pub async fn preview_monthly_task_plan(
  app: tauri::AppHandle,
  task: MonthlyTask,
) -> Result<MonthlyTaskPlanPreview, CommandError> {
  let paths = resolve_paths(&app)?;
  reading_task::preview_monthly_task_plan(&paths, &task).map_err(CommandError::from)
}

#[tauri::command]
pub async fn get_monthly_tasks(
  app: tauri::AppHandle,
) -> Result<Vec<reading_task::MonthlyTask>, CommandError> {
  let paths = resolve_paths(&app)?;
  log_command(
    "INFO",
    "get_monthly_tasks",
    format!("db_path={}", paths.db_path.display()),
  );
  let tasks = reading_task::get_all_monthly_tasks(&paths).map_err(|error| {
    log_command("ERROR", "get_monthly_tasks", error.to_string());
    CommandError::from(error)
  })?;
  log_command(
    "INFO",
    "get_monthly_tasks",
    format!("loaded {} monthly tasks", tasks.len()),
  );
  Ok(tasks)
}

#[tauri::command]
pub async fn create_monthly_task(
  app: tauri::AppHandle,
  task: MonthlyTask,
) -> Result<MonthlyTaskPlanPreview, CommandError> {
  let paths = resolve_paths(&app)?;
  reading_task::create_monthly_task_with_plan(&paths, &task).map_err(CommandError::from)
}

#[tauri::command]
pub async fn delete_monthly_task(app: tauri::AppHandle, id: String) -> Result<(), CommandError> {
  let paths = resolve_paths(&app)?;
  reading_task::delete_monthly_task(&paths, &id).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn get_daily_progress(
  app: tauri::AppHandle,
  task_id: String,
  date: String,
) -> Result<Option<reading_task::DailyProgress>, CommandError> {
  let paths = resolve_paths(&app)?;
  let progress =
    reading_task::get_daily_progress(&paths, &task_id, &date).map_err(CommandError::from)?;
  Ok(progress)
}

#[tauri::command]
pub async fn run_daily_task(
  app: tauri::AppHandle,
  task_id: String,
  date: String,
) -> Result<TaskRunSummary, CommandError> {
  let paths = resolve_paths(&app)?;
  let pause_registry = app.state::<TaskPauseRegistry>();
  let pause_flag = pause_registry.register(&task_id);
  let handle = app.clone();
  let summary = reading_task::run_daily_task_with_progress_controlled(
    &paths,
    &task_id,
    &date,
    move |progress| {
      let _ = handle.emit("reading-task://progress", progress);
    },
    move || pause_flag.load(Ordering::SeqCst),
  )
  .await;
  pause_registry.clear(&task_id);
  let summary = summary.map_err(CommandError::from)?;
  Ok(summary)
}

#[tauri::command]
pub async fn pause_daily_task(
  app: tauri::AppHandle,
  task_id: String,
) -> Result<bool, CommandError> {
  let pause_registry = app.state::<TaskPauseRegistry>();
  Ok(pause_registry.pause(&task_id))
}

#[tauri::command]
pub async fn get_task_results(
  app: tauri::AppHandle,
  task_id: String,
) -> Result<Vec<reading_task::TaskItemResult>, CommandError> {
  let paths = resolve_paths(&app)?;
  let results = reading_task::get_task_results(&paths, &task_id).map_err(CommandError::from)?;
  Ok(results)
}
