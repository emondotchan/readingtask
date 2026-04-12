use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use log::Level;
use reading_task::{
  AppPaths, CourseRecord, DailyTask as DailyTask, DbContext, FcRecord, MonthlyTask,
  MonthlyTaskPlanPreview, OpenIdRecord, ShopRecord, TaskRunRequest, TaskRunSummary,
};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::bootstrap::{self, RuntimePaths};
use crate::error::CommandError;

fn log_command(level: Level, command: &str, message: impl AsRef<str>) {
  log::log!(
    target: &format!("reading_task::tauri::{command}"),
    level,
    "{}",
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
  inner: Mutex<RuntimeStateInner>,
}

#[derive(Debug)]
struct RuntimeStateInner {
  paths: RuntimePaths,
  db: Option<DbContext>,
}

impl RuntimeState {
  pub fn new(paths: RuntimePaths, db: Option<DbContext>) -> Self {
    Self {
      inner: Mutex::new(RuntimeStateInner { paths, db }),
    }
  }

  fn snapshot_paths(&self) -> Result<RuntimePaths, CommandError> {
    self
      .inner
      .lock()
      .map(|state| state.paths.clone())
      .map_err(|_| resource_error("运行时路径锁已损坏"))
  }

  fn snapshot_db(&self) -> Result<Option<DbContext>, CommandError> {
    self
      .inner
      .lock()
      .map(|state| state.db.clone())
      .map_err(|_| resource_error("运行时路径锁已损坏"))
  }

  fn replace_db(&self, db_path: PathBuf, db: DbContext) -> Result<RuntimePaths, CommandError> {
    let mut state = self
      .inner
      .lock()
      .map_err(|_| resource_error("运行时路径锁已损坏"))?;
    state.paths.db_path = Some(db_path);
    state.db = Some(db);
    Ok(state.paths.clone())
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
    return Err(validation_error(
      "SQLite 存储文件路径必须是文件，不能是目录",
    ));
  }

  if let Some(parent) = db_path.parent() {
    if !parent.as_os_str().is_empty() {
      fs::create_dir_all(parent)
        .map_err(|error| resource_error(format!("无法创建 SQLite 目录: {error}")))?;
    }
  }

  Ok(db_path)
}

fn build_runtime_status(paths: &RuntimePaths, db: Option<&DbContext>) -> RuntimeStatus {
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

fn resolve_db(app: &tauri::AppHandle) -> Result<DbContext, CommandError> {
  let runtime_state = app.state::<RuntimeState>();
  runtime_state
    .snapshot_db()?
    .ok_or_else(|| resource_error("请先在首页配置 SQLite 存储文件路径"))
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
  pub course_ready: bool,
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

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertFcInput {
  pub fc: FcRecord,
  pub previous_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertCourseInput {
  pub course: CourseRecord,
  pub previous_month: Option<String>,
  pub previous_course_id: Option<String>,
  pub previous_task_type: Option<String>,
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
  let runtime_paths = runtime_state.snapshot_paths()?;
  let db = runtime_state.snapshot_db()?;
  Ok(build_runtime_status(&runtime_paths, db.as_ref()))
}

#[tauri::command]
pub async fn set_sqlite_path(
  app: tauri::AppHandle,
  sqlite_path: String,
) -> Result<RuntimeStatus, CommandError> {
  let db_path = normalize_sqlite_path(&sqlite_path)?;
  let runtime_state = app.state::<RuntimeState>();
  let runtime_paths = runtime_state.snapshot_paths()?;
  let app_paths = AppPaths::new_with_db_path(db_path.clone());
  let db = reading_task::init_db_context(&app_paths).map_err(CommandError::from)?;

  bootstrap::save_sqlite_path(&runtime_paths.sqlite_settings_path, &db_path)
    .map_err(|error| resource_error(format!("保存 SQLite 路径失败: {error}")))?;

  let updated_paths = runtime_state.replace_db(db_path, db.clone())?;
  Ok(build_runtime_status(&updated_paths, Some(&db)))
}

#[tauri::command]
pub async fn run_reading_task(
  app: tauri::AppHandle,
  request: RunTaskInput,
) -> Result<TaskRunSummary, CommandError> {
  let db = resolve_db(&app)?;
  let run_date = request.run_date.clone();
  let task_request: TaskRunRequest = request.into();

  let handle = app.clone();
  let summary = reading_task::run_task_with_progress(
    &db,
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
  let db = resolve_db(&app)?;
  let ids = reading_task::get_all_open_id_records(&db).map_err(CommandError::from)?;
  Ok(ids)
}

#[tauri::command]
pub async fn add_open_id(app: tauri::AppHandle, open_id: OpenIdRecord) -> Result<(), CommandError> {
  let db = resolve_db(&app)?;
  reading_task::add_open_id(&db, &open_id).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn delete_open_id(app: tauri::AppHandle, open_id: String) -> Result<(), CommandError> {
  let db = resolve_db(&app)?;
  reading_task::delete_open_id(&db, &open_id).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn import_open_ids_csv(
  app: tauri::AppHandle,
  csv_text: String,
) -> Result<usize, CommandError> {
  let db = resolve_db(&app)?;
  reading_task::import_open_ids_csv(&db, &csv_text).map_err(CommandError::from)
}

#[tauri::command]
pub async fn get_shops(app: tauri::AppHandle) -> Result<Vec<ShopRecord>, CommandError> {
  let db = resolve_db(&app)?;
  let shops = reading_task::get_all_shops(&db).map_err(CommandError::from)?;
  Ok(shops)
}

#[tauri::command]
pub async fn add_or_update_shop(
  app: tauri::AppHandle,
  shop: ShopRecord,
) -> Result<(), CommandError> {
  let db = resolve_db(&app)?;
  reading_task::add_or_update_shop(&db, &shop).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn delete_shop(app: tauri::AppHandle, shop_code: String) -> Result<(), CommandError> {
  let db = resolve_db(&app)?;
  reading_task::delete_shop(&db, &shop_code).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn get_fcs(app: tauri::AppHandle) -> Result<Vec<FcRecord>, CommandError> {
  let db = resolve_db(&app)?;
  log_command(
    Level::Debug,
    "get_fcs",
    format!("db_path={}", db.db_path().display()),
  );
  let fcs = reading_task::get_all_fcs(&db).map_err(|error| {
    log_command(Level::Error, "get_fcs", error.to_string());
    CommandError::from(error)
  })?;
  log_command(
    Level::Debug,
    "get_fcs",
    format!("loaded {} fc records", fcs.len()),
  );
  Ok(fcs)
}

#[tauri::command]
pub async fn add_or_update_fc(
  app: tauri::AppHandle,
  fc: UpsertFcInput,
) -> Result<(), CommandError> {
  let db = resolve_db(&app)?;
  reading_task::add_or_update_fc(&db, fc.previous_name.as_deref(), &fc.fc)
    .map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn delete_fc(app: tauri::AppHandle, name: String) -> Result<(), CommandError> {
  let db = resolve_db(&app)?;
  reading_task::delete_fc(&db, &name).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn get_courses(app: tauri::AppHandle) -> Result<Vec<CourseRecord>, CommandError> {
  let db = resolve_db(&app)?;
  reading_task::get_all_courses(&db).map_err(CommandError::from)
}

#[tauri::command]
pub async fn add_or_update_course(
  app: tauri::AppHandle,
  course: UpsertCourseInput,
) -> Result<(), CommandError> {
  let db = resolve_db(&app)?;
  reading_task::add_or_update_course(
    &db,
    match (
      course.previous_month.as_deref(),
      course.previous_course_id.as_deref(),
      course.previous_task_type.as_deref(),
    ) {
      (Some(month), Some(course_id), Some(task_type)) => Some((month, course_id, task_type)),
      _ => None,
    },
    &course.course,
  )
  .map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn delete_course(
  app: tauri::AppHandle,
  month: String,
  course_id: String,
  task_type: String,
) -> Result<(), CommandError> {
  let db = resolve_db(&app)?;
  reading_task::delete_course(&db, &month, &course_id, &task_type).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn get_shop_count(
  app: tauri::AppHandle,
  fc_name: String,
  task_type: String,
) -> Result<usize, CommandError> {
  let db = resolve_db(&app)?;
  log_command(
    Level::Debug,
    "get_shop_count",
    format!("fc_name={fc_name} task_type={task_type}"),
  );
  let count = reading_task::get_shop_count_by_fc_and_type(&db, &fc_name, &task_type)
    .map_err(CommandError::from)?;
  log_command(Level::Debug, "get_shop_count", format!("count={count}"));
  Ok(count)
}

#[tauri::command]
pub async fn preview_monthly_task_plan(
  app: tauri::AppHandle,
  task: MonthlyTask,
) -> Result<MonthlyTaskPlanPreview, CommandError> {
  let db = resolve_db(&app)?;
  reading_task::preview_monthly_task_plan(&db, &task).map_err(CommandError::from)
}

#[tauri::command]
pub async fn get_monthly_tasks(
  app: tauri::AppHandle,
) -> Result<Vec<reading_task::MonthlyTask>, CommandError> {
  let db = resolve_db(&app)?;
  log_command(
    Level::Debug,
    "get_monthly_tasks",
    format!("db_path={}", db.db_path().display()),
  );
  let tasks = reading_task::get_all_monthly_tasks(&db).map_err(|error| {
    log_command(Level::Error, "get_monthly_tasks", error.to_string());
    CommandError::from(error)
  })?;
  log_command(
    Level::Debug,
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
  let db = resolve_db(&app)?;
  reading_task::create_monthly_task_with_plan(&db, &task).map_err(CommandError::from)
}

#[tauri::command]
pub async fn delete_monthly_task(app: tauri::AppHandle, id: String) -> Result<(), CommandError> {
  let db = resolve_db(&app)?;
  reading_task::delete_monthly_task(&db, &id).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn get_daily_task(
  app: tauri::AppHandle,
  task_id: String,
  date: String,
) -> Result<Option<DailyTask>, CommandError> {
  let db = resolve_db(&app)?;
  let progress =
    reading_task::get_daily_task(&db, &task_id, &date).map_err(CommandError::from)?;
  Ok(progress)
}

#[tauri::command]
pub async fn get_task_daily_tasks(
  app: tauri::AppHandle,
  task_id: String,
) -> Result<Vec<DailyTask>, CommandError> {
  let db = resolve_db(&app)?;
  reading_task::get_all_daily_tasks_for_task(&db, &task_id).map_err(CommandError::from)
}

#[tauri::command]
pub async fn save_daily_task(
  app: tauri::AppHandle,
  task: DailyTask,
) -> Result<(), CommandError> {
  let db = resolve_db(&app)?;
  let existing = reading_task::get_daily_task(&db, &task.task_id, &task.date)
    .map_err(CommandError::from)?
    .ok_or_else(|| resource_error("未找到对应的每日任务进度"))?;

  if existing.is_locked {
    return Err(validation_error("已执行的每日任务不可编辑"));
  }

  let updated = DailyTask {
    shopcodes: task.shopcodes,
    ..existing
  };

  reading_task::save_daily_task(&db, &updated).map_err(CommandError::from)?;
  Ok(())
}

#[tauri::command]
pub async fn run_daily_task(
  app: tauri::AppHandle,
  task_id: String,
  date: String,
) -> Result<TaskRunSummary, CommandError> {
  let db = resolve_db(&app)?;
  let pause_registry = app.state::<TaskPauseRegistry>();
  let pause_flag = pause_registry.register(&task_id);
  let handle = app.clone();
  let summary = reading_task::run_daily_task_with_progress_controlled(
    &db,
    &task_id,
    &date,
    move |progress| {
      let _ = handle.emit("reading-task://progress", progress);
    },
    move || pause_flag.load(Ordering::SeqCst),
  )
  .await;
  pause_registry.clear(&task_id);
  let summary = summary.map_err(|e| {
    log_command(
      log::Level::Error,
      "run_daily_task",
      format!("执行任务失败: {:?}", e),
    );
    CommandError::from(e)
  })?;

  log_command(log::Level::Debug, "run_daily_task", "执行日常任务成功");
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
  let db = resolve_db(&app)?;
  let results = reading_task::get_task_results(&db, &task_id).map_err(CommandError::from)?;
  Ok(results)
}
