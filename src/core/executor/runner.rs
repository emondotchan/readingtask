use std::collections::HashSet;
use std::time::Duration;

use chrono::{Local, Utc};

use super::archive::archive_quick_run_results;
use super::client::{SubmitReadLogBody, execute_single_request, get_http_client};
use super::planner::{
  MAX_DAILY_TARGET, MIN_DAILY_TARGET, calculate_sleep_secs, is_daily_task_completed,
  random_daily_target, resolve_request_reading_link, resolve_task_reading_link,
};
use super::selector::{
  normalize_open_ids, sample_shops, select_open_ids, select_planned_shops, select_shops,
};
use crate::core::db::DbContext;
use crate::core::error::AppError;
use crate::core::model::{
  DailyTask, MonthlyTask, ShopRecord, TaskItemOutcome, TaskItemResult, TaskProgress,
  TaskRunRequest, TaskRunSummary, civil_from_days,
};

#[derive(Debug)]
pub(crate) struct PreparedRun {
  pub requested_count: usize,
  pub selected_open_ids: Vec<String>,
  pub selected_shops: Vec<ShopRecord>,
}

pub(crate) fn prepare_run(
  db: &DbContext,
  request: &TaskRunRequest,
  run_date: Option<&str>,
) -> Result<PreparedRun, AppError> {
  validate_request(request)?;
  let selected_shops = select_shops(db, request)?;
  let requested_count = if request.shopcodes.is_empty() {
    request.count
  } else {
    selected_shops.len()
  };
  let run_date = run_date
    .map(ToOwned::to_owned)
    .unwrap_or_else(current_date_string);
  let month_prefix = task_month_prefix_from_date(&run_date)?;
  // Attempt to infer task_type from an existing monthly task for the same month/fc/course.
  let matched_tasks = crate::core::db::find_monthly_tasks_by_month_fc_course(
    db,
    &month_prefix,
    &request.fc,
    &request.s_course_id,
  )?;
  let task_type_opt: Option<&str> = if matched_tasks.len() == 1 {
    Some(matched_tasks[0].task_type.as_str())
  } else {
    None
  };
  let excluded_open_ids = if matched_tasks.len() == 1 {
    normalize_open_ids(&matched_tasks[0].excluded_open_ids)
      .into_iter()
      .collect::<HashSet<_>>()
  } else {
    HashSet::new()
  };

  let used_open_ids =
    crate::core::db::get_used_open_ids_for_month(db, &month_prefix, task_type_opt)?;
  let open_ids = crate::core::db::get_open_ids_by_fc(db, &request.fc)?;
  let selected_open_ids = select_open_ids(
    open_ids,
    &request.fc,
    &used_open_ids,
    &excluded_open_ids,
    requested_count,
  )?;

  Ok(PreparedRun {
    requested_count,
    selected_open_ids,
    selected_shops,
  })
}

pub(crate) fn validate_request(request: &TaskRunRequest) -> Result<(), AppError> {
  if request.count == 0 {
    return Err(AppError::ValidationError(
      "-n/--count 必须大于 0".to_string(),
    ));
  }
  Ok(())
}

pub async fn run_task(
  db: &DbContext,
  request: TaskRunRequest,
  run_date: Option<&str>,
) -> Result<TaskRunSummary, AppError> {
  run_task_with_progress(db, request, |_| {}, run_date).await
}

