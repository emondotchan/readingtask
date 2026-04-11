use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;
use rand::RngExt;
use rand::seq::SliceRandom;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};

use super::db::DbContext;
use super::error::AppError;
use super::loader::load_runtime_data;
use super::model::{
  DailyProgress, MonthlyTask, MonthlyTaskPlanPreview, OpenIdRecord, QuickRunArchiveResult,
  QuickRunArchiveStatus, SHOP_TYPE_AVENE, SHOP_TYPE_AVENE_KLORANE, SHOP_TYPE_KLORANE, ShopRecord,
  TaskItemOutcome, TaskItemResult, TaskProgress, TaskRunRequest, TaskRunSummary,
};

const SUBMIT_READ_LOG_URL: &str =
  "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx/SubmitReadLog";
const MIN_DAILY_TARGET: usize = 15;
const MAX_DAILY_TARGET: usize = 25;

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

pub fn preview_monthly_task_plan(
  db: &DbContext,
  task: &MonthlyTask,
) -> Result<MonthlyTaskPlanPreview, AppError> {
  let runtime_data = load_runtime_data(db)?;
  build_monthly_task_plan(task, runtime_data.shops)
}

pub fn create_monthly_task_with_plan(
  db: &DbContext,
  task: &MonthlyTask,
) -> Result<MonthlyTaskPlanPreview, AppError> {
  let plan = preview_monthly_task_plan(db, task)?;
  let planned_task = MonthlyTask {
    total_target: plan.total_target,
    target_days: plan.target_days,
    shopcodes: plan
      .daily_plans
      .iter()
      .flat_map(|daily_plan| daily_plan.shopcodes.iter().cloned())
      .collect(),
    ..task.clone()
  };

  super::db::add_monthly_task(db, &planned_task)?;
  for daily_plan in &plan.daily_plans {
    super::db::save_daily_progress(db, daily_plan)?;
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
  let client = build_http_client()?;
  let started_at = now_timestamp_string();

  // Get all progress to compute today's target
  let all_progress = super::db::get_all_progress_for_task(db, task_id)?;
  let total_completed: usize = all_progress.iter().map(|p| p.completed_count).sum();

  if total_completed >= task.total_target {
    log::warn!(
      "任务 {} ({}) 已全部完成 (completed: {}, target: {})",
      task.id,
      task.fc_name,
      total_completed,
      task.total_target
    );
    return Err(AppError::ExecutionError("该月度任务已全部完成".to_string()));
  }

  log::info!(
    "任务 {} 进度: 总完成 {}/{}, 准备初始化今日进度...",
    task.id,
    total_completed,
    task.total_target
  );

  let mut today_progress = ensure_daily_progress(db, &task, date)?;

  super::db::save_daily_progress(db, &today_progress)?;

  if today_progress.is_locked {
    return Err(AppError::ExecutionError(
      "今日任务已经执行完成，请明天再执行".to_string(),
    ));
  }

  if today_progress.completed_count >= today_progress.target_count {
    today_progress.is_locked = true;
    super::db::save_daily_progress(db, &today_progress)?;
    return Err(AppError::ExecutionError(
      "今日任务已经完成，请明天再执行".to_string(),
    ));
  }

  let to_run = today_progress.target_count - today_progress.completed_count;

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

  let selected_shops = if today_progress.shopcodes.is_empty() {
    let used_shop_codes = super::db::get_task_results(db, task_id)?
      .into_iter()
      .map(|item| item.shop_code)
      .collect::<HashSet<_>>();
    let random_shops = select_unused_monthly_shops(
      valid_shops,
      &used_shop_codes,
      to_run,
      task.shopcodes.is_empty(),
    )?;
    log::info!(
      "未指定今日门店，从未使用门店中随机挑选 {} 家",
      random_shops.len()
    );
    random_shops
  } else {
    let planned_shops = select_planned_shops(valid_shops, &today_progress.shopcodes)?;
    if planned_shops.len() < today_progress.shopcodes.len() {
      log::warn!(
        "已指定今日门店 {} 家，但仅筛选出 {} 家有效门店 (可能有部分被删除/不匹配)",
        today_progress.shopcodes.len(),
        planned_shops.len()
      );
    } else {
      log::info!("已指定今日门店，筛选出 {} 家", planned_shops.len());
    }

    // 如果所有的门店都失效了，也应该阻止执行并报错，或者仅仅当作没任务
    if planned_shops.is_empty() && !today_progress.shopcodes.is_empty() {
      return Err(AppError::ResourceUnavailableError(
        "今日计划的所有门店均不存在或已被删除".to_string(),
      ));
    }

    planned_shops
  };
  let month_prefix = task_month_prefix_from_date(date)?;
  let used_open_ids = super::db::get_used_open_ids_for_month(db, &month_prefix, Some(&task.task_type))?;
  let selected_open_ids = select_manager_open_ids(
    runtime_data.open_ids,
    &task.s_manager_id,
    &used_open_ids,
    to_run,
  )?;

  let mut items = Vec::new();

  for (index, (shop, open_id)) in selected_shops
    .iter()
    .zip(selected_open_ids.iter())
    .enumerate()
  {
    ensure_not_paused(&should_pause)?;

    // Sleep random 1-3 mins if not the first request
    if index > 0 {
      let sleep_secs = rand::rng().random_range(60..=180);
      sleep_with_pause_check(sleep_secs, &should_pause).await?;
    }

    ensure_not_paused(&should_pause)?;

    let body = SubmitReadLogBody {
      s_course_id: &task.s_course_id,
      s_manager_id: &task.s_manager_id,
      open_id,
      province: &shop.province,
      city: &shop.city,
      shop_code: &shop.shop_code,
    };

    let uid = generate_random_string(6);
    let code_len = rand::rng().random_range(30..=40);
    let code = generate_random_string(code_len);
    let referer = format!(
      "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID={}&UID={}&code={}&state=STATE",
      task.s_course_id, uid, code
    );

    let item = execute_single_request(&client, &body, referer, index, open_id, shop).await;

    // Persist result to DB for history viewing
    let _ = super::db::save_task_result(db, task_id, &item);

    if item.outcome == TaskItemOutcome::Success {
      log::info!("请求 {}/{}: 成功 ({})", index + 1, to_run, shop.shop_code);
      today_progress.completed_count += 1;
      let _ = super::db::save_daily_progress(db, &today_progress);
    } else {
      log::error!(
        "请求 {}/{}: 失败 ({}) - {:?}",
        index + 1,
        to_run,
        shop.shop_code,
        item.rtn_msg
      );
    }

    items.push(item.clone());
    on_progress(TaskProgress {
      task_id: Some(task_id.to_string()),
      processed_count: items.len(),
      requested_count: to_run,
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

  today_progress.is_locked = true;
  super::db::save_daily_progress(db, &today_progress)?;

  Ok(TaskRunSummary {
    requested_count: to_run,
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
          response_text: None,
          error_message: Some(format!("读取响应体失败: {error}")),
          outcome: TaskItemOutcome::ResponseReadError,
        },
      }
    }
    Err(error) => TaskItemResult {
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
      response_text: None,
      error_message: Some(format!("请求失败: {error}")),
      outcome: TaskItemOutcome::RequestError,
    },
  }
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

  for (index, (open_id, shop)) in prepared
    .selected_open_ids
    .iter()
    .zip(prepared.selected_shops.iter())
    .enumerate()
  {
    let body = SubmitReadLogBody {
      s_course_id: &request.s_course_id,
      s_manager_id: &request.s_manager_id,
      open_id,
      province: &shop.province,
      city: &shop.city,
      shop_code: &shop.shop_code,
    };

    let uid = generate_random_string(6);
    let code_len = rand::rng().random_range(30..=40);
    let code = generate_random_string(code_len);
    let referer = format!(
      "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID={}&UID={}&code={}&state=STATE",
      request.s_course_id, uid, code
    );

    let item = match client
      .post(SUBMIT_READ_LOG_URL)
      .header("Referer", &referer)
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
            response_text: None,
            error_message: Some(format!("读取响应体失败: {error}")),
            outcome: TaskItemOutcome::ResponseReadError,
          },
        }
      }
      Err(error) => TaskItemResult {
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
        response_text: None,
        error_message: Some(format!("请求失败: {error}")),
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
  let month_prefix = task_month_prefix_from_date(&run_date)?;
  let matched_tasks = super::db::find_monthly_tasks_by_month_fc_course(
    db,
    &month_prefix,
    &request.fc,
    &request.s_course_id,
  )?;

  if matched_tasks.is_empty() {
    return Ok(QuickRunArchiveResult {
      status: QuickRunArchiveStatus::NoMatchingTask,
      task_id: None,
      message: format!(
        "未找到可追加的月度任务：{} / {} / {}",
        month_prefix, request.fc, request.s_course_id
      ),
    });
  }

  if matched_tasks.len() > 1 {
    return Ok(QuickRunArchiveResult {
      status: QuickRunArchiveStatus::DuplicateTasks,
      task_id: None,
      message: format!(
        "检测到重复月度任务：{} / {} / {}，请先清理重复数据",
        month_prefix, request.fc, request.s_course_id
      ),
    });
  }

  let task = &matched_tasks[0];
  super::db::save_task_results(db, &task.id, items)?;
  let success_count = items
    .iter()
    .filter(|item| item.outcome == TaskItemOutcome::Success)
    .count();

  if success_count > 0 {
    let mut progress = ensure_daily_progress(db, task, run_date)?;
    progress.completed_count += success_count;
    super::db::save_daily_progress(db, &progress)?;
  }

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
  error_message: Option<String>,
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
    error_message: classified.error_message,
    outcome: classified.outcome,
  }
}

