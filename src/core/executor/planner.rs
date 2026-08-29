use chrono::{DateTime, Local, Timelike};
use rand::RngExt;
use reqwest::Url;

use super::selector::{
  filter_task_shops, normalize_shopcodes, sample_shops, select_custom_monthly_shops,
};
use crate::core::db::DbContext;
use crate::core::error::AppError;
use crate::core::model::{
  DailyTask, MonthlyTask, MonthlyTaskPlanPreview, ShopRecord, TaskRunRequest,
};

pub(crate) const MIN_DAILY_TARGET: usize = 15;
pub(crate) const MAX_DAILY_TARGET: usize = 25;

#[derive(Debug, Clone)]
pub(crate) struct ReadingLinkData {
  pub s_course_id: String,
  pub s_manager_id: String,
  pub referer: String,
}

pub fn parse_reading_url(reading_url: &str) -> Result<ReadingLinkData, AppError> {
  let parsed_url = Url::parse(reading_url)
    .map_err(|error| AppError::ValidationError(format!("无法解析阅读链接: {error}")))?;
  let target_url = match parsed_url.query_pairs().find(|(k, _)| k == "redirect_uri") {
    Some((_, redirect_uri)) => Url::parse(&redirect_uri)
      .map_err(|error| AppError::ValidationError(format!("无法解析重定向阅读链接: {error}")))?,
    None => parsed_url,
  };

  let query_map = target_url
    .query_pairs()
    .collect::<std::collections::HashMap<_, _>>();
  let s_course_id = query_map
    .get("CourseID")
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty())
    .ok_or_else(|| AppError::ValidationError("阅读链接中缺少有效的 CourseID".to_string()))?;
  let s_manager_id = query_map
    .get("UID")
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty())
    .ok_or_else(|| AppError::ValidationError("阅读链接中缺少有效的 UID".to_string()))?;

  Ok(ReadingLinkData {
    s_course_id,
    s_manager_id,
    referer: target_url.to_string(),
  })
}

pub(crate) fn resolve_task_reading_link(task: &MonthlyTask) -> Result<ReadingLinkData, AppError> {
  if !task.reading_url.trim().is_empty() {
    let parsed = parse_reading_url(&task.reading_url)?;
    if parsed.s_course_id == task.s_course_id && parsed.s_manager_id == task.s_manager_id {
      return Ok(parsed);
    }
  }

  let referer = format!(
    "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID={}&UID={}",
    task.s_course_id, task.s_manager_id
  );

  Ok(ReadingLinkData {
    s_course_id: task.s_course_id.clone(),
    s_manager_id: task.s_manager_id.clone(),
    referer,
  })
}

pub(crate) fn resolve_request_reading_link(
  request: &TaskRunRequest,
) -> Result<ReadingLinkData, AppError> {
  if !request.reading_url.trim().is_empty() {
    return parse_reading_url(&request.reading_url);
  }

  if request.s_course_id.trim().is_empty() {
    return Err(AppError::ValidationError(
      "未提供 reading_url 时，必须提供 -c/--course-id".to_string(),
    ));
  }
  if request.s_manager_id.trim().is_empty() {
    return Err(AppError::ValidationError(
      "未提供 reading_url 时，必须提供 -u/--manager-id".to_string(),
    ));
  }

  let referer = format!(
    "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID={}&UID={}",
    request.s_course_id, request.s_manager_id
  );

  Ok(ReadingLinkData {
    s_course_id: request.s_course_id.clone(),
    s_manager_id: request.s_manager_id.clone(),
    referer,
  })
}

pub fn preview_monthly_task_plan(
  db: &DbContext,
  task: &MonthlyTask,
) -> Result<MonthlyTaskPlanPreview, AppError> {
  parse_reading_url(&task.reading_url)?;
  let shops = crate::core::db::get_shops_by_fc_and_type(db, &task.fc_name, &task.task_type)?;
  build_monthly_task_plan(task, shops)
}