pub async fn run_task_with_progress<F>(
  db: &DbContext,
  request: TaskRunRequest,
  mut on_progress: F,
  run_date: Option<&str>,
) -> Result<TaskRunSummary, AppError>
where
  F: FnMut(TaskProgress) + Send,
{
  let reading_link = resolve_request_reading_link(&request)?;
  let prepared = prepare_run(db, &request, run_date)?;
  let client = get_http_client()?;
  let started_at = now_timestamp_string();

  let mut items = Vec::new();
  for (index, (shop, open_id)) in prepared
    .selected_shops
    .iter()
    .zip(prepared.selected_open_ids.iter())
    .enumerate()
  {
    let body = SubmitReadLogBody {
      s_course_id: &reading_link.s_course_id,
      s_manager_id: &reading_link.s_manager_id,
      open_id,
      province: &shop.province,
      city: &shop.city,
      shop_code: &shop.shop_code,
    };

    let item = execute_single_request(
      client,
      &body,
      reading_link.referer.clone(),
      index,
      open_id,
      shop,
    )
    .await;

    items.push(item.clone());
    on_progress(TaskProgress {
      task_id: None,
      processed_count: items.len(),
      requested_count: prepared.requested_count,
      latest_item: item,
    });
  }

  let success_count = items
    .iter()
    .filter(|item| item.outcome == TaskItemOutcome::Success)
    .count();
  let processed_count = items.len();
  let failure_count = processed_count.saturating_sub(success_count);
  let archive_result = archive_quick_run_results(db, &request, &items, run_date)?;

  Ok(TaskRunSummary {
    requested_count: prepared.requested_count,
    processed_count,
    success_count,
    failure_count,
    started_at,
    finished_at: now_timestamp_string(),
    items,
    archive_result: Some(archive_result),
  })
}

pub async fn run_daily_task_with_progress<F>(
  db: &DbContext,
  task_id: &str,
  date: &str,
  on_progress: F,
) -> Result<TaskRunSummary, AppError>
where
  F: FnMut(TaskProgress) + Send,
{
  run_daily_task_with_progress_controlled(db, task_id, date, on_progress, || false).await
}