fn classify_submit_read_log_response(text: &str) -> ClassifiedSubmitReadLogResponse {
  if let Some(payload) = parse_submit_read_log_payload(text) {
    let response_text = payload.rtn_msg.clone();
    let error_message = if payload.err == 0 {
      None
    } else {
      Some(response_text.clone())
    };
    let outcome = if payload.err == 0 {
      TaskItemOutcome::Success
    } else {
      TaskItemOutcome::RequestError
    };

    return ClassifiedSubmitReadLogResponse {
      response_text,
      submit_err: Some(payload.err),
      rtn_msg: Some(payload.rtn_msg),
      read_id: payload.read_id,
      error_message,
      outcome,
    };
  }

  ClassifiedSubmitReadLogResponse {
    response_text: text.to_string(),
    submit_err: None,
    rtn_msg: Some(text.to_string()),
    read_id: None,
    error_message: None,
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

  let used_open_ids = super::db::get_used_open_ids_for_month(db, &month_prefix, task_type_opt)?;
  let selected_open_ids = select_manager_open_ids(
    runtime_data.open_ids,
    &request.s_manager_id,
    &used_open_ids,
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

fn select_manager_open_ids(
  open_ids: Vec<OpenIdRecord>,
  manager_id: &str,
  used_open_ids: &HashSet<String>,
  count: usize,
) -> Result<Vec<String>, AppError> {
  let available_open_ids = open_ids
    .into_iter()
    .filter(|record| record.manager_id == manager_id)
    .filter(|record| !used_open_ids.contains(record.open_id.as_str()))
    .map(|record| record.open_id)
    .collect::<Vec<_>>();

  if available_open_ids.is_empty() {
    return Err(AppError::ResourceUnavailableError(format!(
      "ManagerID={} 没有可用 OpenID，或本月可用 OpenID 已全部使用完毕",
      manager_id
    )));
  }

  let avail_len = available_open_ids.len();
  if count > avail_len {
    // 不应该阻止当日任务；如果可用 OpenID 不足，则只使用剩余的 OpenID 执行可执行的请求数量
    log::warn!(
      "ManagerID={} 本月剩余可用 OpenID 数量 {}，不足以完成请求数量 {}；将仅使用剩余数量执行请求",
      manager_id,
      avail_len,
      count
    );
    return Ok(sample_open_ids(available_open_ids, avail_len));
  }

  Ok(sample_open_ids(available_open_ids, count))
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

fn ensure_daily_progress(
  db: &DbContext,
  task: &MonthlyTask,
  date: &str,
) -> Result<super::model::DailyProgress, AppError> {
  if let Some(progress) = super::db::get_daily_progress(db, &task.id, date)? {
    return Ok(progress);
  }

  let all_progress = super::db::get_all_progress_for_task(db, &task.id)?;
  let total_completed: usize = all_progress.iter().map(|p| p.completed_count).sum();
  let remaining_count = task.total_target.saturating_sub(total_completed);
  let target_count = random_daily_target(remaining_count);

  Ok(DailyProgress {
    task_id: task.id.clone(),
    date: date.to_string(),
    target_count,
    completed_count: 0,
    is_locked: false,
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

  let total_target = calculate_monthly_target(eligible_shops.len(), &task.task_type);
  if total_target == 0 {
    return Err(AppError::ValidationError(format!(
      "FC={} 在任务类型={} 下没有可执行的月度目标",
      task.fc_name, task.task_type
    )));
  }

  let selected_shops = sample_shops(eligible_shops, total_target);
  let daily_targets = build_daily_targets(total_target);
  let start_date = extract_start_date(&task.created_at)?;
  let mut offset_days = 0_usize;
  let mut shop_offset = 0_usize;
  let mut daily_plans = Vec::with_capacity(daily_targets.len());

  for target_count in daily_targets {
    let next_offset = shop_offset + target_count;
    let shopcodes = selected_shops[shop_offset..next_offset]
      .iter()
      .map(|shop| shop.shop_code.clone())
      .collect::<Vec<_>>();
    daily_plans.push(DailyProgress {
      task_id: task.id.clone(),
      date: add_days_to_date(&start_date, offset_days as i64),
      target_count,
      completed_count: 0,
      is_locked: false,
      shopcodes,
    });
    shop_offset = next_offset;
    offset_days += 1;
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
    "Avene" => (75, 85),
    "Klorane" => (85, 95),
    _ => (100, 100),
  };

  let min_target = ((eligible_shop_count * min_coverage) + 99) / 100;
  let max_target = ((eligible_shop_count * max_coverage) / 100)
    .max(min_target)
    .min(eligible_shop_count);

  (min_target.min(eligible_shop_count), max_target)
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

fn add_days_to_date(date: &str, delta_days: i64) -> String {
  if let Some((year, month, day)) = parse_date_parts(date) {
    let days = days_from_civil(year, month, day) + delta_days;
    let (new_year, new_month, new_day) = civil_from_days(days);
    return format!("{new_year:04}-{new_month:02}-{new_day:02}");
  }

  date.to_string()
}

fn parse_date_parts(date: &str) -> Option<(i32, u32, u32)> {
  let year = date.get(0..4)?.parse().ok()?;
  let month = date.get(5..7)?.parse().ok()?;
  let day = date.get(8..10)?.parse().ok()?;
  Some((year, month, day))
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
  let year = year - if month <= 2 { 1 } else { 0 };
  let era = if year >= 0 { year } else { year - 399 } / 400;
  let yoe = year - era * 400;
  let month = month as i32;
  let day = day as i32;
  let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
  let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  (era * 146_097 + doe - 719_468) as i64
}

fn generate_random_string(len: usize) -> String {
  const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let mut rng = rand::rng();
  (0..len)
    .map(|_| {
      let idx = rng.random_range(0..CHARSET.len());
      CHARSET[idx] as char
    })
    .collect()
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

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
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
