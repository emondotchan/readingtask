use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Local, Timelike};
use rand::RngExt;
use rand::seq::SliceRandom;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};

use super::db::DbContext;
use super::error::AppError;
use super::loader::load_runtime_data;
use super::model::{
  DailyTask, MonthlyTask, MonthlyTaskPlanPreview, OpenIdRecord, QuickRunArchiveResult,
  QuickRunArchiveStatus, SHOP_TYPE_AVENE, SHOP_TYPE_AVENE_KLORANE, SHOP_TYPE_KLORANE, ShopRecord,
  TaskItemOutcome, TaskItemResult, TaskProgress, TaskRunRequest, TaskRunSummary, add_days_to_date,
  civil_from_days,
};

const SUBMIT_READ_LOG_URL: &str =
  "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx/SubmitReadLog";
const MIN_DAILY_TARGET: usize = 15;
const MAX_DAILY_TARGET: usize = 25;
const GENERATED_OPEN_ID_PREFIX: &str = "o-kP6s";
const GENERATED_OPEN_ID_LEN: usize = 28;
const AVENE_MONTHLY_COMPLETION_PERCENT: usize = 65;
const KLORANE_MONTHLY_COMPLETION_PERCENT: usize = 85;

#[derive(Debug, Serialize)]
struct SubmitReadLogBody<'a> {
  #[serde(rename = "sCourseID")]
  s_course_id: &'a str,
  #[serde(rename = "sManagerID")]
  s_manager_id: &'a str,
  #[serde(rename = "OpenID")]
  open_id: &'a str,
  #[serde(rename = "Province")]
  province: &'a str,
  #[serde(rename = "City")]
  city: &'a str,
  #[serde(rename = "ShopCode")]
  shop_code: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SubmitReadLogResponseEnvelope {
  Wrapped { d: String },
  Direct(SubmitReadLogPayload),
}

#[derive(Debug, Deserialize)]
struct SubmitReadLogPayload {
  err: i32,
  #[serde(rename = "RtnMsg")]
  rtn_msg: String,
  #[serde(rename = "ReadID")]
  read_id: Option<String>,
}

#[derive(Debug)]
struct PreparedRun {
  requested_count: usize,
  selected_open_ids: Vec<String>,
  selected_shops: Vec<ShopRecord>,
}

struct ReadingLinkData {
  s_course_id: String,
  s_manager_id: String,
  referer: String,
}

pub fn preview_monthly_task_plan(
  db: &DbContext,
  task: &MonthlyTask,
) -> Result<MonthlyTaskPlanPreview, AppError> {
  parse_reading_url(&task.reading_url)?;
  let runtime_data = load_runtime_data(db)?;
  build_monthly_task_plan(task, runtime_data.shops)
}

pub fn create_monthly_task_with_plan(
  db: &DbContext,
  task: &MonthlyTask,
) -> Result<MonthlyTaskPlanPreview, AppError> {
  let reading_link = parse_reading_url(&task.reading_url)?;
  let plan = preview_monthly_task_plan(db, task)?;
  let planned_task = MonthlyTask {
    s_course_id: reading_link.s_course_id,
    s_manager_id: reading_link.s_manager_id,
    reading_url: task.reading_url.trim().to_string(),
    total_target: plan.total_target,
    target_days: plan.target_days,
    shopcodes: plan
      .daily_plans
      .iter()
      .flat_map(|daily_plan| daily_plan.shopcodes.iter().cloned())
      .collect(),
    excluded_open_ids: normalize_open_ids(&task.excluded_open_ids),
    ..task.clone()
  };

  super::db::add_monthly_task(db, &planned_task)?;
  for daily_plan in &plan.daily_plans {
    super::db::save_daily_task(db, daily_plan)?;
  }

  Ok(plan)
}

pub async fn run_task(db: &DbContext, request: TaskRunRequest) -> Result<TaskRunSummary, AppError> {
  run_task_with_progress(db, request, |_| {}, None).await
}

pub async fn run_daily_task_with_progress<F>(
  db: &DbContext,
  task_id: &str,
  date: &str,
  on_progress: F,
) -> Result<TaskRunSummary, AppError>
where
  F: FnMut(TaskProgress),
{
  run_daily_task_with_progress_controlled(db, task_id, date, on_progress, || false).await
}