pub async fn run_daily_task_with_progress_controlled<F, P>(
  db: &DbContext,
  task_id: &str,
  date: &str,
  mut on_progress: F,
  should_pause: P,
) -> Result<TaskRunSummary, AppError>
where
  F: FnMut(TaskProgress) + Send,
  P: Fn() -> bool + Send,
{
  let tasks = crate::core::db::get_all_monthly_tasks(db)?;
  let task = tasks
    .into_iter()
    .find(|t| t.id == task_id)
    .ok_or_else(|| AppError::ResourceUnavailableError(format!("未找到月度任务: {}", task_id)))?;

  let valid_shops = crate::core::db::get_shops_by_fc_and_type(db, &task.fc_name, &task.task_type)?;
  if valid_shops.is_empty() {
    log::error!(
      "执行失败: 未找到 FC={} task_type={} 对应的门店",
      task.fc_name,
      task.task_type
    );
    return Err(AppError::ResourceUnavailableError(format!(
      "未找到 FC={} 对应的门店",
      task.fc_name
    )));
  }

  let target_shops = if !task.shopcodes.is_empty() {
    select_planned_shops(valid_shops.clone(), &task.shopcodes)?
  } else {
    valid_shops.clone()
  };

  let client = get_http_client()?;
  let started_at = now_timestamp_string();

  let used_shop_codes = crate::core::db::get_task_result_shop_codes(db, task_id)?;
  let unused_target_shops = target_shops
    .iter()
    .filter(|shop| !used_shop_codes.contains(shop.shop_code.as_str()))
    .cloned()
    .collect::<Vec<_>>();

  let existing_daily_task = crate::core::db::get_daily_task(db, task_id, date)?;
  if existing_daily_task.is_none() && unused_target_shops.is_empty() {
    log::warn!(
      "任务 {} ({}) 已全部完成 (当月目标门店已全部执行)",
      task.id,
      task.fc_name
    );
    return Err(AppError::ExecutionError("该月度任务已全部完成".to_string()));
  }

  let mut today_progress = match existing_daily_task {
    Some(progress) => progress,
    None => ensure_daily_task(db, &task, date)?,
  };

  if is_daily_task_completed(&today_progress) {
    today_progress.is_locked = true;
    today_progress.run_status = "completed".to_string();
    crate::core::db::save_daily_task(db, &today_progress)?;
    return Err(AppError::ExecutionError(
      "今日任务已经完成，请明天再执行".to_string(),
    ));
  }

  if today_progress.is_locked {
    today_progress.is_locked = false;
    today_progress.run_status = "not_started".to_string();
    crate::core::db::save_daily_task(db, &today_progress)?;
  }

  today_progress.run_status = "running".to_string();
  crate::core::db::save_daily_task(db, &today_progress)?;

  let requested_count = today_progress
    .target_count
    .saturating_sub(today_progress.completed_count);
  let planned_today_shops = select_planned_shops(valid_shops.clone(), &today_progress.shopcodes)?;
  let mut selected_shops = planned_today_shops
    .into_iter()
    .filter(|shop| !used_shop_codes.contains(shop.shop_code.as_str()))
    .take(requested_count)
    .collect::<Vec<_>>();

  if selected_shops.len() < requested_count && !unused_target_shops.is_empty() {
    let already_selected: HashSet<String> =
      selected_shops.iter().map(|s| s.shop_code.clone()).collect();
    let missing_count = requested_count - selected_shops.len();
    let additional_shops = unused_target_shops
      .into_iter()
      .filter(|s| !already_selected.contains(&s.shop_code))
      .take(missing_count)
      .collect::<Vec<_>>();
    selected_shops.extend(additional_shops);
  }

  let requested_count = selected_shops.len();
  if requested_count == 0 {
    log::info!("今日任务 {} 没有剩余可执行门店，跳过请求发送", task_id);
    if is_daily_task_completed(&today_progress) {
      today_progress.is_locked = true;
      today_progress.run_status = "completed".to_string();
    } else {
      today_progress.is_locked = false;
      today_progress.run_status = "paused".to_string();
    }
    crate::core::db::save_daily_task(db, &today_progress)?;
    return Ok(TaskRunSummary {
      requested_count: 0,
      processed_count: 0,
      success_count: 0,
      failure_count: 0,
      started_at,
      finished_at: now_timestamp_string(),
      items: Vec::new(),
      archive_result: None,
    });
  }

  let month_prefix = task_month_prefix_from_date(date)?;
  let used_open_ids =
    crate::core::db::get_used_open_ids_for_month(db, &month_prefix, Some(&task.task_type))?;
  let excluded_open_ids = normalize_open_ids(&task.excluded_open_ids)
    .into_iter()
    .collect::<HashSet<_>>();
  let open_ids = crate::core::db::get_open_ids_by_fc(db, &task.fc_name)?;
  let selected_open_ids = select_open_ids(
    open_ids,
    &task.fc_name,
    &used_open_ids,
    &excluded_open_ids,
    requested_count,
  )?;

  let mut items = Vec::new();
  let reading_link = resolve_task_reading_link(&task)?;

  for (index, (shop, open_id)) in selected_shops
    .iter()
    .zip(selected_open_ids.iter())
    .enumerate()
  {
    if let Err(error) = ensure_not_paused(&should_pause) {
      today_progress.run_status = "paused".to_string();
      crate::core::db::save_daily_task(db, &today_progress)?;
      return Err(error);
    }

    let body = SubmitReadLogBody {
      s_course_id: &reading_link.s_course_id,
      s_manager_id: &reading_link.s_manager_id,
      open_id,
      province: &shop.province,
      city: &shop.city,
      shop_code: &shop.shop_code,
    };

    let mut item = execute_single_request(
      client,
      &body,
      reading_link.referer.clone(),
      index,
      open_id,
      shop,
    )
    .await;

    // Persist result to DB for history viewing
    let result_id = crate::core::db::save_task_result(db, task_id, &item)?;
    item.result_id = Some(result_id);

    // Update progress only on success
    if item.outcome == TaskItemOutcome::Success {
      today_progress.completed_count += 1;
      crate::core::db::save_daily_task(db, &today_progress)?;
    }

    items.push(item.clone());
    on_progress(TaskProgress {
      task_id: Some(task_id.to_string()),
      processed_count: items.len(),
      requested_count,
      latest_item: item,
    });

    if index + 1 < requested_count {
      let remaining_items = requested_count - (index + 1);
      let sleep_secs = calculate_sleep_secs(remaining_items);
      log::info!(
        "单次请求发送完成 (剩余 {} 项)，根据当日 21:00 截止时间平摊，休眠 {} 秒后继续...",
        remaining_items,
        sleep_secs
      );
      if let Err(error) = sleep_with_pause_check(sleep_secs, &should_pause).await {
        today_progress.run_status = "paused".to_string();
        crate::core::db::save_daily_task(db, &today_progress)?;
        return Err(error);
      }
    }
  }

  let success_count = items
    .iter()
    .filter(|item| item.outcome == TaskItemOutcome::Success)
    .count();
  let processed_count = items.len();
  let failure_count = processed_count.saturating_sub(success_count);

  if is_daily_task_completed(&today_progress) {
    today_progress.is_locked = true;
    today_progress.run_status = "completed".to_string();
    log::info!(
      "今日任务全部完成: 成功 {} 项, 失败 {} 项, 锁定任务",
      success_count,
      failure_count
    );
  } else {
    today_progress.is_locked = false;
    today_progress.run_status = "paused".to_string();
    log::info!(
      "今日任务未完全完成: 成功 {}/{} 项, 设置为暂停状态以便后续继续",
      today_progress.completed_count,
      today_progress.target_count
    );
  }
  crate::core::db::save_daily_task(db, &today_progress)?;

  Ok(TaskRunSummary {
    requested_count,
    processed_count,
    success_count,
    failure_count,
    started_at,
    finished_at: now_timestamp_string(),
    items,
    archive_result: None,
  })
}

