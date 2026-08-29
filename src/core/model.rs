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

impl Default for AppPaths {
  fn default() -> Self {
    Self::new()
  }
}

fn default_db_path() -> Option<std::path::PathBuf> {
  std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".reading.sqlite"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRunRequest {
  pub s_course_id: String,
  pub s_manager_id: String,
  #[serde(default)]
  pub reading_url: String,
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
  #[serde(default)]
  pub result_id: Option<i64>,
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
  #[serde(default)]
  pub shop_name: String,
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

pub fn default_daily_task_run_status() -> String {
  "not_started".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FcRecord {
  pub name: String,
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
  pub fc_name: String,
  pub open_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthlyTask {
  pub id: String,
  pub fc_name: String,
  pub s_manager_id: String,
  pub s_course_id: String,
  #[serde(default)]
  pub reading_url: String,
  #[serde(default = "default_task_type")]
  pub task_type: String,
  pub total_target: usize,
  #[serde(default)]
  pub target_days: usize,
  pub created_at: String,
  #[serde(default)]
  pub shopcodes: Vec<String>,
  #[serde(default)]
  pub excluded_open_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyTask {
  pub task_id: String,
  pub date: String,
  pub target_count: usize,
  pub completed_count: usize,
  #[serde(default)]
  pub is_locked: bool,
  #[serde(default = "default_daily_task_run_status")]
  pub run_status: String,
  #[serde(default)]
  pub shopcodes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthlyTaskPlanPreview {
  pub eligible_shop_count: usize,
  pub total_target: usize,
  #[serde(default)]
  pub target_days: usize,
  #[serde(default)]
  pub shopcodes: Vec<String>,
  #[serde(default)]
  pub daily_plans: Vec<DailyTask>,
}

pub fn add_days_to_date(date: &str, delta_days: i64) -> String {
  if let Some((year, month, day)) = parse_date_parts(date) {
    let days = days_from_civil(year, month, day) + delta_days;
    let (new_year, new_month, new_day) = civil_from_days(days);
    return format!("{new_year:04}-{new_month:02}-{new_day:02}");
  }

  date.to_string()
}

pub fn parse_date_parts(date: &str) -> Option<(i32, u32, u32)> {
  let year = date.get(0..4)?.parse().ok()?;
  let month = date.get(5..7)?.parse().ok()?;
  let day = date.get(8..10)?.parse().ok()?;
  Some((year, month, day))
}

pub fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
  let year = year - if month <= 2 { 1 } else { 0 };
  let era = if year >= 0 { year } else { year - 399 } / 400;
  let yoe = year - era * 400;
  let month = month as i32;
  let day = day as i32;
  let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
  let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  (era * 146_097 + doe - 719_468) as i64
}

pub fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
  let z = days_since_unix_epoch + 719_468;
  let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
  let doe = z - era * 146_097;
  let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
  let y = yoe + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let d = doy - (153 * mp + 2) / 5 + 1;
  let m = mp + if mp < 10 { 3 } else { -9 };
  let year = y + if m <= 2 { 1 } else { 0 };
  (year as i32, m as u32, d as u32)
}
