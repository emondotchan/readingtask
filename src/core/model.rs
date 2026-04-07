use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppPaths {
  pub config_dir: PathBuf,
  pub db_path: PathBuf,
}

impl AppPaths {
  pub fn new(config_dir: impl Into<PathBuf>) -> Self {
    let config_dir = config_dir.into();
    let db_path = default_db_path().unwrap_or_else(|| config_dir.join("app_data.db"));
    Self {
      config_dir,
      db_path,
    }
  }

  pub fn new_with_db_path(config_dir: impl Into<PathBuf>, db_path: impl Into<PathBuf>) -> Self {
    Self {
      config_dir: config_dir.into(),
      db_path: db_path.into(),
    }
  }
}

fn default_db_path() -> Option<PathBuf> {
  std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".reading.db"))
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
pub enum TaskItemOutcome {
  Success,
  RequestError,
  ResponseReadError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskItemResult {
  pub index: usize,
  pub open_id: String,
  pub shop_code: String,
  pub province: String,
  pub city: String,
  pub http_status: Option<u16>,
  pub response_text: Option<String>,
  pub error_message: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopRecord {
  pub province: String,
  pub city: String,
  pub shop_code: String,
  pub fc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FcRecord {
  pub name: String,
  pub manager_id: String,
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
  pub total_target: usize,
  pub target_days: usize,
  pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyProgress {
  pub task_id: String,
  pub date: String,
  pub target_count: usize,
  pub completed_count: usize,
}