pub async fn retry_task_result(
  db: &DbContext,
  task_id: &str,
  result_id: i64,
) -> Result<TaskItemResult, AppError> {
  let tasks = crate::core::db::get_all_monthly_tasks(db)?;
  let task = tasks
    .into_iter()
    .find(|t| t.id == task_id)
    .ok_or_else(|| AppError::ResourceUnavailableError(format!("未找到月度任务: {}", task_id)))?;

  let item = crate::core::db::get_task_result(db, task_id, result_id)?.ok_or_else(|| {
    AppError::ResourceUnavailableError(format!("未找到执行结果记录: {}", result_id))
  })?;

  validate_retry_task_result(&item)?;
  let reading_link = resolve_task_reading_link(&task)?;
  let client = get_http_client()?;

  let body = SubmitReadLogBody {
    s_course_id: &reading_link.s_course_id,
    s_manager_id: &reading_link.s_manager_id,
    open_id: &item.open_id,
    province: &item.province,
    city: &item.city,
    shop_code: &item.shop_code,
  };

  let shop = ShopRecord {
    province: item.province.clone(),
    city: item.city.clone(),
    shop_code: item.shop_code.clone(),
    shop_name: String::new(),
    fc: Some(task.fc_name.clone()),
    shop_type: 0,
  };

  let mut retried = execute_single_request(
    client,
    &body,
    reading_link.referer.clone(),
    item.index.saturating_sub(1),
    &item.open_id,
    &shop,
  )
  .await;

  retried.result_id = Some(result_id);
  retried.index = item.index;
  let saved_id = crate::core::db::save_retried_task_result(db, task_id, result_id, &retried)?;
  retried.result_id = Some(saved_id);

  if item.outcome != TaskItemOutcome::Success && retried.outcome == TaskItemOutcome::Success {
    let execution_date = item
      .executed_date
      .as_deref()
      .and_then(|dt| dt.get(..10))
      .unwrap_or_default();

    if let Some(mut daily_task) = crate::core::db::get_daily_task(db, task_id, execution_date)? {
      daily_task.completed_count += 1;
      if is_daily_task_completed(&daily_task) {
        daily_task.is_locked = true;
        daily_task.run_status = "completed".to_string();
      }
      crate::core::db::save_daily_task(db, &daily_task)?;
    }
  }

  Ok(retried)
}

