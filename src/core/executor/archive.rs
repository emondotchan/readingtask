use super::planner::resolve_request_reading_link;
use super::runner::{current_date_string, task_month_prefix_from_date};
use crate::core::db::DbContext;
use crate::core::error::AppError;
use crate::core::model::{
  QuickRunArchiveResult, QuickRunArchiveStatus, TaskItemResult, TaskRunRequest,
};

pub(crate) fn archive_quick_run_results(
  db: &DbContext,
  request: &TaskRunRequest,
  items: &[TaskItemResult],
  run_date: Option<&str>,
) -> Result<QuickRunArchiveResult, AppError> {
  let run_date = run_date
    .map(ToOwned::to_owned)
    .unwrap_or_else(current_date_string);
  archive_quick_run_results_for_date(db, request, items, &run_date)
}

pub(crate) fn archive_quick_run_results_for_date(
  db: &DbContext,
  request: &TaskRunRequest,
  items: &[TaskItemResult],
  run_date: &str,
) -> Result<QuickRunArchiveResult, AppError> {
  let month_prefix = task_month_prefix_from_date(run_date)?;
  let reading_link = resolve_request_reading_link(request)?;
  let matched_tasks = crate::core::db::find_monthly_tasks_by_month_fc_course(
    db,
    &month_prefix,
    &request.fc,
    &reading_link.s_course_id,
  )?;

  if matched_tasks.is_empty() {
    return Ok(QuickRunArchiveResult {
      status: QuickRunArchiveStatus::NoMatchingTask,
      task_id: None,
      message: format!(
        "未找到可追加的月度任务：{} / {} / {}",
        month_prefix, request.fc, reading_link.s_course_id
      ),
    });
  }

  if matched_tasks.len() > 1 {
    return Ok(QuickRunArchiveResult {
      status: QuickRunArchiveStatus::DuplicateTasks,
      task_id: None,
      message: format!(
        "检测到重复月度任务：{} / {} / {}，请先清理重复数据",
        month_prefix, request.fc, reading_link.s_course_id
      ),
    });
  }

  let task = &matched_tasks[0];
  crate::core::db::save_task_results(db, &task.id, items)?;
  crate::core::db::reconcile_daily_task_progress_for_task(db, &task.id)?;

  Ok(QuickRunArchiveResult {
    status: QuickRunArchiveStatus::Archived,
    task_id: Some(task.id.clone()),
    message: format!("已追加到月度任务 {}", task.id),
  })
}