pub fn create_monthly_task_with_plan(
  db: &DbContext,
  task: &MonthlyTask,
) -> Result<MonthlyTaskPlanPreview, AppError> {
  let reading_link = parse_reading_url(&task.reading_url)?;
  let plan = preview_monthly_task_plan(db, task)?;
  let planned_task = MonthlyTask {
    id: task.id.clone(),
    fc_name: task.fc_name.clone(),
    s_manager_id: reading_link.s_manager_id,
    s_course_id: reading_link.s_course_id,
    reading_url: task.reading_url.clone(),
    task_type: task.task_type.clone(),
    total_target: plan.total_target,
    target_days: 0,
    created_at: task.created_at.clone(),
    shopcodes: plan.shopcodes.clone(),
    excluded_open_ids: task.excluded_open_ids.clone(),
  };

  crate::core::db::add_monthly_task(db, &planned_task)?;
  Ok(plan)
}

pub(crate) fn build_monthly_task_plan(
  task: &MonthlyTask,
  shops: Vec<ShopRecord>,
) -> Result<MonthlyTaskPlanPreview, AppError> {
  let eligible_shops = filter_task_shops(shops, &task.fc_name, &task.task_type);
  let eligible_shop_count = eligible_shops.len();
  if eligible_shops.is_empty() {
    return Err(AppError::ResourceUnavailableError(format!(
      "未找到 FC={} 且任务类型={} 的可用门店",
      task.fc_name, task.task_type
    )));
  }

  let custom_shopcodes = normalize_shopcodes(&task.shopcodes);
  let total_target = if custom_shopcodes.is_empty() {
    calculate_monthly_target(eligible_shop_count, &task.task_type)
  } else {
    custom_shopcodes.len()
  };
  if total_target == 0 {
    return Err(AppError::ValidationError(format!(
      "FC={} 在任务类型={} 下没有可执行的月度目标",
      task.fc_name, task.task_type
    )));
  }

  let selected_shops = if custom_shopcodes.is_empty() {
    sample_shops(eligible_shops, total_target)
  } else {
    select_custom_monthly_shops(eligible_shops, &custom_shopcodes)?
  };

  let selected_shopcodes = selected_shops
    .into_iter()
    .map(|shop| shop.shop_code)
    .collect::<Vec<_>>();

  Ok(MonthlyTaskPlanPreview {
    eligible_shop_count,
    total_target,
    target_days: 0,
    shopcodes: selected_shopcodes,
    daily_plans: Vec::new(),
  })
}

pub(crate) fn calculate_monthly_target(eligible_shop_count: usize, task_type: &str) -> usize {
  let (min, max) = calculate_monthly_target_bounds(eligible_shop_count, task_type);
  if min == max {
    return min;
  }
  rand::rng().random_range(min..=max)
}

pub(crate) fn calculate_monthly_target_bounds(
  eligible_shop_count: usize,
  task_type: &str,
) -> (usize, usize) {
  if eligible_shop_count == 0 {
    return (0, 0);
  }

  match task_type {
    "Avene" => {
      let min = (eligible_shop_count * 70).div_ceil(100);
      let max = (eligible_shop_count * 75) / 100;
      if min > max { (max, max) } else { (min, max) }
    }
    "Klorane" => {
      let min = (eligible_shop_count * 85).div_ceil(100);
      let max = (eligible_shop_count * 95) / 100;
      if min > max { (max, max) } else { (min, max) }
    }
    _ => (eligible_shop_count, eligible_shop_count),
  }
}

pub(crate) fn is_daily_task_completed(task: &DailyTask) -> bool {
  task.completed_count >= task.target_count
}

pub(crate) fn random_daily_target(min: usize, max: usize) -> usize {
  if min >= max {
    return min;
  }
  rand::rng().random_range(min..=max)
}

pub(crate) fn calculate_sleep_secs(total_to_run: usize) -> u64 {
  calculate_sleep_secs_at(Local::now(), total_to_run)
}

pub(crate) fn calculate_sleep_secs_at(now: DateTime<Local>, total_to_run: usize) -> u64 {
  if total_to_run == 0 {
    return 0;
  }

  let Some(deadline) = now.with_hour(21).and_then(|time| {
    time
      .with_minute(0)
      .and_then(|time| time.with_second(0))
      .and_then(|time| time.with_nanosecond(0))
  }) else {
    return 0;
  };

  if now >= deadline {
    return 0;
  }

  let remaining_secs = (deadline - now).num_seconds().max(0) as u64;
  remaining_secs / (total_to_run as u64)
}