pub async fn run_daily_task_with_progress_controlled<F, C>(
  db: &DbContext,
  task_id: &str,
  date: &str,
  mut on_progress: F,
  should_pause: C,
) -> Result<TaskRunSummary, AppError>
where
  F: FnMut(TaskProgress),
  C: Fn() -> bool,
{
  let tasks = super::db::get_all_monthly_tasks(db)?;
  let task = tasks
    .into_iter()
    .find(|t| t.id == task_id)
    .ok_or_else(|| AppError::ResourceUnavailableError(format!("未找到月度任务: {}", task_id)))?;

  let runtime_data = load_runtime_data(db)?;
  let monthly_completion_target = calculate_monthly_completion_target_for_task(
    &runtime_data.shops,
    &task.fc_name,
    &task.task_type,
  );
  let client = build_http_client()?;
  let started_at = now_timestamp_string();

  // Get all progress to compute today's target
  let all_progress = super::db::get_all_daily_tasks_for_task(db, task_id)?;
  let total_completed: usize = all_progress.iter().map(|p| p.completed_count).sum();
  let completed_days = all_progress
    .iter()
    .filter(|progress| is_daily_task_completed(progress))
    .count();
  let existing_daily_task = all_progress
    .iter()
    .find(|progress| progress.date == date)
    .cloned();
  let has_incomplete_daily_task = existing_daily_task
    .as_ref()
    .is_some_and(|progress| !is_daily_task_completed(progress));

  if monthly_completion_target > 0
    && total_completed >= monthly_completion_target
    && !has_incomplete_daily_task
  {
    log::warn!(
      "任务 {} ({}) 已全部完成 (total_completed: {}, completion_target: {})",
      task.id,
      task.fc_name,
      total_completed,
      monthly_completion_target
    );
    return Err(AppError::ExecutionError("该月度任务已全部完成".to_string()));
  }

  if monthly_completion_target > 0
    && total_completed >= monthly_completion_target
    && has_incomplete_daily_task
  {
    log::info!(
      "任务 {} ({}) 已达到月度完成门槛，但每日任务 {} 尚未完成，继续补做",
      task.id,
      task.fc_name,
      date
    );
  }

  log::info!(
    "任务 {} 进度: 已完成天数 {}/{}, 总成功数 {}/{}, 准备初始化今日进度...",
    task.id,
    completed_days,
    task.target_days,
    total_completed,
    monthly_completion_target
  );

  let had_existing_daily_task = existing_daily_task.is_some();
  let mut today_progress = match existing_daily_task {
    Some(progress) => progress,
    None => ensure_daily_task(db, &task, date)?,
  };
  if !had_existing_daily_task && monthly_completion_target > 0 {
    let monthly_remaining_count = monthly_completion_target.saturating_sub(total_completed);
    let daily_remaining_count = today_progress
      .target_count
      .saturating_sub(today_progress.completed_count);
    if monthly_remaining_count < daily_remaining_count {
      today_progress.target_count = today_progress.completed_count + monthly_remaining_count;
    }
  }

  super::db::save_daily_task(db, &today_progress)?;

  if is_daily_task_completed(&today_progress) {
    today_progress.is_locked = true;
    today_progress.run_status = "completed".to_string();
    super::db::save_daily_task(db, &today_progress)?;
    return Err(AppError::ExecutionError(
      "今日任务已经完成，请明天再执行".to_string(),
    ));
  }

  if today_progress.is_locked {
    today_progress.is_locked = false;
    today_progress.run_status = "not_started".to_string();
    super::db::save_daily_task(db, &today_progress)?;
  }

  today_progress.run_status = "running".to_string();
  super::db::save_daily_task(db, &today_progress)?;

  let valid_shops = filter_task_shops(runtime_data.shops, &task.fc_name, &task.task_type);
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

  log::info!(
    "符合条件 (FC: {}, Type: {}) 的门店总计: {}",
    task.fc_name,
    task.task_type,
    valid_shops.len()
  );

  let requested_count = today_progress.target_count - today_progress.completed_count;
  let selected_shops = if today_progress.shopcodes.is_empty() {
    let used_shop_codes = super::db::get_task_result_shop_codes(db, task_id)?;
    let random_shops = select_unused_monthly_shops(
      valid_shops,
      &used_shop_codes,
      requested_count,
      task.shopcodes.is_empty(),
    )?;
    log::info!(
      "未指定今日门店，从未使用门店中随机挑选 {} 家",
      random_shops.len()
    );
    random_shops
  } else {
    let task_used_shop_codes = super::db::get_task_result_shop_codes(db, task_id)?;
    let planned_shops = select_planned_shops(valid_shops, &today_progress.shopcodes)?;
    if planned_shops.is_empty() && !today_progress.shopcodes.is_empty() {
      return Err(AppError::ResourceUnavailableError(
        "今日计划的所有门店均不存在或已被删除".to_string(),
      ));
    }
    let planned_shops = planned_shops
      .into_iter()
      .filter(|shop| !task_used_shop_codes.contains(shop.shop_code.as_str()))
      .take(requested_count)
      .collect::<Vec<_>>();
    if !task_used_shop_codes.is_empty() {
      log::info!(
        "任务 {} 已有 {} 家门店发送过阅读请求，本次跳过已执行门店",
        task_id,
        task_used_shop_codes.len()
      );
    }
    if planned_shops.len() < today_progress.shopcodes.len() {
      log::warn!(
        "已指定今日门店 {} 家，本次剩余 {} 家可执行门店 (部分可能已请求/被删除/不匹配)",
        today_progress.shopcodes.len(),
        planned_shops.len()
      );
    } else {
      log::info!("已指定今日门店，筛选出 {} 家", planned_shops.len());
    }

    planned_shops
  };
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
    super::db::save_daily_task(db, &today_progress)?;
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
    super::db::get_used_open_ids_for_month(db, &month_prefix, Some(&task.task_type))?;
  let excluded_open_ids = normalize_open_ids(&task.excluded_open_ids)
    .into_iter()
    .collect::<HashSet<_>>();
  let selected_open_ids = select_open_ids(
    runtime_data.open_ids,
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
      super::db::save_daily_task(db, &today_progress)?;
      return Err(error);
    }

    // Keep requests moving while avoiding back-to-back submissions.
    // Execute the first request immediately, and space out subsequent ones.
    if index > 0 {
      let sleep_secs = calculate_sleep_secs(selected_shops.len() - index);
      if sleep_secs > 0 {
        log::info!("任务 {}: 等待 {} 秒后执行下一个请求", task.id, sleep_secs);
        if let Err(error) = sleep_with_pause_check(sleep_secs, &should_pause).await {
          today_progress.run_status = "paused".to_string();
          super::db::save_daily_task(db, &today_progress)?;
          return Err(error);
        }
      }
    }

    if let Err(error) = ensure_not_paused(&should_pause) {
      today_progress.run_status = "paused".to_string();
      super::db::save_daily_task(db, &today_progress)?;
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
      &client,
      &body,
      reading_link.referer.clone(),
      index,
      open_id,
      shop,
    )
    .await;

    // Persist result to DB for history viewing
    match super::db::save_task_result(db, task_id, &item) {
      Ok(result_id) => item.result_id = Some(result_id),
      Err(error) => {
        log::error!(
          "保存执行记录失败: task_id={}, shop_code={}, error={}",
          task_id,
          item.shop_code,
          error
        );
      }
    }

    today_progress.completed_count += 1;
    if let Err(error) = super::db::save_daily_task(db, &today_progress) {
      log::error!(
        "更新每日任务进度失败: task_id={}, date={}, error={}",
        task_id,
        date,
        error
      );
    }

    if item.outcome == TaskItemOutcome::Success {
      log::info!(
        "请求 {}/{}: 成功 ({})",
        index + 1,
        requested_count,
        shop.shop_code
      );
    } else {
      log::error!(
        "请求 {}/{}: 失败 ({}) - {:?}",
        index + 1,
        requested_count,
        shop.shop_code,
        item.rtn_msg
      );
    }

    items.push(item.clone());
    on_progress(TaskProgress {
      task_id: Some(task_id.to_string()),
      processed_count: items.len(),
      requested_count,
      latest_item: item,
    });
  }

  let success_count = items
    .iter()
    .filter(|item| item.outcome == TaskItemOutcome::Success)
    .count();
  let processed_count = items.len();
  let failure_count = processed_count.saturating_sub(success_count);

  log::info!(
    "今日任务 {} ({}) 执行完成: 总计: {}, 成功: {}, 失败: {}",
    task_id,
    date,
    processed_count,
    success_count,
    failure_count
  );

  if is_daily_task_completed(&today_progress) {
    today_progress.is_locked = true;
    today_progress.run_status = "completed".to_string();
  } else {
    today_progress.is_locked = false;
    today_progress.run_status = "paused".to_string();
  }
  super::db::save_daily_task(db, &today_progress)?;

  // 如果总完成数已经达到或超过月度完成门槛，将该任务下所有的每日任务状态标记为已完成并锁定
  let all_progress = super::db::get_all_daily_tasks_for_task(db, task_id)?;
  let total_completed: usize = all_progress.iter().map(|p| p.completed_count).sum();
  if monthly_completion_target > 0 && total_completed >= monthly_completion_target {
    for mut progress in all_progress {
      if is_daily_task_completed(&progress)
        && (!progress.is_locked || progress.run_status != "completed")
      {
        progress.is_locked = true;
        progress.run_status = "completed".to_string();
        if let Err(e) = super::db::save_daily_task(db, &progress) {
          log::error!(
            "同步更新每日任务完成状态失败: task_id={}, date={}, error={}",
            task_id,
            progress.date,
            e
          );
        }
      }
    }
  }

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

