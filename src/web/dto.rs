use crate as reading_task;
use reading_task::{CourseRecord, FcRecord, TaskRunRequest, TaskRunSummary};
use serde::{Deserialize, Serialize};

use crate::web::error::CommandError;

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

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReschedulePlansInput {
  pub start_date: Option<String>,
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
