mod bootstrap;
mod error;

use crate as reading_task;
use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::path::{Component, Path as StdPath, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::{
  Json, Router,
  routing::{delete, get, post},
};
use include_dir::{Dir, include_dir};
use log::Level;
use reading_task::{
  AppPaths, CourseRecord, DailyTask, DbContext, FcRecord, MonthlyTask, MonthlyTaskPlanPreview,
  OpenIdRecord, ShopRecord, TaskProgress, TaskRunRequest, TaskRunSummary,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

use crate::web::bootstrap::RuntimePaths;
use crate::web::error::CommandError;

static WEB_DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/web/dist");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunTaskInput {
  pub s_course_id: String,
  pub s_manager_id: String,
  #[serde(default)]
  pub reading_url: String,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSqlitePathInput {
  pub sqlite_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteCourseQuery {
  pub month: String,
  pub course_id: String,
  pub task_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShopCountQuery {
  #[serde(rename = "fcName")]
  pub fc_name: String,
  pub task_type: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateShopTypesInput {
  pub shop_codes: Vec<String>,
  pub shop_type: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DailyTaskQuery {
  pub date: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRunDailyTasksInput {
  pub task_ids: Vec<String>,
  pub date: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRunDailyTasksResponse {
  pub accepted_count: usize,
  pub skipped_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyTaskRunSnapshot {
  pub task_id: String,
  pub date: String,
  pub run_state: String,
  pub processed_count: usize,
  pub requested_count: usize,
  pub items: Vec<reading_task::TaskItemResult>,
  pub summary: Option<TaskRunSummary>,
  pub error: Option<CommandError>,
}

#[derive(Debug)]
pub struct RuntimeStateInner {
  paths: RuntimePaths,
  db: Option<DbContext>,
}

#[derive(Debug, Default)]
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
    if let Ok(flags) = self.flags.lock()
      && let Some(flag) = flags.get(task_id)
    {
      flag.store(true, Ordering::SeqCst);
      return true;
    }

    false
  }

  fn clear(&self, task_id: &str) {
    if let Ok(mut flags) = self.flags.lock() {
      flags.remove(task_id);
    }
  }
}

#[derive(Debug, Default)]
pub struct TaskRunRegistry {
  snapshots: Mutex<HashMap<String, DailyTaskRunSnapshot>>,
}

impl TaskRunRegistry {
  fn is_running(&self, task_id: &str) -> bool {
    self
      .snapshots
      .lock()
      .ok()
      .and_then(|snapshots| snapshots.get(task_id).cloned())
      .is_some_and(|snapshot| snapshot.run_state == "running")
  }

  fn start(&self, task_id: &str, date: &str) {
    if let Ok(mut snapshots) = self.snapshots.lock() {
      snapshots.insert(
        task_id.to_string(),
        DailyTaskRunSnapshot {
          task_id: task_id.to_string(),
          date: date.to_string(),
          run_state: "running".to_string(),
          processed_count: 0,
          requested_count: 0,
          items: Vec::new(),
          summary: None,
          error: None,
        },
      );
    }
  }

  fn record_progress(&self, progress: TaskProgress) {
    let Some(task_id) = progress.task_id else {
      return;
    };

    if let Ok(mut snapshots) = self.snapshots.lock() {
      let snapshot = snapshots
        .entry(task_id.clone())
        .or_insert_with(|| DailyTaskRunSnapshot {
          task_id: task_id.clone(),
          date: String::new(),
          run_state: "running".to_string(),
          processed_count: 0,
          requested_count: 0,
          items: Vec::new(),
          summary: None,
          error: None,
        });
      snapshot.run_state = "running".to_string();
      snapshot.processed_count = progress.processed_count;
      snapshot.requested_count = progress.requested_count;
      snapshot.items.push(progress.latest_item);
      snapshot.error = None;
    }
  }

  fn finish_success(&self, task_id: &str, date: &str, summary: TaskRunSummary) {
    if let Ok(mut snapshots) = self.snapshots.lock() {
      snapshots.insert(
        task_id.to_string(),
        DailyTaskRunSnapshot {
          task_id: task_id.to_string(),
          date: date.to_string(),
          run_state: "completed".to_string(),
          processed_count: summary.processed_count,
          requested_count: summary.requested_count,
          items: summary.items.clone(),
          summary: Some(summary),
          error: None,
        },
      );
    }
  }

  fn finish_error(&self, task_id: &str, date: &str, error: CommandError) {
    let run_state = if error.category == "paused" {
      "paused"
    } else if error.category == "completed" && error.message.contains("该月度任务已全部完成")
    {
      "monthly-completed"
    } else if error.category == "completed" {
      "completed"
    } else {
      "error"
    };

    if let Ok(mut snapshots) = self.snapshots.lock() {
      let snapshot = snapshots
        .entry(task_id.to_string())
        .or_insert_with(|| DailyTaskRunSnapshot {
          task_id: task_id.to_string(),
          date: date.to_string(),
          run_state: run_state.to_string(),
          processed_count: 0,
          requested_count: 0,
          items: Vec::new(),
          summary: None,
          error: None,
        });
      snapshot.date = date.to_string();
      snapshot.run_state = run_state.to_string();
      snapshot.error = if error.category == "completed" {
        None
      } else {
        Some(error)
      };
    }
  }

  fn mark_paused(&self, task_id: &str) -> Option<String> {
    if let Ok(mut snapshots) = self.snapshots.lock()
      && let Some(snapshot) = snapshots.get_mut(task_id)
      && snapshot.run_state == "running"
    {
      snapshot.run_state = "paused".to_string();
      return Some(snapshot.date.clone());
    }
    None
  }

  fn snapshots(&self) -> Vec<DailyTaskRunSnapshot> {
    self
      .snapshots
      .lock()
      .map(|snapshots| snapshots.values().cloned().collect())
      .unwrap_or_default()
  }
}

#[derive(Debug, Clone)]
pub struct AppState {
  inner: Arc<Mutex<RuntimeStateInner>>,
  pause_registry: Arc<TaskPauseRegistry>,
  run_registry: Arc<TaskRunRegistry>,
}

impl AppState {
  fn new(paths: RuntimePaths, db: Option<DbContext>) -> Self {
    Self {
      inner: Arc::new(Mutex::new(RuntimeStateInner { paths, db })),
      pause_registry: Arc::new(TaskPauseRegistry::default()),
      run_registry: Arc::new(TaskRunRegistry::default()),
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

  fn resolve_db(&self) -> Result<DbContext, CommandError> {
    self
      .snapshot_db()?
      .ok_or_else(|| resource_error("请先在首页配置 SQLite 存储文件路径"))
  }
}

#[derive(Debug, Clone, serde::Serialize)]
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

fn log_command(level: Level, command: &str, message: impl AsRef<str>) {
  log::log!(
    target: &format!("reading_task::web::{command}"),
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

  if let Some(parent) = db_path.parent()
    && !parent.as_os_str().is_empty()
  {
    fs::create_dir_all(parent)
      .map_err(|error| resource_error(format!("无法创建 SQLite 目录: {error}")))?;
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

impl From<RunTaskInput> for TaskRunRequest {
  fn from(input: RunTaskInput) -> Self {
    Self {
      s_course_id: input.s_course_id,
      s_manager_id: input.s_manager_id,
      reading_url: input.reading_url,
      fc: input.fc,
      count: input.count,
      shopcodes: input.shopcodes,
    }
  }
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
  reading_task::init_logging();

  let runtime_paths = bootstrap::initialize()?;
  let db = runtime_paths
    .db_path
    .as_ref()
    .map(|db_path| reading_task::init_db_context(&AppPaths::new_with_db_path(db_path.clone())))
    .transpose()?;
  let state = AppState::new(runtime_paths, db);

  let api_router = Router::new()
    .route("/health", get(health))
    .route("/runtime-status", get(get_runtime_status))
    .route("/sqlite-path", post(set_sqlite_path))
    .route("/run-reading-task", post(run_reading_task))
    .route("/open-ids", get(get_open_ids).post(add_open_id))
    .route("/open-ids/{open_id}", delete(delete_open_id))
    .route("/shops", get(get_shops).delete(delete_all_shops))
    .route("/shops/import", post(import_shops))
    .route("/shops/shop-types", post(update_shop_types))
    .route("/fcs", get(get_fcs).post(add_or_update_fc))
    .route("/fcs/{name}", delete(delete_fc))
    .route(
      "/courses",
      get(get_courses)
        .post(add_or_update_course)
        .delete(delete_course),
    )
    .route("/shop-count", get(get_shop_count))
    .route(
      "/monthly-tasks",
      get(get_monthly_tasks).post(create_monthly_task),
    )
    .route("/monthly-tasks/preview", post(preview_monthly_task_plan))
    .route("/monthly-tasks/{id}", delete(delete_monthly_task))
    .route(
      "/daily-tasks/{task_id}",
      get(get_daily_task).post(save_daily_task),
    )
    .route("/daily-tasks/{task_id}/all", get(get_task_daily_tasks))
    .route("/daily-tasks/batch-run", post(batch_run_daily_tasks))
    .route("/daily-tasks/run-status", get(get_daily_task_run_status))
    .route("/daily-tasks/{task_id}/run", post(run_daily_task))
    .route("/daily-tasks/{task_id}/pause", post(pause_daily_task))
    .route("/tasks/{task_id}/results", get(get_task_results))
    .with_state(state.clone());

  let app = Router::new()
    .nest("/api", api_router)
    .fallback(serve_embedded_static)
    .layer(CorsLayer::permissive())
    .with_state(state);

  let addr: SocketAddr = std::env::var("READING_TASK_BIND")
    .ok()
    .and_then(|value| value.parse().ok())
    .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 10086)));
  let listener = TcpListener::bind(addr).await?;
  log::info!("web server listening on http://{}", listener.local_addr()?);
  axum::serve(listener, app).await?;
  Ok(())
}

async fn serve_embedded_static(uri: Uri) -> Response {
  let request_path = uri.path().trim_start_matches('/');
  let asset_path = if request_path.is_empty() {
    "index.html"
  } else {
    request_path
  };

  match embedded_asset(asset_path).or_else(|| embedded_asset("index.html")) {
    Some((path, contents)) => {
      let content_type = mime_guess::from_path(path).first_or_octet_stream();
      ([(header::CONTENT_TYPE, content_type.as_ref())], contents).into_response()
    }
    None => StatusCode::NOT_FOUND.into_response(),
  }
}

fn embedded_asset(path: &str) -> Option<(&'static StdPath, &'static [u8])> {
  let clean_path = sanitize_embedded_path(path)?;
  let file = WEB_DIST.get_file(clean_path)?;
  Some((file.path(), file.contents()))
}

fn sanitize_embedded_path(path: &str) -> Option<&str> {
  let path = path.trim_start_matches('/');
  if path.is_empty()
    || StdPath::new(path)
      .components()
      .any(|component| !matches!(component, Component::Normal(_)))
  {
    return None;
  }
  Some(path)
}

async fn health() -> &'static str {
  "ok"
}

async fn get_runtime_status(
  State(state): State<AppState>,
) -> Result<Json<RuntimeStatus>, CommandError> {
  let runtime_paths = state.snapshot_paths()?;
  let db = state.snapshot_db()?;
  Ok(Json(build_runtime_status(&runtime_paths, db.as_ref())))
}

async fn set_sqlite_path(
  State(state): State<AppState>,
  Json(input): Json<SaveSqlitePathInput>,
) -> Result<Json<RuntimeStatus>, CommandError> {
  let db_path = normalize_sqlite_path(&input.sqlite_path)?;
  let runtime_paths = state.snapshot_paths()?;
  let app_paths = AppPaths::new_with_db_path(db_path.clone());
  let db = reading_task::init_db_context(&app_paths).map_err(CommandError::from)?;

  bootstrap::save_sqlite_path(&runtime_paths.sqlite_settings_path, &db_path)
    .map_err(|error| resource_error(format!("保存 SQLite 路径失败: {error}")))?;

  let updated_paths = state.replace_db(db_path, db.clone())?;
  Ok(Json(build_runtime_status(&updated_paths, Some(&db))))
}

async fn run_reading_task(
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

async fn get_open_ids(
  State(state): State<AppState>,
) -> Result<Json<Vec<OpenIdRecord>>, CommandError> {
  let db = state.resolve_db()?;
  let ids = reading_task::get_all_open_id_records(&db).map_err(CommandError::from)?;
  Ok(Json(ids))
}

async fn add_open_id(
  State(state): State<AppState>,
  Json(open_id): Json<OpenIdRecord>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  reading_task::add_open_id(&db, &open_id).map_err(CommandError::from)?;
  Ok(StatusCode::NO_CONTENT)
}

async fn delete_open_id(
  State(state): State<AppState>,
  Path(open_id): Path<String>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  reading_task::delete_open_id(&db, &open_id).map_err(CommandError::from)?;
  Ok(StatusCode::NO_CONTENT)
}

async fn get_shops(State(state): State<AppState>) -> Result<Json<Vec<ShopRecord>>, CommandError> {
  let db = state.resolve_db()?;
  let shops = reading_task::get_all_shops(&db).map_err(CommandError::from)?;
  Ok(Json(shops))
}

async fn import_shops(
  State(state): State<AppState>,
  Json(shops): Json<Vec<ShopRecord>>,
) -> Result<Json<usize>, CommandError> {
  if shops.is_empty() {
    return Err(validation_error("导入门店不能为空"));
  }

  if shops.iter().any(|shop| shop.shop_code.trim().is_empty()) {
    return Err(validation_error("导入门店存在空的 ShopCode"));
  }

  let db = state.resolve_db()?;
  let imported = reading_task::import_shops(&db, &shops).map_err(CommandError::from)?;
  Ok(Json(imported))
}

async fn update_shop_types(
  State(state): State<AppState>,
  Json(input): Json<UpdateShopTypesInput>,
) -> Result<Json<usize>, CommandError> {
  let shop_codes = input
    .shop_codes
    .iter()
    .map(|shop_code| shop_code.trim().to_string())
    .filter(|shop_code| !shop_code.is_empty())
    .collect::<Vec<_>>();

  if shop_codes.is_empty() {
    return Err(validation_error("更新门店类型不能为空"));
  }

  let db = state.resolve_db()?;
  let updated = reading_task::update_shop_type_by_codes(&db, &shop_codes, input.shop_type)
    .map_err(CommandError::from)?;
  Ok(Json(updated))
}

async fn delete_all_shops(State(state): State<AppState>) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  reading_task::delete_all_shops(&db).map_err(CommandError::from)?;
  Ok(StatusCode::NO_CONTENT)
}

async fn get_fcs(State(state): State<AppState>) -> Result<Json<Vec<FcRecord>>, CommandError> {
  let db = state.resolve_db()?;
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
  Ok(Json(fcs))
}

async fn add_or_update_fc(
  State(state): State<AppState>,
  Json(input): Json<UpsertFcInput>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  reading_task::add_or_update_fc(&db, input.previous_name.as_deref(), &input.fc)
    .map_err(CommandError::from)?;
  Ok(StatusCode::NO_CONTENT)
}

async fn delete_fc(
  State(state): State<AppState>,
  Path(name): Path<String>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  reading_task::delete_fc(&db, &name).map_err(CommandError::from)?;
  Ok(StatusCode::NO_CONTENT)
}

async fn get_courses(
  State(state): State<AppState>,
) -> Result<Json<Vec<CourseRecord>>, CommandError> {
  let db = state.resolve_db()?;
  Ok(Json(
    reading_task::get_all_courses(&db).map_err(CommandError::from)?,
  ))
}

async fn add_or_update_course(
  State(state): State<AppState>,
  Json(course): Json<UpsertCourseInput>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
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
  Ok(StatusCode::NO_CONTENT)
}

async fn delete_course(
  State(state): State<AppState>,
  Query(query): Query<DeleteCourseQuery>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  reading_task::delete_course(&db, &query.month, &query.course_id, &query.task_type)
    .map_err(CommandError::from)?;
  Ok(StatusCode::NO_CONTENT)
}

async fn get_shop_count(
  State(state): State<AppState>,
  Query(query): Query<ShopCountQuery>,
) -> Result<Json<usize>, CommandError> {
  let db = state.resolve_db()?;
  log_command(
    Level::Debug,
    "get_shop_count",
    format!("fc_name={} task_type={}", query.fc_name, query.task_type),
  );
  let count = reading_task::get_shop_count_by_fc_and_type(&db, &query.fc_name, &query.task_type)
    .map_err(CommandError::from)?;
  log_command(Level::Debug, "get_shop_count", format!("count={count}"));
  Ok(Json(count))
}

async fn preview_monthly_task_plan(
  State(state): State<AppState>,
  Json(task): Json<MonthlyTask>,
) -> Result<Json<MonthlyTaskPlanPreview>, CommandError> {
  let db = state.resolve_db()?;
  Ok(Json(
    reading_task::preview_monthly_task_plan(&db, &task).map_err(CommandError::from)?,
  ))
}

async fn get_monthly_tasks(
  State(state): State<AppState>,
) -> Result<Json<Vec<reading_task::MonthlyTask>>, CommandError> {
  let db = state.resolve_db()?;
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
  Ok(Json(tasks))
}

async fn create_monthly_task(
  State(state): State<AppState>,
  Json(task): Json<MonthlyTask>,
) -> Result<Json<MonthlyTaskPlanPreview>, CommandError> {
  let db = state.resolve_db()?;
  Ok(Json(
    reading_task::create_monthly_task_with_plan(&db, &task).map_err(CommandError::from)?,
  ))
}

async fn delete_monthly_task(
  State(state): State<AppState>,
  Path(id): Path<String>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  reading_task::delete_monthly_task(&db, &id).map_err(CommandError::from)?;
  Ok(StatusCode::NO_CONTENT)
}

async fn get_daily_task(
  State(state): State<AppState>,
  Path(task_id): Path<String>,
  Query(query): Query<DailyTaskQuery>,
) -> Result<Json<Option<DailyTask>>, CommandError> {
  let db = state.resolve_db()?;
  let progress =
    reading_task::get_daily_task(&db, &task_id, &query.date).map_err(CommandError::from)?;
  Ok(Json(progress))
}

async fn get_task_daily_tasks(
  State(state): State<AppState>,
  Path(task_id): Path<String>,
) -> Result<Json<Vec<DailyTask>>, CommandError> {
  let db = state.resolve_db()?;
  Ok(Json(
    reading_task::get_all_daily_tasks_for_task(&db, &task_id).map_err(CommandError::from)?,
  ))
}

async fn save_daily_task(
  State(state): State<AppState>,
  Path(task_id): Path<String>,
  Json(task): Json<DailyTask>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  let existing = reading_task::get_daily_task(&db, &task_id, &task.date)
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
  Ok(StatusCode::NO_CONTENT)
}

async fn run_daily_task(
  State(state): State<AppState>,
  Path(task_id): Path<String>,
  Query(query): Query<DailyTaskQuery>,
) -> Result<Json<TaskRunSummary>, CommandError> {
  let db = state.resolve_db()?;
  let pause_flag = state.pause_registry.register(&task_id);
  state.run_registry.start(&task_id, &query.date);
  let run_registry = Arc::clone(&state.run_registry);
  let summary = reading_task::run_daily_task_with_progress_controlled(
    &db,
    &task_id,
    &query.date,
    move |progress| {
      run_registry.record_progress(progress);
    },
    move || pause_flag.load(Ordering::SeqCst),
  )
  .await;
  state.pause_registry.clear(&task_id);
  let summary = summary.map_err(|e| {
    let command_error = CommandError::from(e);
    if command_error.category == "completed" {
      log_command(
        log::Level::Warn,
        "run_daily_task",
        format!("任务无需继续执行: {}", command_error.message),
      );
    } else {
      log_command(
        log::Level::Error,
        "run_daily_task",
        format!("执行任务失败: {}", command_error.message),
      );
    }
    state
      .run_registry
      .finish_error(&task_id, &query.date, command_error.clone());
    command_error
  })?;

  state
    .run_registry
    .finish_success(&task_id, &query.date, summary.clone());
  log_command(log::Level::Debug, "run_daily_task", "执行日常任务成功");
  Ok(Json(summary))
}

async fn batch_run_daily_tasks(
  State(state): State<AppState>,
  Json(input): Json<BatchRunDailyTasksInput>,
) -> Result<Json<BatchRunDailyTasksResponse>, CommandError> {
  let db = state.resolve_db()?;
  let mut accepted_count = 0_usize;
  let mut skipped_count = 0_usize;

  for task_id in input.task_ids {
    if task_id.trim().is_empty() || state.run_registry.is_running(&task_id) {
      skipped_count += 1;
      continue;
    }

    accepted_count += 1;
    state.run_registry.start(&task_id, &input.date);
    let state_for_task = state.clone();
    let db_for_task = db.clone();
    let date = input.date.clone();
    tokio::spawn(async move {
      run_daily_task_background(state_for_task, db_for_task, task_id, date).await;
    });
  }

  Ok(Json(BatchRunDailyTasksResponse {
    accepted_count,
    skipped_count,
  }))
}

async fn run_daily_task_background(state: AppState, db: DbContext, task_id: String, date: String) {
  let pause_flag = state.pause_registry.register(&task_id);
  let run_registry = Arc::clone(&state.run_registry);
  let result = reading_task::run_daily_task_with_progress_controlled(
    &db,
    &task_id,
    &date,
    move |progress| {
      run_registry.record_progress(progress);
    },
    move || pause_flag.load(Ordering::SeqCst),
  )
  .await;
  state.pause_registry.clear(&task_id);

  match result {
    Ok(summary) => {
      state
        .run_registry
        .finish_success(&task_id, &date, summary.clone());
      log_command(
        log::Level::Debug,
        "batch_run_daily_tasks",
        format!("任务 {} 执行完成", task_id),
      );
    }
    Err(error) => {
      let command_error = CommandError::from(error);
      if command_error.category == "completed" {
        log_command(
          log::Level::Warn,
          "batch_run_daily_tasks",
          format!("任务 {} 无需继续执行: {}", task_id, command_error.message),
        );
      } else {
        log_command(
          log::Level::Error,
          "batch_run_daily_tasks",
          format!("任务 {} 执行失败: {}", task_id, command_error.message),
        );
      }
      state
        .run_registry
        .finish_error(&task_id, &date, command_error);
    }
  }
}

async fn get_daily_task_run_status(
  State(state): State<AppState>,
) -> Json<Vec<DailyTaskRunSnapshot>> {
  Json(state.run_registry.snapshots())
}

async fn pause_daily_task(
  State(state): State<AppState>,
  Path(task_id): Path<String>,
) -> Result<Json<bool>, CommandError> {
  let paused = state.pause_registry.pause(&task_id);
  if paused {
    if let Some(date) = state.run_registry.mark_paused(&task_id)
      && let Ok(db) = state.resolve_db()
      && let Err(error) = reading_task::update_daily_task_run_status(&db, &task_id, &date, "paused")
    {
      log_command(
        log::Level::Error,
        "pause_daily_task",
        format!("更新任务暂停状态失败: {}", error),
      );
    }
  }
  Ok(Json(paused))
}

async fn get_task_results(
  State(state): State<AppState>,
  Path(task_id): Path<String>,
) -> Result<Json<Vec<reading_task::TaskItemResult>>, CommandError> {
  let db = state.resolve_db()?;
  let results = reading_task::get_task_results(&db, &task_id).map_err(CommandError::from)?;
  Ok(Json(results))
}