fn ensure_not_paused<C>(should_pause: &C) -> Result<(), AppError>
where
  C: Fn() -> bool,
{
  if should_pause() {
    Err(AppError::Paused("任务已暂停".to_string()))
  } else {
    Ok(())
  }
}

async fn sleep_with_pause_check<C>(seconds: u64, should_pause: &C) -> Result<(), AppError>
where
  C: Fn() -> bool,
{
  for _ in 0..seconds {
    ensure_not_paused(should_pause)?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
  }

  ensure_not_paused(should_pause)
}

async fn execute_single_request(
  client: &reqwest::Client,
  body: &SubmitReadLogBody<'_>,
  referer: String,
  index: usize,
  open_id: &str,
  shop: &ShopRecord,
) -> TaskItemResult {
  match client
    .post(SUBMIT_READ_LOG_URL)
    .header("Referer", &referer)
    .json(body)
    .send()
    .await
  {
    Ok(response) => {
      let status = response.status().as_u16();
      match response.text().await {
        Ok(text) => build_task_item_result(
          index,
          open_id,
          shop,
          Some(status),
          classify_submit_read_log_response(&text),
        ),
        Err(error) => TaskItemResult {
          result_id: None,
          index: index + 1,
          executed_date: Some(current_datetime_string()),
          submit_err: None,
          rtn_msg: None,
          read_id: None,
          open_id: open_id.to_string(),
          shop_code: shop.shop_code.clone(),
          province: shop.province.clone(),
          city: shop.city.clone(),
          http_status: Some(status),
          response_text: Some(format!("读取响应体失败: {error}")),
          outcome: TaskItemOutcome::ResponseReadError,
        },
      }
    }
    Err(error) => TaskItemResult {
      result_id: None,
      index: index + 1,
      executed_date: Some(current_datetime_string()),
      submit_err: None,
      rtn_msg: None,
      read_id: None,
      open_id: open_id.to_string(),
      shop_code: shop.shop_code.clone(),
      province: shop.province.clone(),
      city: shop.city.clone(),
      http_status: None,
      response_text: Some(format!("请求失败: {error}")),
      outcome: TaskItemOutcome::RequestError,
    },
  }
}

pub async fn retry_task_result(
  db: &DbContext,
  task_id: &str,
  result_id: i64,
) -> Result<TaskItemResult, AppError> {
  let failed_result = super::db::get_task_result(db, task_id, result_id)?
    .ok_or_else(|| AppError::ResourceUnavailableError(format!("未找到失败记录: {result_id}")))?;
  validate_retry_task_result(&failed_result)?;

  let task = super::db::get_all_monthly_tasks(db)?
    .into_iter()
    .find(|task| task.id == task_id)
    .ok_or_else(|| AppError::ResourceUnavailableError(format!("未找到月度任务: {task_id}")))?;
  let reading_link = resolve_task_reading_link(&task)?;
  let client = build_http_client()?;
  let shop = ShopRecord {
    province: failed_result.province.clone(),
    city: failed_result.city.clone(),
    shop_code: failed_result.shop_code.clone(),
    shop_name: String::new(),
    fc: Some(task.fc_name.clone()),
    shop_type: match task.task_type.as_str() {
      "Klorane" => SHOP_TYPE_KLORANE,
      _ => SHOP_TYPE_AVENE,
    },
  };
  let body = SubmitReadLogBody {
    s_course_id: &reading_link.s_course_id,
    s_manager_id: &reading_link.s_manager_id,
    open_id: &failed_result.open_id,
    province: &failed_result.province,
    city: &failed_result.city,
    shop_code: &failed_result.shop_code,
  };
  let mut retried = execute_single_request(
    &client,
    &body,
    reading_link.referer,
    failed_result.index.saturating_sub(1),
    &failed_result.open_id,
    &shop,
  )
  .await;

  retried.result_id = Some(super::db::save_retried_task_result(
    db, task_id, result_id, &retried,
  )?);
  Ok(retried)
}

fn validate_retry_task_result(result: &TaskItemResult) -> Result<(), AppError> {
  if result.outcome == TaskItemOutcome::Success || result.submit_err == Some(0) {
    return Err(AppError::ValidationError("成功记录无需重做".to_string()));
  }

  if result.open_id.trim().is_empty()
    || result.shop_code.trim().is_empty()
    || result.province.trim().is_empty()
    || result.city.trim().is_empty()
  {
    return Err(AppError::ValidationError(
      "失败记录缺少重做所需的请求参数".to_string(),
    ));
  }

  Ok(())
}