pub(crate) fn validate_retry_task_result(item: &TaskItemResult) -> Result<(), AppError> {
  if item.outcome == TaskItemOutcome::Success {
    return Err(AppError::ValidationError(
      "该记录已执行成功，无需重新提交".to_string(),
    ));
  }

  if item.open_id.trim().is_empty() {
    return Err(AppError::ValidationError(
      "记录中缺少有效的 OpenID，无法重试".to_string(),
    ));
  }

  if item.shop_code.trim().is_empty() {
    return Err(AppError::ValidationError(
      "记录中缺少有效的 ShopCode，无法重试".to_string(),
    ));
  }

  Ok(())
}

pub(crate) fn ensure_daily_task(
  db: &DbContext,
  task: &MonthlyTask,
  date: &str,
) -> Result<DailyTask, AppError> {
  if let Some(existing) = crate::core::db::get_daily_task(db, &task.id, date)? {
    return Ok(existing);
  }

  let valid_shops = crate::core::db::get_shops_by_fc_and_type(db, &task.fc_name, &task.task_type)?;
  let target_shops = if !task.shopcodes.is_empty() {
    select_planned_shops(valid_shops, &task.shopcodes)?
  } else {
    valid_shops
  };

  let used_shop_codes = crate::core::db::get_task_result_shop_codes(db, &task.id)?;
  let unused_target_shops: Vec<ShopRecord> = target_shops
    .into_iter()
    .filter(|shop| !used_shop_codes.contains(shop.shop_code.as_str()))
    .collect();

  if unused_target_shops.is_empty() {
    return Err(AppError::ExecutionError("该月度任务已全部完成".to_string()));
  }

  let allocated_count = if unused_target_shops.len() <= MIN_DAILY_TARGET {
    unused_target_shops.len()
  } else {
    let max = unused_target_shops.len().min(MAX_DAILY_TARGET);
    random_daily_target(MIN_DAILY_TARGET, max)
  };

  let selected_today_shops = sample_shops(unused_target_shops, allocated_count);
  let selected_shopcodes = selected_today_shops
    .into_iter()
    .map(|s| s.shop_code)
    .collect::<Vec<_>>();

  let daily_task = DailyTask {
    task_id: task.id.clone(),
    date: date.to_string(),
    target_count: selected_shopcodes.len(),
    completed_count: 0,
    is_locked: false,
    shopcodes: selected_shopcodes,
    run_status: "not_started".to_string(),
  };

  crate::core::db::save_daily_task(db, &daily_task)?;
  Ok(daily_task)
}

pub(crate) fn ensure_not_paused<P>(should_pause: &P) -> Result<(), AppError>
where
  P: Fn() -> bool,
{
  if should_pause() {
    Err(AppError::Paused("任务已被用户暂停".to_string()))
  } else {
    Ok(())
  }
}

pub(crate) async fn sleep_with_pause_check<P>(
  sleep_secs: u64,
  should_pause: &P,
) -> Result<(), AppError>
where
  P: Fn() -> bool,
{
  let interval = Duration::from_millis(200);
  let total = Duration::from_secs(sleep_secs);
  let mut elapsed = Duration::ZERO;

  while elapsed < total {
    ensure_not_paused(should_pause)?;
    let step = (total - elapsed).min(interval);
    tokio::time::sleep(step).await;
    elapsed += step;
  }

  ensure_not_paused(should_pause)
}

pub(crate) fn current_date_string() -> String {
  let today_civil = civil_from_days((Utc::now().timestamp_micros() / 1_000_000 + 8 * 3600) / 86400);
  format!(
    "{:04}-{:02}-{:02}",
    today_civil.0, today_civil.1, today_civil.2
  )
}

pub(crate) fn now_timestamp_string() -> String {
  format!("{}", Utc::now().timestamp_micros())
}

pub(crate) fn current_datetime_string() -> String {
  Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub(crate) fn task_month_prefix_from_date(date: &str) -> Result<String, AppError> {
  let year = date
    .get(2..4)
    .ok_or_else(|| AppError::ValidationError(format!("无效日期格式: {date}")))?;
  let month = date
    .get(5..7)
    .ok_or_else(|| AppError::ValidationError(format!("无效日期格式: {date}")))?;
  Ok(format!("{year}{month}"))
}
