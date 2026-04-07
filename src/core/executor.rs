use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::Rng;
use rand::seq::SliceRandom;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use serde::Serialize;

use super::error::AppError;
use super::loader::load_runtime_data;
use super::model::{
  AppPaths, MonthlyTask, OpenIdRecord, QuickRunArchiveResult, QuickRunArchiveStatus, ShopRecord,
  TaskItemOutcome, TaskItemResult, TaskProgress, TaskRunRequest, TaskRunSummary,
};

const SUBMIT_READ_LOG_URL: &str =
  "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx/SubmitReadLog";

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

#[derive(Debug)]
struct PreparedRun {
  requested_count: usize,
  selected_open_ids: Vec<String>,
  selected_shops: Vec<ShopRecord>,
}

pub async fn run_task(
  paths: &AppPaths,
  request: TaskRunRequest,
) -> Result<TaskRunSummary, AppError> {
  run_task_with_progress(paths, request, |_| {}, None).await
}

pub async fn run_daily_task_with_progress<F>(
  paths: &AppPaths,
  task_id: &str,
  date: &str,
  mut on_progress: F,
) -> Result<TaskRunSummary, AppError>
where
  F: FnMut(TaskProgress),
{
  let tasks = super::db::get_all_monthly_tasks(paths)?;
  let task = tasks
    .into_iter()
    .find(|t| t.id == task_id)
    .ok_or_else(|| AppError::ResourceUnavailableError(format!("未找到月度任务: {}", task_id)))?;

  let runtime_data = load_runtime_data(paths)?;
  let client = build_http_client()?;
  let started_at = now_timestamp_string();

  // Get all progress to compute today's target
  let all_progress = super::db::get_all_progress_for_task(paths, task_id)?;
  let total_completed: usize = all_progress.iter().map(|p| p.completed_count).sum();

  if total_completed >= task.total_target {
    return Err(AppError::ExecutionError("该月度任务已全部完成".to_string()));
  }

  let mut today_progress = all_progress
    .iter()
    .find(|p| p.date == date)
    .cloned()
    .unwrap_or_else(|| {
      let remaining_count = task.total_target.saturating_sub(total_completed);
      let past_days = all_progress.iter().filter(|p| p.date != date).count();
      let remaining_days = task.target_days.saturating_sub(past_days).max(1);
      let target_count = (remaining_count as f64 / remaining_days as f64).ceil() as usize;
      super::model::DailyProgress {
        task_id: task_id.to_string(),
        date: date.to_string(),
        target_count,
        completed_count: 0,
      }
    });

  super::db::save_daily_progress(paths, &today_progress)?;

  if today_progress.completed_count >= today_progress.target_count {
    return Err(AppError::ExecutionError("今日任务已经完成".to_string()));
  }

  let to_run = today_progress.target_count - today_progress.completed_count;

  let valid_shops = runtime_data
    .shops
    .into_iter()
    .filter(|s| s.fc.as_deref() == Some(task.fc_name.as_str()))
    .collect::<Vec<_>>();
  if valid_shops.is_empty() {
    return Err(AppError::ResourceUnavailableError(format!(
      "未找到 FC={} 对应的门店",
      task.fc_name
    )));
  }

  let used_shop_codes = super::db::get_task_results(paths, task_id)?
    .into_iter()
    .map(|item| item.shop_code)
    .collect::<HashSet<_>>();
  let selected_shops = select_unused_monthly_shops(valid_shops, &used_shop_codes, to_run)?;
  let month_prefix = task_month_prefix_from_date(date)?;
  let used_open_ids = super::db::get_used_open_ids_for_month(paths, &month_prefix)?;
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
    // Sleep random 1-3 mins if not the first request
    if index > 0 {
      let sleep_secs = rand::thread_rng().gen_range(60..=180);
      tokio::time::sleep(std::time::Duration::from_secs(sleep_secs)).await;
    }

    let body = SubmitReadLogBody {
      s_course_id: &task.s_course_id,
      s_manager_id: &task.s_manager_id,
      open_id,
      province: &shop.province,
      city: &shop.city,
      shop_code: &shop.shop_code,
    };

    let uid = generate_random_string(6);
    let code_len = rand::thread_rng().gen_range(30..=40);
    let code = generate_random_string(code_len);
    let referer = format!(
      "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID={}&UID={}&code={}&state=STATE",
      task.s_course_id, uid, code
    );

    let item = execute_single_request(&client, &body, referer, index, open_id, shop).await;

    // Persist result to DB for history viewing
    let _ = super::db::save_task_result(paths, task_id, &item);

    if item.outcome == TaskItemOutcome::Success {
      today_progress.completed_count += 1;
      let _ = super::db::save_daily_progress(paths, &today_progress);
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
        Ok(text) => TaskItemResult {
          index: index + 1,
          open_id: open_id.to_string(),
          shop_code: shop.shop_code.clone(),
          province: shop.province.clone(),
          city: shop.city.clone(),
          http_status: Some(status),
          response_text: Some(text),
          error_message: None,
          outcome: TaskItemOutcome::Success,
        },
        Err(error) => TaskItemResult {
          index: index + 1,
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
  paths: &AppPaths,
  request: TaskRunRequest,
  mut on_progress: F,
  run_date: Option<&str>,
) -> Result<TaskRunSummary, AppError>
where
  F: FnMut(TaskProgress),
{
  let prepared = prepare_run(paths, &request, run_date)?;
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
    let code_len = rand::thread_rng().gen_range(30..=40);
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
          Ok(text) => TaskItemResult {
            index: index + 1,
            open_id: open_id.clone(),
            shop_code: shop.shop_code.clone(),
            province: shop.province.clone(),
            city: shop.city.clone(),
            http_status: Some(status),
            response_text: Some(text),
            error_message: None,
            outcome: TaskItemOutcome::Success,
          },
          Err(error) => TaskItemResult {
            index: index + 1,
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
  let archive_result = archive_quick_run_results(paths, &request, &items, run_date)?;

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
  paths: &AppPaths,
  request: &TaskRunRequest,
  items: &[TaskItemResult],
  run_date: Option<&str>,
) -> Result<QuickRunArchiveResult, AppError> {
  let run_date = run_date
    .map(ToOwned::to_owned)
    .unwrap_or_else(current_date_string);
  archive_quick_run_results_for_date(paths, request, items, &run_date)
}

fn archive_quick_run_results_for_date(
  paths: &AppPaths,
  request: &TaskRunRequest,
  items: &[TaskItemResult],
  run_date: &str,
) -> Result<QuickRunArchiveResult, AppError> {
  let month_prefix = task_month_prefix_from_date(&run_date)?;
  let matched_tasks = super::db::find_monthly_tasks_by_month_fc_course(
    paths,
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
  super::db::save_task_results(paths, &task.id, items)?;
  let success_count = items
    .iter()
    .filter(|item| item.outcome == TaskItemOutcome::Success)
    .count();

  if success_count > 0 {
    let mut progress = ensure_daily_progress(paths, task, run_date)?;
    progress.completed_count += success_count;
    super::db::save_daily_progress(paths, &progress)?;
  }

  Ok(QuickRunArchiveResult {
    status: QuickRunArchiveStatus::Archived,
    task_id: Some(task.id.clone()),
    message: format!("已追加到月度任务 {}", task.id),
  })
}

fn prepare_run(
  paths: &AppPaths,
  request: &TaskRunRequest,
  run_date: Option<&str>,
) -> Result<PreparedRun, AppError> {
  validate_request(request)?;
  let runtime_data = load_runtime_data(paths)?;
  let selected_shops = select_shops(runtime_data.shops, request)?;
  let run_date = run_date
    .map(ToOwned::to_owned)
    .unwrap_or_else(current_date_string);
  let month_prefix = task_month_prefix_from_date(&run_date)?;
  let used_open_ids = super::db::get_used_open_ids_for_month(paths, &month_prefix)?;
  let selected_open_ids = select_manager_open_ids(
    runtime_data.open_ids,
    &request.s_manager_id,
    &used_open_ids,
    request.count,
  )?;

  Ok(PreparedRun {
    requested_count: request.count,
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
    let codes_set: HashSet<&str> = request.shopcodes.iter().map(String::as_str).collect();
    let matched_shops = shops
      .into_iter()
      .filter(|shop| codes_set.contains(shop.shop_code.as_str()))
      .collect::<Vec<_>>();

    if matched_shops.is_empty() {
      return Err(AppError::ResourceUnavailableError(format!(
        "未在 shop.toml 中找到指定的门店代码: {:?}",
        request.shopcodes
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
      "未在 shop.toml 中找到 FC={} 对应的门店",
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

fn select_unused_monthly_shops(
  shops: Vec<ShopRecord>,
  used_shop_codes: &HashSet<String>,
  count: usize,
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

  Ok(sample_shops(available_shops, count))
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

  if count > available_open_ids.len() {
    return Err(AppError::ResourceUnavailableError(format!(
      "ManagerID={} 本月剩余可用 OpenID 数量 {}，不足以完成请求数量 {}。同月内使用过的 OpenID 不能重复执行",
      manager_id,
      available_open_ids.len(),
      count
    )));
  }

  Ok(sample_open_ids(available_open_ids, count))
}

fn sample_open_ids(mut open_ids: Vec<String>, count: usize) -> Vec<String> {
  let mut rng = rand::thread_rng();
  open_ids.shuffle(&mut rng);
  open_ids.into_iter().take(count).collect()
}

fn sample_shops(mut shops: Vec<ShopRecord>, count: usize) -> Vec<ShopRecord> {
  let mut rng = rand::thread_rng();
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
  paths: &AppPaths,
  task: &MonthlyTask,
  date: &str,
) -> Result<super::model::DailyProgress, AppError> {
  if let Some(progress) = super::db::get_daily_progress(paths, &task.id, date)? {
    return Ok(progress);
  }

  let all_progress = super::db::get_all_progress_for_task(paths, &task.id)?;
  let total_completed: usize = all_progress.iter().map(|p| p.completed_count).sum();
  let remaining_count = task.total_target.saturating_sub(total_completed);
  let past_days = all_progress.iter().filter(|p| p.date != date).count();
  let remaining_days = task.target_days.saturating_sub(past_days).max(1);
  let target_count = (remaining_count as f64 / remaining_days as f64).ceil() as usize;

  Ok(super::model::DailyProgress {
    task_id: task.id.clone(),
    date: date.to_string(),
    target_count,
    completed_count: 0,
  })
}

fn generate_random_string(len: usize) -> String {
  const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let mut rng = rand::thread_rng();
  (0..len)
    .map(|_| {
      let idx = rng.gen_range(0..CHARSET.len());
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

#[cfg(test)]
mod tests {
  use std::collections::HashSet;
  use std::fs;

  use tempfile::TempDir;

  use super::*;
  use crate::core::db;

  #[test]
  fn prepare_run_rejects_zero_count() {
    let (_temp_dir, paths) =
      create_config_dir(r#"openids = ["openid-a"]"#, SHOPS_TOML, PROVINCES_TOML);
    seed_manager_open_ids(&paths);
    let request = build_request(0, vec![]);

    let error = prepare_run(&paths, &request, None).unwrap_err();
    assert!(matches!(error, AppError::ValidationError(_)));
  }

  #[test]
  fn prepare_run_reports_missing_open_ids_file() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config_dir = temp_dir.path().join("config");
    fs::create_dir_all(&config_dir).expect("create config dir");
    fs::write(config_dir.join("shop.toml"), SHOPS_TOML).expect("write shop.toml");
    fs::write(config_dir.join("province.toml"), PROVINCES_TOML).expect("write province.toml");

    let paths = AppPaths::new_with_db_path(config_dir, temp_dir.path().join(".reading.db"));
    let request = build_request(1, vec![]);

    let error = prepare_run(&paths, &request, None).unwrap_err();
    assert!(matches!(error, AppError::ResourceUnavailableError(_)));
  }

  #[test]
  fn prepare_run_reports_parse_error() {
    let (_temp_dir, paths) =
      create_config_dir(r#"openids = ["openid-a""#, SHOPS_TOML, PROVINCES_TOML);
    let request = build_request(1, vec![]);

    let error = prepare_run(&paths, &request, None).unwrap_err();
    assert!(matches!(error, AppError::ConfigParseError { .. }));
  }

  #[test]
  fn prepare_run_rejects_insufficient_shops_for_fc() {
    let (_temp_dir, paths) = create_config_dir(
      r#"openids = ["openid-a", "openid-b"]"#,
      SHOPS_TOML,
      PROVINCES_TOML,
    );
    seed_manager_open_ids(&paths);
    let request = build_request(3, vec![]);

    let error = prepare_run(&paths, &request, None).unwrap_err();
    assert!(matches!(error, AppError::ResourceUnavailableError(_)));
    assert!(error.to_string().contains("超过可用门店数量"));
  }

  #[test]
  fn prepare_run_rejects_insufficient_open_ids() {
    let (_temp_dir, paths) =
      create_config_dir(r#"openids = ["openid-a"]"#, SHOPS_TOML, PROVINCES_TOML);
    seed_manager_open_ids(&paths);
    let request = build_request(2, vec![]);

    let error = prepare_run(&paths, &request, None).unwrap_err();
    assert!(matches!(error, AppError::ResourceUnavailableError(_)));
    assert!(error.to_string().contains("剩余可用 OpenID 数量"));
  }

  #[test]
  fn prepare_run_rejects_unmatched_shopcodes() {
    let (_temp_dir, paths) = create_config_dir(
      r#"openids = ["openid-a", "openid-b"]"#,
      SHOPS_TOML,
      PROVINCES_TOML,
    );
    seed_manager_open_ids(&paths);
    let request = build_request(1, vec!["not-exist".to_string()]);

    let error = prepare_run(&paths, &request, None).unwrap_err();
    assert!(matches!(error, AppError::ResourceUnavailableError(_)));
    assert!(
      error
        .to_string()
        .contains("未在 shop.toml 中找到指定的门店代码")
    );
  }

  #[test]
  fn prepare_run_deduplicates_open_ids() {
    let (_temp_dir, paths) = create_config_dir(
      r#"openids = ["openid-a", "openid-a", "openid-b"]"#,
      SHOPS_TOML,
      PROVINCES_TOML,
    );
    seed_manager_open_ids(&paths);
    let request = build_request(2, vec![]);

    let prepared = prepare_run(&paths, &request, None).expect("prepare run");
    assert_eq!(prepared.selected_open_ids.len(), 2);

    let unique = prepared
      .selected_open_ids
      .iter()
      .map(String::as_str)
      .collect::<HashSet<_>>();
    assert_eq!(unique.len(), 2);
  }

  #[test]
  fn select_manager_open_ids_filters_by_manager() {
    let selected = select_manager_open_ids(
      vec![
        OpenIdRecord {
          manager_id: "manager-a".to_string(),
          open_id: "openid-a".to_string(),
        },
        OpenIdRecord {
          manager_id: "manager-b".to_string(),
          open_id: "openid-b".to_string(),
        },
      ],
      "manager-a",
      &HashSet::new(),
      1,
    )
    .expect("select manager open ids");

    assert_eq!(selected, vec!["openid-a".to_string()]);
  }

  #[test]
  fn prepare_run_excludes_open_ids_used_in_same_month() {
    let (_temp_dir, paths) = create_config_dir(
      r#"openids = ["openid-a", "openid-b"]"#,
      SHOPS_TOML,
      PROVINCES_TOML,
    );
    seed_manager_open_ids(&paths);
    db::add_monthly_task(
      &paths,
      &MonthlyTask {
        id: "2604:course:manager".to_string(),
        fc_name: "fc-a".to_string(),
        s_manager_id: "manager".to_string(),
        s_course_id: "course".to_string(),
        total_target: 300,
        target_days: 20,
        created_at: "2026-04-01T00:00:00Z".to_string(),
      },
    )
    .expect("add monthly task");
    db::save_task_result(
      &paths,
      "2604:course:manager",
      &TaskItemResult {
        index: 1,
        open_id: "openid-a".to_string(),
        shop_code: "100".to_string(),
        province: "安徽".to_string(),
        city: "安庆".to_string(),
        http_status: Some(200),
        response_text: Some("ok".to_string()),
        error_message: None,
        outcome: TaskItemOutcome::Success,
      },
    )
    .expect("save task result");

    let prepared =
      prepare_run(&paths, &build_request(1, vec![]), Some("2026-04-07")).expect("prepare run");

    assert_eq!(prepared.selected_open_ids, vec!["openid-b".to_string()]);
  }

  #[test]
  fn archive_quick_run_results_appends_to_unique_monthly_task() {
    let (_temp_dir, paths) =
      create_config_dir(r#"openids = ["openid-a"]"#, SHOPS_TOML, PROVINCES_TOML);
    seed_manager_open_ids(&paths);
    db::init_db(&paths).expect("init db");
    db::add_monthly_task(
      &paths,
      &MonthlyTask {
        id: "2604:course:manager".to_string(),
        fc_name: "fc-a".to_string(),
        s_manager_id: "manager".to_string(),
        s_course_id: "course".to_string(),
        total_target: 300,
        target_days: 20,
        created_at: "2026-04-07T00:00:00Z".to_string(),
      },
    )
    .expect("add monthly task");

    let result = archive_quick_run_results_for_date(
      &paths,
      &build_request(1, vec![]),
      &[TaskItemResult {
        index: 1,
        open_id: "openid-a".to_string(),
        shop_code: "100".to_string(),
        province: "安徽".to_string(),
        city: "安庆".to_string(),
        http_status: Some(200),
        response_text: Some("ok".to_string()),
        error_message: None,
        outcome: TaskItemOutcome::Success,
      }],
      "2026-04-07",
    )
    .expect("archive results");

    assert_eq!(result.status, QuickRunArchiveStatus::Archived);
    assert_eq!(result.task_id.as_deref(), Some("2604:course:manager"));
    assert_eq!(
      db::get_task_results(&paths, "2604:course:manager")
        .expect("get results")
        .len(),
      1
    );

    let progress = db::get_daily_progress(&paths, "2604:course:manager", "2026-04-07")
      .expect("get daily progress")
      .expect("progress should exist");
    assert_eq!(progress.completed_count, 1);
  }

  #[test]
  fn archive_quick_run_results_reports_duplicate_tasks() {
    let (_temp_dir, paths) =
      create_config_dir(r#"openids = ["openid-a"]"#, SHOPS_TOML, PROVINCES_TOML);
    seed_manager_open_ids(&paths);
    db::init_db(&paths).expect("init db");
    for task_id in ["2604:course:manager-a", "2604:course:manager-b"] {
      db::add_monthly_task(
        &paths,
        &MonthlyTask {
          id: task_id.to_string(),
          fc_name: "fc-a".to_string(),
          s_manager_id: "manager".to_string(),
          s_course_id: "course".to_string(),
          total_target: 300,
          target_days: 20,
          created_at: "2026-04-07T00:00:00Z".to_string(),
        },
      )
      .expect("add monthly task");
    }

    let result =
      archive_quick_run_results_for_date(&paths, &build_request(1, vec![]), &[], "2026-04-07")
        .expect("archive should not fail");

    assert_eq!(result.status, QuickRunArchiveStatus::DuplicateTasks);
    assert!(result.message.contains("重复月度任务"));
  }

  #[test]
  fn select_unused_monthly_shops_excludes_used_shopcodes() {
    let used_shop_codes = ["100".to_string()].into_iter().collect::<HashSet<_>>();
    let shops = vec![
      ShopRecord {
        province: "安徽".to_string(),
        city: "安庆".to_string(),
        shop_code: "100".to_string(),
        fc: Some("fc-a".to_string()),
      },
      ShopRecord {
        province: "安徽".to_string(),
        city: "蚌埠".to_string(),
        shop_code: "101".to_string(),
        fc: Some("fc-a".to_string()),
      },
    ];

    let selected = select_unused_monthly_shops(shops, &used_shop_codes, 1).expect("select shops");

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].shop_code, "101");
  }

  fn create_config_dir(open_ids: &str, shops: &str, provinces: &str) -> (TempDir, AppPaths) {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config_dir = temp_dir.path().join("config");
    let db_path = temp_dir.path().join(".reading.db");
    fs::create_dir_all(&config_dir).expect("create config dir");

    fs::write(config_dir.join("open_ids.toml"), open_ids).expect("write open_ids.toml");
    fs::write(config_dir.join("shop.toml"), shops).expect("write shop.toml");
    fs::write(config_dir.join("province.toml"), provinces).expect("write province.toml");

    (temp_dir, AppPaths::new_with_db_path(config_dir.clone(), db_path))
  }

  fn seed_manager_open_ids(paths: &AppPaths) {
    db::init_db(paths).expect("init db");
    for open_id in crate::core::loader::load_open_ids_from_toml(&paths.config_dir.join("open_ids.toml"))
      .expect("load open ids")
    {
      db::add_open_id(
        paths,
        &OpenIdRecord {
          manager_id: "manager".to_string(),
          open_id,
        },
      )
      .expect("seed open id record");
    }
  }

  fn build_request(count: usize, shopcodes: Vec<String>) -> TaskRunRequest {
    TaskRunRequest {
      s_course_id: "course".to_string(),
      s_manager_id: "manager".to_string(),
      fc: "fc-a".to_string(),
      count,
      shopcodes,
    }
  }

  const SHOPS_TOML: &str = r#"
[[shops]]
Province = "安徽"
City = "安庆"
ShopCode = "100"
FC = "fc-a"

[[shops]]
Province = "安徽"
City = "蚌埠"
ShopCode = "101"
FC = "fc-a"
"#;

  const PROVINCES_TOML: &str = r#"
[[provinces]]
ProvinceName = "安徽"
CityName = "安庆"
"#;
}