pub async fn run_task_with_progress<F>(
  db: &DbContext,
  request: TaskRunRequest,
  mut on_progress: F,
  run_date: Option<&str>,
) -> Result<TaskRunSummary, AppError>
where
  F: FnMut(TaskProgress),
{
  let prepared = prepare_run(db, &request, run_date)?;
  let started_at = now_timestamp_string();
  let client = build_http_client()?;
  let mut items = Vec::new();
  let reading_link = resolve_request_reading_link(&request)?;

  for (index, (open_id, shop)) in prepared
    .selected_open_ids
    .iter()
    .zip(prepared.selected_shops.iter())
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

    let item = match client
      .post(SUBMIT_READ_LOG_URL)
      .header("Referer", &reading_link.referer)
      .json(&body)
      .send()
      .await
    {
      Ok(response) => {
        let status = response.status().as_u16();
        match response.text().await {
          Ok(text) => build_task_item_result(
            index,
            open_id,
            shop,
            Some(status),
            classify_submit_read_log_response(&text),
          ),
          Err(error) => TaskItemResult {
            result_id: None,
            index: index + 1,
            executed_date: Some(current_datetime_string()),
            submit_err: None,
            rtn_msg: None,
            read_id: None,
            open_id: open_id.clone(),
            shop_code: shop.shop_code.clone(),
            province: shop.province.clone(),
            city: shop.city.clone(),
            http_status: Some(status),
            response_text: Some(format!("读取响应体失败: {error}")),
            outcome: TaskItemOutcome::ResponseReadError,
          },
        }
      }
      Err(error) => TaskItemResult {
        result_id: None,
        index: index + 1,
        executed_date: Some(current_datetime_string()),
        submit_err: None,
        rtn_msg: None,
        read_id: None,
        open_id: open_id.clone(),
        shop_code: shop.shop_code.clone(),
        province: shop.province.clone(),
        city: shop.city.clone(),
        http_status: None,
        response_text: Some(format!("请求失败: {error}")),
        outcome: TaskItemOutcome::RequestError,
      },
    };

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

fn archive_quick_run_results(
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

fn archive_quick_run_results_for_date(
  db: &DbContext,
  request: &TaskRunRequest,
  items: &[TaskItemResult],
  run_date: &str,
) -> Result<QuickRunArchiveResult, AppError> {
  let month_prefix = task_month_prefix_from_date(run_date)?;
  let reading_link = resolve_request_reading_link(request)?;
  let matched_tasks = super::db::find_monthly_tasks_by_month_fc_course(
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
  super::db::save_task_results(db, &task.id, items)?;
  super::db::reconcile_daily_task_progress_for_task(db, &task.id)?;

  Ok(QuickRunArchiveResult {
    status: QuickRunArchiveStatus::Archived,
    task_id: Some(task.id.clone()),
    message: format!("已追加到月度任务 {}", task.id),
  })
}

#[derive(Debug, Clone)]
struct ClassifiedSubmitReadLogResponse {
  response_text: String,
  submit_err: Option<i32>,
  rtn_msg: Option<String>,
  read_id: Option<String>,
  outcome: TaskItemOutcome,
}

fn build_task_item_result(
  index: usize,
  open_id: &str,
  shop: &ShopRecord,
  http_status: Option<u16>,
  classified: ClassifiedSubmitReadLogResponse,
) -> TaskItemResult {
  TaskItemResult {
    result_id: None,
    index: index + 1,
    executed_date: Some(current_datetime_string()),
    submit_err: classified.submit_err,
    rtn_msg: classified.rtn_msg,
    read_id: classified.read_id,
    open_id: open_id.to_string(),
    shop_code: shop.shop_code.clone(),
    province: shop.province.clone(),
    city: shop.city.clone(),
    http_status,
    response_text: Some(classified.response_text),
    outcome: classified.outcome,
  }
}

fn classify_submit_read_log_response(text: &str) -> ClassifiedSubmitReadLogResponse {
  if let Some(payload) = parse_submit_read_log_payload(text) {
    let outcome = if payload.err == 0 {
      TaskItemOutcome::Success
    } else {
      TaskItemOutcome::RequestError
    };

    return ClassifiedSubmitReadLogResponse {
      response_text: text.to_string(),
      submit_err: Some(payload.err),
      rtn_msg: Some(payload.rtn_msg),
      read_id: payload.read_id,
      outcome,
    };
  }

  ClassifiedSubmitReadLogResponse {
    response_text: text.to_string(),
    submit_err: None,
    rtn_msg: None,
    read_id: None,
    outcome: TaskItemOutcome::Success,
  }
}

fn parse_submit_read_log_payload(text: &str) -> Option<SubmitReadLogPayload> {
  let envelope = serde_json::from_str::<SubmitReadLogResponseEnvelope>(text).ok()?;
  match envelope {
    SubmitReadLogResponseEnvelope::Wrapped { d } => {
      serde_json::from_str::<SubmitReadLogPayload>(&d).ok()
    }
    SubmitReadLogResponseEnvelope::Direct(payload) => Some(payload),
  }
}

fn resolve_task_reading_link(task: &MonthlyTask) -> Result<ReadingLinkData, AppError> {
  if task.reading_url.trim().is_empty() {
    return Ok(ReadingLinkData {
      s_course_id: task.s_course_id.clone(),
      s_manager_id: task.s_manager_id.clone(),
      referer: format!(
        "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID={}&UID={}",
        task.s_course_id, task.s_manager_id
      ),
    });
  }

  parse_reading_url(&task.reading_url)
}

fn resolve_request_reading_link(request: &TaskRunRequest) -> Result<ReadingLinkData, AppError> {
  if request.reading_url.trim().is_empty() {
    return Ok(ReadingLinkData {
      s_course_id: request.s_course_id.clone(),
      s_manager_id: request.s_manager_id.clone(),
      referer: format!(
        "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID={}&UID={}",
        request.s_course_id, request.s_manager_id
      ),
    });
  }

  parse_reading_url(&request.reading_url)
}

fn parse_reading_url(reading_url: &str) -> Result<ReadingLinkData, AppError> {
  let reading_url = reading_url.trim();
  if reading_url.is_empty() {
    return Err(AppError::ValidationError("请输入阅读链接".to_string()));
  }

  let outer_url = reqwest::Url::parse(reading_url)
    .map_err(|error| AppError::ValidationError(format!("阅读链接格式无效: {error}")))?;
  let redirect_uri = outer_url.query_pairs().find_map(|(key, value)| {
    if key == "redirect_uri" {
      Some(value.into_owned())
    } else {
      None
    }
  });
  let target_url = match redirect_uri.as_deref() {
    Some(redirect_uri) => reqwest::Url::parse(redirect_uri)
      .map_err(|error| AppError::ValidationError(format!("redirect_uri 格式无效: {error}")))?,
    None => outer_url.clone(),
  };
  let query_source = if redirect_uri.is_some() {
    "redirect_uri"
  } else {
    "阅读链接"
  };
  let mut s_course_id = None;
  let mut s_manager_id = None;

  for (key, value) in target_url.query_pairs() {
    match key.as_ref() {
      "CourseID" => s_course_id = Some(value.into_owned()),
      "UID" => s_manager_id = Some(value.into_owned()),
      _ => {}
    }
  }

  let s_course_id = s_course_id
    .filter(|value| !value.trim().is_empty())
    .ok_or_else(|| AppError::ValidationError(format!("{query_source} 缺少 CourseID")))?;
  let s_manager_id = s_manager_id
    .filter(|value| !value.trim().is_empty())
    .ok_or_else(|| AppError::ValidationError(format!("{query_source} 缺少 UID")))?;

  Ok(ReadingLinkData {
    s_course_id,
    s_manager_id,
    referer: redirect_uri.unwrap_or_else(|| reading_url.to_string()),
  })
}

fn prepare_run(
  db: &DbContext,
  request: &TaskRunRequest,
  run_date: Option<&str>,
) -> Result<PreparedRun, AppError> {
  validate_request(request)?;
  let runtime_data = load_runtime_data(db)?;
  let selected_shops = select_shops(runtime_data.shops, request)?;
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
  let matched_tasks = super::db::find_monthly_tasks_by_month_fc_course(
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

  let used_open_ids = super::db::get_used_open_ids_for_month(db, &month_prefix, task_type_opt)?;
  let selected_open_ids = select_open_ids(
    runtime_data.open_ids,
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

fn validate_request(request: &TaskRunRequest) -> Result<(), AppError> {
  if request.count == 0 {
    return Err(AppError::ValidationError(
      "-n/--count 必须大于 0".to_string(),
    ));
  }
  Ok(())
}

fn select_shops(
  shops: Vec<ShopRecord>,
  request: &TaskRunRequest,
) -> Result<Vec<ShopRecord>, AppError> {
  if !request.shopcodes.is_empty() {
    let requested_shopcodes = normalize_shopcodes(&request.shopcodes);
    if requested_shopcodes.is_empty() {
      return Err(AppError::ValidationError(
        "至少提供一个有效的 shopcode".to_string(),
      ));
    }

    let mut shops_by_code = shops
      .into_iter()
      .map(|shop| (shop.shop_code.clone(), shop))
      .collect::<HashMap<_, _>>();
    let mut matched_shops = Vec::with_capacity(requested_shopcodes.len());
    let mut missing_shopcodes = Vec::new();

    for shopcode in requested_shopcodes {
      match shops_by_code.remove(&shopcode) {
        Some(shop) => matched_shops.push(shop),
        None => missing_shopcodes.push(shopcode),
      }
    }

    if !missing_shopcodes.is_empty() {
      return Err(AppError::ResourceUnavailableError(format!(
        "未在 SQLite 门店数据中找到指定的门店代码: {:?}",
        missing_shopcodes
      )));
    }

    return Ok(matched_shops);
  }

  let matched_shops = shops
    .into_iter()
    .filter(|shop| shop.fc.as_deref() == Some(request.fc.as_str()))
    .collect::<Vec<_>>();

  if matched_shops.is_empty() {
    return Err(AppError::ResourceUnavailableError(format!(
      "未在 SQLite 门店数据中找到 FC={} 对应的门店",
      request.fc
    )));
  }

  if request.count > matched_shops.len() {
    return Err(AppError::ResourceUnavailableError(format!(
      "请求数量 {} 超过可用门店数量 {}",
      request.count,
      matched_shops.len()
    )));
  }

  Ok(sample_shops(matched_shops, request.count))
}

fn normalize_shopcodes(shopcodes: &[String]) -> Vec<String> {
  let mut seen = HashSet::new();

  shopcodes
    .iter()
    .map(|shopcode| shopcode.trim())
    .filter(|shopcode| !shopcode.is_empty())
    .filter(|shopcode| seen.insert((*shopcode).to_string()))
    .map(ToOwned::to_owned)
    .collect()
}

fn normalize_open_ids(open_ids: &[String]) -> Vec<String> {
  let mut seen = HashSet::new();

  open_ids
    .iter()
    .map(|open_id| open_id.trim())
    .filter(|open_id| !open_id.is_empty())
    .filter(|open_id| seen.insert((*open_id).to_string()))
    .map(ToOwned::to_owned)
    .collect()
}

fn select_unused_monthly_shops(
  shops: Vec<ShopRecord>,
  used_shop_codes: &HashSet<String>,
  count: usize,
  randomize: bool,
) -> Result<Vec<ShopRecord>, AppError> {
  let available_shops = shops
    .into_iter()
    .filter(|shop| !used_shop_codes.contains(shop.shop_code.as_str()))
    .collect::<Vec<_>>();

  if count > available_shops.len() {
    return Err(AppError::ResourceUnavailableError(format!(
      "本月剩余未使用门店数量 {}，不足以完成今日计划 {}。已使用的 shopcode 不能重复执行",
      available_shops.len(),
      count
    )));
  }

  if randomize {
    Ok(sample_shops(available_shops, count))
  } else {
    Ok(available_shops.into_iter().take(count).collect())
  }
}

fn select_open_ids(
  open_ids: Vec<OpenIdRecord>,
  fc_name: &str,
  used_open_ids: &HashSet<String>,
  excluded_open_ids: &HashSet<String>,
  count: usize,
) -> Result<Vec<String>, AppError> {
  let mut known_open_ids = used_open_ids
    .iter()
    .chain(excluded_open_ids.iter())
    .cloned()
    .collect::<HashSet<_>>();
  let available_open_ids = open_ids
    .into_iter()
    .inspect(|record| {
      known_open_ids.insert(record.open_id.clone());
    })
    .filter(|record| record.fc_name == fc_name)
    .filter(|record| !used_open_ids.contains(record.open_id.as_str()))
    .filter(|record| !excluded_open_ids.contains(record.open_id.as_str()))
    .map(|record| record.open_id)
    .collect::<Vec<_>>();

  let avail_len = available_open_ids.len();
  let mut selected_open_ids = sample_open_ids(available_open_ids, count.min(avail_len));
  if count > avail_len {
    let generated_count = count - avail_len;
    log::warn!(
      "FC={} 本月剩余可用 OpenID 数量 {}，不足以完成请求数量 {}；将自动生成 {} 个 OpenID 补齐",
      fc_name,
      avail_len,
      count,
      generated_count
    );
    selected_open_ids.extend(generate_open_ids(generated_count, &mut known_open_ids));
  }

  Ok(selected_open_ids)
}

fn generate_open_ids(count: usize, known_open_ids: &mut HashSet<String>) -> Vec<String> {
  let mut generated_open_ids = Vec::with_capacity(count);

  while generated_open_ids.len() < count {
    let open_id = generate_open_id();
    if known_open_ids.insert(open_id.clone()) {
      generated_open_ids.push(open_id);
    }
  }

  generated_open_ids
}

fn generate_open_id() -> String {
  let suffix_len = GENERATED_OPEN_ID_LEN - GENERATED_OPEN_ID_PREFIX.len();
  format!(
    "{GENERATED_OPEN_ID_PREFIX}{}",
    generate_open_id_suffix(suffix_len)
  )
}

fn generate_open_id_suffix(len: usize) -> String {
  const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
  let mut rng = rand::rng();
  (0..len)
    .map(|_| {
      let idx = rng.random_range(0..CHARSET.len());
      CHARSET[idx] as char
    })
    .collect()
}

fn sample_open_ids(mut open_ids: Vec<String>, count: usize) -> Vec<String> {
  let mut rng = rand::rng();
  open_ids.shuffle(&mut rng);
  open_ids.into_iter().take(count).collect()
}

fn sample_shops(mut shops: Vec<ShopRecord>, count: usize) -> Vec<ShopRecord> {
  let mut rng = rand::rng();
  shops.shuffle(&mut rng);
  shops.into_iter().take(count).collect()
}

fn build_http_client() -> Result<reqwest::Client, AppError> {
  let mut headers = HeaderMap::new();
  headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
  headers.insert(
    "Accept",
    HeaderValue::from_static("application/json, text/javascript, */*; q=0.01"),
  );
  headers.insert(
    "Origin",
    HeaderValue::from_static("https://e-learning.eau-thermale-avene.cn"),
  );
  headers.insert("Sec-Fetch-Site", HeaderValue::from_static("same-origin"));
  headers.insert(
    "X-Requested-With",
    HeaderValue::from_static("XMLHttpRequest"),
  );
  headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("cors"));
  headers.insert(
    USER_AGENT,
    HeaderValue::from_static(
      "Mozilla/5.0 (iPhone; CPU iPhone OS 26_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148 MicroMessenger/8.0.70(0x18004629) NetType/WIFI Language/en",
    ),
  );
  headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("empty"));
  headers.insert("Cache-Control", HeaderValue::from_static("no-cache"));
  headers.insert(
    "Accept-Language",
    HeaderValue::from_static("en-US,en;q=0.9"),
  );
  headers.insert("Priority", HeaderValue::from_static("u=3, i"));
  headers.insert(
    "Accept-Encoding",
    HeaderValue::from_static("gzip, deflate, br, zstd"),
  );
  headers.insert("Connection", HeaderValue::from_static("keep-alive"));

  reqwest::Client::builder()
    .default_headers(headers)
    .build()
    .map_err(|error| AppError::ExecutionError(format!("创建 HTTP 客户端失败: {error}")))
}

fn ensure_daily_task(
  db: &DbContext,
  task: &MonthlyTask,
  date: &str,
) -> Result<super::model::DailyTask, AppError> {
  if let Some(progress) = super::db::get_daily_task(db, &task.id, date)? {
    return Ok(progress);
  }

  let all_progress = super::db::get_all_daily_tasks_for_task(db, &task.id)?;
  let total_completed: usize = all_progress.iter().map(|p| p.completed_count).sum();
  let remaining_count = task.total_target.saturating_sub(total_completed);
  let target_count = random_daily_target(remaining_count);

  Ok(DailyTask {
    task_id: task.id.clone(),
    date: date.to_string(),
    target_count,
    completed_count: 0,
    is_locked: false,
    run_status: "not_started".to_string(),
    shopcodes: vec![],
  })
}

fn random_daily_target(remaining_count: usize) -> usize {
  if remaining_count == 0 {
    return 0;
  }

  rand::rng()
    .random_range(MIN_DAILY_TARGET..=MAX_DAILY_TARGET)
    .min(remaining_count)
}

pub fn estimate_target_days(total_target: usize) -> usize {
  if total_target == 0 {
    0
  } else {
    total_target.div_ceil(MAX_DAILY_TARGET)
  }
}

fn build_monthly_task_plan(
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
  let daily_targets = build_daily_targets(total_target);
  let start_date = extract_start_date(&task.created_at)?;
  let mut shop_offset = 0_usize;
  let mut daily_plans = Vec::with_capacity(daily_targets.len());

  for (offset_days, target_count) in daily_targets.into_iter().enumerate() {
    let next_offset = shop_offset + target_count;
    let shopcodes = selected_shops[shop_offset..next_offset]
      .iter()
      .map(|shop| shop.shop_code.clone())
      .collect::<Vec<_>>();
    daily_plans.push(DailyTask {
      task_id: task.id.clone(),
      date: add_days_to_date(&start_date, offset_days as i64),
      target_count,
      completed_count: 0,
      is_locked: false,
      run_status: "not_started".to_string(),
      shopcodes,
    });
    shop_offset = next_offset;
  }

  Ok(MonthlyTaskPlanPreview {
    eligible_shop_count,
    total_target,
    target_days: daily_plans.len(),
    daily_plans,
  })
}

fn calculate_monthly_target(eligible_shop_count: usize, task_type: &str) -> usize {
  if eligible_shop_count == 0 {
    return 0;
  }

  let (min_target, max_target) = calculate_monthly_target_bounds(eligible_shop_count, task_type);

  if min_target >= max_target {
    min_target
  } else {
    rand::rng().random_range(min_target..=max_target)
  }
}

fn calculate_monthly_target_bounds(eligible_shop_count: usize, task_type: &str) -> (usize, usize) {
  let (min_coverage, max_coverage) = match task_type {
    "Avene" => (70, 75),
    "Klorane" => (85, 95),
    _ => (100, 100),
  };

  let min_target = (eligible_shop_count * min_coverage).div_ceil(100);
  let max_target = ((eligible_shop_count * max_coverage) / 100)
    .max(min_target)
    .min(eligible_shop_count);

  (min_target.min(eligible_shop_count), max_target)
}

fn is_daily_task_completed(task: &DailyTask) -> bool {
  task.completed_count >= task.target_count
}

fn calculate_monthly_completion_target_for_task(
  shops: &[ShopRecord],
  fc_name: &str,
  task_type: &str,
) -> usize {
  let eligible_shop_count = shops
    .iter()
    .filter(|shop| shop.fc.as_deref() == Some(fc_name))
    .filter(|shop| task_type_matches_shop(task_type, shop.shop_type))
    .count();

  calculate_monthly_completion_target(eligible_shop_count, task_type)
}

fn calculate_monthly_completion_target(eligible_shop_count: usize, task_type: &str) -> usize {
  if eligible_shop_count == 0 {
    return 0;
  }

  let completion_percent = match task_type {
    "Avene" => AVENE_MONTHLY_COMPLETION_PERCENT,
    "Klorane" => KLORANE_MONTHLY_COMPLETION_PERCENT,
    _ => 100,
  };

  (eligible_shop_count * completion_percent)
    .div_ceil(100)
    .min(eligible_shop_count)
}

fn calculate_sleep_secs(total_to_run: usize) -> u64 {
  calculate_sleep_secs_at(Local::now(), total_to_run)
}

fn calculate_sleep_secs_at(now: chrono::DateTime<Local>, total_to_run: usize) -> u64 {
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

fn filter_task_shops(shops: Vec<ShopRecord>, fc_name: &str, task_type: &str) -> Vec<ShopRecord> {
  shops
    .into_iter()
    .filter(|shop| shop.fc.as_deref() == Some(fc_name))
    .filter(|shop| task_type_matches_shop(task_type, shop.shop_type))
    .collect()
}

fn task_type_matches_shop(task_type: &str, shop_type: u8) -> bool {
  match task_type {
    "Avene" => shop_type == SHOP_TYPE_AVENE || shop_type == SHOP_TYPE_AVENE_KLORANE,
    "Klorane" => shop_type == SHOP_TYPE_KLORANE || shop_type == SHOP_TYPE_AVENE_KLORANE,
    _ => true,
  }
}

fn select_planned_shops(
  shops: Vec<ShopRecord>,
  planned_shopcodes: &[String],
) -> Result<Vec<ShopRecord>, AppError> {
  let mut shops_by_code = shops
    .into_iter()
    .map(|shop| (shop.shop_code.clone(), shop))
    .collect::<HashMap<_, _>>();
  let mut selected = Vec::with_capacity(planned_shopcodes.len());

  for shopcode in planned_shopcodes {
    if let Some(shop) = shops_by_code.remove(shopcode) {
      selected.push(shop);
    } else {
      log::warn!("计划门店不存在或已被删除，跳过该门店: {}", shopcode);
    }
  }

  Ok(selected)
}

fn select_custom_monthly_shops(
  shops: Vec<ShopRecord>,
  requested_shopcodes: &[String],
) -> Result<Vec<ShopRecord>, AppError> {
  let mut shops_by_code = shops
    .into_iter()
    .map(|shop| (shop.shop_code.clone(), shop))
    .collect::<HashMap<_, _>>();
  let mut selected = Vec::with_capacity(requested_shopcodes.len());
  let mut missing = Vec::new();

  for shopcode in requested_shopcodes {
    if let Some(shop) = shops_by_code.remove(shopcode) {
      selected.push(shop);
    } else {
      missing.push(shopcode.clone());
    }
  }

  if !missing.is_empty() {
    return Err(AppError::ValidationError(format!(
      "以下 shopcode 不存在、已删除，或不属于当前 FC/任务类型: {}",
      missing.join(", ")
    )));
  }

  Ok(selected)
}

fn build_daily_targets(total_target: usize) -> Vec<usize> {
  if total_target == 0 {
    return Vec::new();
  }

  let mut rng = rand::rng();
  let mut remaining = total_target;
  let mut daily_targets = Vec::new();

  while remaining > MAX_DAILY_TARGET {
    let next_target = rng
      .random_range(MIN_DAILY_TARGET..=MAX_DAILY_TARGET)
      .min(remaining);
    daily_targets.push(next_target);
    remaining -= next_target;
  }

  if remaining > 0 {
    daily_targets.push(remaining);
  }

  daily_targets
}

fn extract_start_date(created_at: &str) -> Result<String, AppError> {
  created_at
    .get(0..10)
    .map(ToOwned::to_owned)
    .ok_or_else(|| AppError::ValidationError(format!("无效 created_at: {created_at}")))
}

fn now_timestamp_string() -> String {
  let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  timestamp.to_string()
}

fn current_date_string() -> String {
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  let days = (now / 86_400) as i64;
  let (year, month, day) = civil_from_days(days);
  format!("{year:04}-{month:02}-{day:02}")
}

fn current_datetime_string() -> String {
  Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn task_month_prefix_from_date(date: &str) -> Result<String, AppError> {
  let year = date
    .get(2..4)
    .ok_or_else(|| AppError::ValidationError(format!("无效日期格式: {date}")))?;
  let month = date
    .get(5..7)
    .ok_or_else(|| AppError::ValidationError(format!("无效日期格式: {date}")))?;
  Ok(format!("{year}{month}"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use chrono::Timelike;
  use tempfile::tempdir;

  use crate::core::{
    AppPaths, add_monthly_task, add_open_id, get_daily_task, import_shops, init_db_context,
    save_daily_task,
  };

  fn open_id_record(fc_name: &str, open_id: &str) -> OpenIdRecord {
    OpenIdRecord {
      fc_name: fc_name.to_string(),
      open_id: open_id.to_string(),
    }
  }

  #[tokio::test]
  async fn incomplete_daily_task_can_resume_after_monthly_threshold_reached() {
    let temp_dir = tempdir().expect("create temp dir");
    let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
    let db = init_db_context(&paths).expect("init db");
    let shops = (0..11)
      .map(|index| ShopRecord {
        province: "province".to_string(),
        city: "city".to_string(),
        shop_code: format!("shop-{index:02}"),
        shop_name: String::new(),
        fc: Some("fc-a".to_string()),
        shop_type: SHOP_TYPE_KLORANE,
      })
      .collect::<Vec<_>>();
    import_shops(&db, &shops).expect("import shops");
    add_open_id(&db, &open_id_record("fc-a", "open-id-1")).expect("add open id");

    let task = MonthlyTask {
      id: "task-1".to_string(),
      fc_name: "fc-a".to_string(),
      s_manager_id: "manager-1".to_string(),
      s_course_id: "course-1".to_string(),
      reading_url: String::new(),
      task_type: "Klorane".to_string(),
      total_target: 11,
      target_days: 2,
      created_at: "2026-08-01 00:00:00".to_string(),
      shopcodes: shops.iter().map(|shop| shop.shop_code.clone()).collect(),
      excluded_open_ids: Vec::new(),
    };
    add_monthly_task(&db, &task).expect("add task");
    save_daily_task(
      &db,
      &DailyTask {
        task_id: task.id.clone(),
        date: "2026-08-01".to_string(),
        target_count: 10,
        completed_count: 10,
        is_locked: true,
        run_status: "completed".to_string(),
        shopcodes: shops[..10]
          .iter()
          .map(|shop| shop.shop_code.clone())
          .collect(),
      },
    )
    .expect("save completed day");
    save_daily_task(
      &db,
      &DailyTask {
        task_id: task.id.clone(),
        date: "2026-08-02".to_string(),
        target_count: 1,
        completed_count: 0,
        is_locked: false,
        run_status: "paused".to_string(),
        shopcodes: vec![shops[10].shop_code.clone()],
      },
    )
    .expect("save interrupted day");

    let error =
      run_daily_task_with_progress_controlled(&db, &task.id, "2026-08-02", |_| {}, || true)
        .await
        .expect_err("the pause signal should stop the resumed task before requests are sent");

    assert!(
      matches!(error, AppError::Paused(_)),
      "unexpected error: {error:?}"
    );
    let pending = get_daily_task(&db, &task.id, "2026-08-02")
      .expect("load interrupted day")
      .expect("interrupted day should still exist");
    assert_eq!(pending.target_count, 1);
    assert_eq!(pending.completed_count, 0);
    assert_eq!(pending.run_status, "paused");
  }

  #[test]
  fn retry_validation_accepts_only_failed_records_with_complete_request_data() {
    let failed = TaskItemResult {
      result_id: Some(1),
      index: 1,
      executed_date: Some("2026-08-16 20:07:31".to_string()),
      submit_err: None,
      rtn_msg: None,
      read_id: None,
      open_id: "open-id-1".to_string(),
      shop_code: "shop-01".to_string(),
      province: "province".to_string(),
      city: "city".to_string(),
      http_status: None,
      response_text: Some("请求失败".to_string()),
      outcome: TaskItemOutcome::RequestError,
    };

    assert!(validate_retry_task_result(&failed).is_ok());

    let successful = TaskItemResult {
      submit_err: Some(0),
      outcome: TaskItemOutcome::Success,
      ..failed.clone()
    };
    assert!(matches!(
      validate_retry_task_result(&successful),
      Err(AppError::ValidationError(_))
    ));

    let missing_open_id = TaskItemResult {
      open_id: String::new(),
      ..failed
    };
    assert!(matches!(
      validate_retry_task_result(&missing_open_id),
      Err(AppError::ValidationError(_))
    ));
  }

  #[test]
  fn quick_run_archive_credits_results_to_planned_days_by_shopcode() {
    let temp_dir = tempdir().expect("create temp dir");
    let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
    let db = init_db_context(&paths).expect("init db");
    let task = MonthlyTask {
      id: "2608:70:course-1:Klorane".to_string(),
      fc_name: "fc-a".to_string(),
      s_manager_id: "manager-1".to_string(),
      s_course_id: "course-1".to_string(),
      reading_url: String::new(),
      task_type: "Klorane".to_string(),
      total_target: 4,
      target_days: 2,
      created_at: "2026-08-01 00:00:00".to_string(),
      shopcodes: vec![
        "A".to_string(),
        "B".to_string(),
        "C".to_string(),
        "D".to_string(),
      ],
      excluded_open_ids: Vec::new(),
    };
    add_monthly_task(&db, &task).expect("add task");
    for (date, shopcodes) in [
      ("2026-08-10", vec!["A".to_string(), "B".to_string()]),
      ("2026-08-11", vec!["C".to_string(), "D".to_string()]),
    ] {
      save_daily_task(
        &db,
        &DailyTask {
          task_id: task.id.clone(),
          date: date.to_string(),
          target_count: 2,
          completed_count: 0,
          is_locked: false,
          run_status: "not_started".to_string(),
          shopcodes,
        },
      )
      .expect("save daily plan");
    }
    let request = TaskRunRequest {
      s_course_id: task.s_course_id.clone(),
      s_manager_id: task.s_manager_id.clone(),
      reading_url: String::new(),
      fc: task.fc_name.clone(),
      count: 2,
      shopcodes: vec!["B".to_string(), "C".to_string()],
    };
    let items = [
      TaskItemResult {
        result_id: None,
        index: 1,
        executed_date: None,
        submit_err: Some(0),
        rtn_msg: Some("提交成功".to_string()),
        read_id: Some("read-b".to_string()),
        open_id: "open-b".to_string(),
        shop_code: "B".to_string(),
        province: "P".to_string(),
        city: "C".to_string(),
        http_status: Some(200),
        response_text: None,
        outcome: TaskItemOutcome::Success,
      },
      TaskItemResult {
        result_id: None,
        index: 2,
        executed_date: None,
        submit_err: Some(-1),
        rtn_msg: Some("提交失败".to_string()),
        read_id: None,
        open_id: "open-c".to_string(),
        shop_code: "C".to_string(),
        province: "P".to_string(),
        city: "C".to_string(),
        http_status: Some(200),
        response_text: None,
        outcome: TaskItemOutcome::RequestError,
      },
    ];

    archive_quick_run_results_for_date(&db, &request, &items, "2026-08-27")
      .expect("archive quick run");

    assert_eq!(
      get_daily_task(&db, &task.id, "2026-08-10")
        .expect("load first plan")
        .expect("first plan")
        .completed_count,
      1
    );
    assert_eq!(
      get_daily_task(&db, &task.id, "2026-08-11")
        .expect("load second plan")
        .expect("second plan")
        .completed_count,
      1
    );
    assert!(
      get_daily_task(&db, &task.id, "2026-08-27")
        .expect("load archive date")
        .is_none(),
      "quick-run archive must not create an empty daily plan"
    );
  }

  #[test]
  fn parse_reading_url_accepts_direct_course_page_link() {
    let link = parse_reading_url(
      "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID=65&UID=BAS-00868",
    )
    .expect("direct course page link should parse");

    assert_eq!(link.s_course_id, "65");
    assert_eq!(link.s_manager_id, "BAS-00868");
    assert_eq!(
      link.referer,
      "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID=65&UID=BAS-00868"
    );
  }

  #[test]
  fn parse_reading_url_accepts_redirect_uri_link() {
    let link = parse_reading_url(
      "https://open.weixin.qq.com/connect/oauth2/authorize?redirect_uri=https%3A%2F%2Fe-learning.eau-thermale-avene.cn%2FCommon%2FQCSCoursePage.aspx%3FCourseID%3D65%26UID%3DBAS-00868",
    )
    .expect("redirect uri link should parse");

    assert_eq!(link.s_course_id, "65");
    assert_eq!(link.s_manager_id, "BAS-00868");
    assert_eq!(
      link.referer,
      "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID=65&UID=BAS-00868"
    );
  }

  #[test]
  fn generated_open_id_uses_required_format() {
    let open_id = generate_open_id();

    assert!(open_id.starts_with(GENERATED_OPEN_ID_PREFIX));
    assert_eq!(open_id.len(), GENERATED_OPEN_ID_LEN);
  }

  #[test]
  fn select_open_ids_generates_missing_count() {
    let open_ids = vec![
      open_id_record("fc-a", "existing-open-id"),
      open_id_record("fc-b", "other-fc-open-id"),
    ];
    let used_open_ids = HashSet::new();
    let excluded_open_ids = HashSet::new();

    let selected = select_open_ids(open_ids, "fc-a", &used_open_ids, &excluded_open_ids, 3)
      .expect("open ids should be selected");

    assert_eq!(selected.len(), 3);
    assert!(selected.contains(&"existing-open-id".to_string()));
    assert!(!selected.contains(&"other-fc-open-id".to_string()));
    assert_eq!(
      selected
        .iter()
        .filter(|open_id| open_id.starts_with(GENERATED_OPEN_ID_PREFIX))
        .count(),
      2
    );
    assert!(
      selected
        .iter()
        .filter(|open_id| open_id.starts_with(GENERATED_OPEN_ID_PREFIX))
        .all(|open_id| open_id.len() == GENERATED_OPEN_ID_LEN)
    );
  }

  #[test]
  fn test_calculate_monthly_target_bounds() {
    // Avene: 70% ~ 75%
    let (min, max) = calculate_monthly_target_bounds(100, "Avene");
    assert_eq!(min, 70);
    assert_eq!(max, 75);

    // Klorane: 85% ~ 95%
    let (min, max) = calculate_monthly_target_bounds(100, "Klorane");
    assert_eq!(min, 85);
    assert_eq!(max, 95);

    // Other: 100% ~ 100%
    let (min, max) = calculate_monthly_target_bounds(100, "Other");
    assert_eq!(min, 100);
    assert_eq!(max, 100);
  }

  #[test]
  fn test_calculate_monthly_completion_target() {
    assert_eq!(calculate_monthly_completion_target(100, "Avene"), 65);
    assert_eq!(calculate_monthly_completion_target(101, "Avene"), 66);
    assert_eq!(calculate_monthly_completion_target(100, "Klorane"), 85);
    assert_eq!(calculate_monthly_completion_target(101, "Klorane"), 86);
    assert_eq!(calculate_monthly_completion_target(0, "Avene"), 0);
  }

  #[test]
  fn test_is_daily_task_completed_uses_target_count_only() {
    let task = DailyTask {
      task_id: "task-1".to_string(),
      date: "2026-07-06".to_string(),
      target_count: 10,
      completed_count: 9,
      is_locked: true,
      run_status: "completed".to_string(),
      shopcodes: Vec::new(),
    };

    assert!(!is_daily_task_completed(&task));

    let task = DailyTask {
      completed_count: 10,
      is_locked: false,
      run_status: "not_started".to_string(),
      ..task
    };

    assert!(is_daily_task_completed(&task));
  }

  #[test]
  fn test_calculate_sleep_secs_at() {
    let now = Local::now()
      .with_hour(20)
      .and_then(|time| time.with_minute(0))
      .and_then(|time| time.with_second(0))
      .and_then(|time| time.with_nanosecond(0))
      .unwrap();

    let sleep = calculate_sleep_secs_at(now, 10);
    assert_eq!(sleep, 360);

    let after_deadline = now.with_hour(21).unwrap();
    let sleep = calculate_sleep_secs_at(after_deadline, 10);
    assert_eq!(sleep, 0);
  }

  #[test]
  fn select_open_ids_generates_when_all_existing_are_unavailable() {
    let open_ids = vec![
      open_id_record("fc-a", "used-open-id"),
      open_id_record("fc-a", "excluded-open-id"),
      open_id_record("fc-b", "other-fc-open-id"),
    ];
    let used_open_ids = HashSet::from(["used-open-id".to_string()]);
    let excluded_open_ids = HashSet::from(["excluded-open-id".to_string()]);

    let selected = select_open_ids(open_ids, "fc-a", &used_open_ids, &excluded_open_ids, 2)
      .expect("open ids should be generated");

    assert_eq!(selected.len(), 2);
    assert!(selected.iter().all(|open_id| {
      open_id.starts_with(GENERATED_OPEN_ID_PREFIX)
        && open_id.len() == GENERATED_OPEN_ID_LEN
        && !used_open_ids.contains(open_id)
        && !excluded_open_ids.contains(open_id)
    }));
  }
}
