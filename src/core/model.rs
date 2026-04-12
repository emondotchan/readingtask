use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppPaths {
  pub db_path: std::path::PathBuf,
}

impl AppPaths {
  pub fn new() -> Self {
    let db_path = default_db_path().unwrap_or_else(|| std::path::PathBuf::from("app_data.db"));
    Self { db_path }
  }

  pub fn new_with_db_path(db_path: impl Into<std::path::PathBuf>) -> Self {
    Self {
      db_path: db_path.into(),
    }
  }
}

fn default_db_path() -> Option<std::path::PathBuf> {
  std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".reading.sqlite"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRunRequest {
  pub s_course_id: String,
  pub s_manager_id: String,
  pub fc: String,
  pub count: usize,
  pub shopcodes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum TaskItemOutcome {
  Success = 0,
  RequestError = 1,
  ResponseReadError = 2,
}

impl From<i32> for TaskItemOutcome {
  fn from(value: i32) -> Self {
    match value {
      1 => TaskItemOutcome::RequestError,
      2 => TaskItemOutcome::ResponseReadError,
      _ => TaskItemOutcome::Success,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskItemResult {
  pub index: usize,
  #[serde(default)]
  pub executed_date: Option<String>,
  #[serde(default)]
  pub submit_err: Option<i32>,
  #[serde(default)]
  pub rtn_msg: Option<String>,
  #[serde(default)]
  pub read_id: Option<String>,
  pub open_id: String,
  pub shop_code: String,
  pub province: String,
  pub city: String,
  pub http_status: Option<u16>,
  pub response_text: Option<String>,
  pub outcome: TaskItemOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRunSummary {
  pub requested_count: usize,
  pub processed_count: usize,
  pub success_count: usize,
  pub failure_count: usize,
  pub started_at: String,
  pub finished_at: String,
  pub items: Vec<TaskItemResult>,
  pub archive_result: Option<QuickRunArchiveResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickRunArchiveResult {
  pub status: QuickRunArchiveStatus,
  pub task_id: Option<String>,
  pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuickRunArchiveStatus {
  Archived,
  NoMatchingTask,
  DuplicateTasks,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskProgress {
  pub task_id: Option<String>,
  pub processed_count: usize,
  pub requested_count: usize,
  pub latest_item: TaskItemResult,
}

pub const SHOP_TYPE_AVENE: u8 = 0;
pub const SHOP_TYPE_KLORANE: u8 = 1;
pub const SHOP_TYPE_AVENE_KLORANE: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopRecord {
  pub province: String,
  pub city: String,
  pub shop_code: String,
  pub fc: Option<String>,
  #[serde(default = "default_shop_type")]
  pub shop_type: u8,
}

fn default_shop_type() -> u8 {
  SHOP_TYPE_AVENE
}

fn default_task_type() -> String {
  "Avene".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FcRecord {
  pub name: String,
  pub manager_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseRecord {
  pub month: String,
  pub course_id: String,
  #[serde(default = "default_task_type")]
  pub task_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenIdRecord {
  pub manager_id: String,
  pub open_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthlyTask {
  pub id: String,
  pub fc_name: String,
  pub s_manager_id: String,
  pub s_course_id: String,
  #[serde(default = "default_task_type")]
  pub task_type: String,
  pub total_target: usize,
  pub target_days: usize,
  pub created_at: String,
  #[serde(default)]
  pub shopcodes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyTask {
  pub task_id: String,
  pub date: String,
  pub target_count: usize,
  pub completed_count: usize,
  #[serde(default)]
  pub is_locked: bool,
  #[serde(default)]
  pub shopcodes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthlyTaskPlanPreview {
  pub eligible_shop_count: usize,
  pub total_target: usize,
  pub target_days: usize,
  #[serde(default)]
  pub daily_plans: Vec<DailyTask>,
}
