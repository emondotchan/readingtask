use rusqlite::params;
use serde::Deserialize;
use std::collections::HashSet;

use super::context::{DbContext, get_conn};
use crate::core::error::AppError;
use crate::core::model::{TaskItemOutcome, TaskItemResult};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum LegacySubmitReadLogResponseEnvelope {
  Wrapped { d: String },
  Direct(LegacySubmitReadLogPayload),
}

#[derive(Debug, Deserialize)]
struct LegacySubmitReadLogPayload {
  err: i32,
  #[serde(rename = "RtnMsg")]
  rtn_msg: String,
  #[serde(rename = "ReadID")]
  read_id: Option<String>,
}

pub fn save_task_result(
  db: &DbContext,
  task_id: &str,
  item: &TaskItemResult,
) -> Result<i64, AppError> {
  let conn = get_conn(db)?;
  let outcome_val = item.outcome as i64;
  let now_micros = chrono::Utc::now().timestamp_micros();
  let http_status_val = item.http_status.map(|s| s as i64);

  conn.execute(
        "INSERT INTO task_results (task_id, timestamp_micros, index_num, open_id, shop_code, province, city, http_status, response_text, outcome, rtn_msg, read_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            task_id,
            now_micros,
            item.index as i64,
            item.open_id,
            item.shop_code,
            item.province,
            item.city,
            http_status_val,
            item.response_text,
            outcome_val,
            item.rtn_msg,
            item.read_id,
        ],
    ).map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let last_id = conn.last_insert_rowid();
  Ok(last_id)
}

pub fn save_task_results(
  db: &DbContext,
  task_id: &str,
  items: &[TaskItemResult],
) -> Result<(), AppError> {
  if items.is_empty() {
    return Ok(());
  }

  let mut conn = get_conn(db)?;
  let tx = conn
    .transaction()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let mut stmt = tx.prepare(
        "INSERT INTO task_results (task_id, timestamp_micros, index_num, open_id, shop_code, province, city, http_status, response_text, outcome, rtn_msg, read_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    ).map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  for item in items {
    let outcome_val = item.outcome as i64;
    let now_micros = chrono::Utc::now().timestamp_micros();
    let http_status_val = item.http_status.map(|s| s as i64);

    stmt
      .execute(params![
        task_id,
        now_micros,
        item.index as i64,
        item.open_id,
        item.shop_code,
        item.province,
        item.city,
        http_status_val,
        item.response_text,
        outcome_val,
        item.rtn_msg,
        item.read_id,
      ])
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  }

  drop(stmt);
  tx.commit()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn save_retried_task_result(
  db: &DbContext,
  task_id: &str,
  result_id: i64,
  item: &TaskItemResult,
) -> Result<i64, AppError> {
  let conn = get_conn(db)?;
  let outcome_val = item.outcome as i64;
  let now_micros = chrono::Utc::now().timestamp_micros();
  let http_status_val = item.http_status.map(|s| s as i64);

  let updated_rows = conn
    .execute(
      "UPDATE task_results
       SET timestamp_micros = ?1,
           index_num = ?2,
           open_id = ?3,
           shop_code = ?4,
           province = ?5,
           city = ?6,
           http_status = ?7,
           response_text = ?8,
           outcome = ?9,
           rtn_msg = ?10,
           read_id = ?11
       WHERE id = ?12 AND task_id = ?13",
      params![
        now_micros,
        item.index as i64,
        item.open_id,
        item.shop_code,
        item.province,
        item.city,
        http_status_val,
        item.response_text,
        outcome_val,
        item.rtn_msg,
        item.read_id,
        result_id,
        task_id,
      ],
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if updated_rows > 0 {
    return Ok(result_id);
  }

  save_task_result(db, task_id, item)
}

pub fn get_task_results(db: &DbContext, task_id: &str) -> Result<Vec<TaskItemResult>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare(
      "SELECT id, timestamp_micros, index_num, open_id, shop_code, province, city, http_status, response_text, outcome, rtn_msg, read_id
       FROM task_results
       WHERE task_id = ?1
       ORDER BY timestamp_micros DESC, id DESC",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let rows = stmt
    .query_map(params![task_id], map_task_result_row)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  Ok(rows)
}

pub fn get_task_result(
  db: &DbContext,
  task_id: &str,
  result_id: i64,
) -> Result<Option<TaskItemResult>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare(
      "SELECT id, timestamp_micros, index_num, open_id, shop_code, province, city, http_status, response_text, outcome, rtn_msg, read_id
       FROM task_results
       WHERE task_id = ?1 AND id = ?2
       LIMIT 1",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let mut rows = stmt
    .query_map(params![task_id, result_id], map_task_result_row)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if let Some(row) = rows.next() {
    Ok(Some(row.map_err(|e| {
      AppError::ResourceUnavailableError(e.to_string())
    })?))
  } else {
    Ok(None)
  }
}

pub fn get_task_result_shop_codes(
  db: &DbContext,
  task_id: &str,
) -> Result<HashSet<String>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare(
      "SELECT DISTINCT shop_code
       FROM task_results
       WHERE task_id = ?1",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let rows = stmt
    .query_map(params![task_id], |row| row.get::<_, String>(0))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<HashSet<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  Ok(rows)
}

pub fn get_task_result_shop_codes_for_date(
  db: &DbContext,
  task_id: &str,
  date: &str,
) -> Result<HashSet<String>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare(
      "SELECT DISTINCT shop_code
       FROM task_results
       WHERE task_id = ?1
         AND date(datetime(timestamp_micros / 1000000, 'unixepoch', 'localtime')) = ?2",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let rows = stmt
    .query_map(params![task_id, date], |row| row.get::<_, String>(0))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<HashSet<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  Ok(rows)
}

pub fn get_used_open_ids_for_month(
  db: &DbContext,
  month_prefix: &str,
  task_type: Option<&str>,
) -> Result<HashSet<String>, AppError> {
  let conn = get_conn(db)?;
  if let Some(task_type) = task_type {
    let mut stmt = conn
      .prepare(
        "SELECT DISTINCT r.open_id
         FROM task_results r
         JOIN monthly_tasks m ON r.task_id = m.id
         WHERE (m.id LIKE ?1 OR r.task_id LIKE ?1)
           AND m.task_type = ?2",
      )
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    let open_ids = stmt
      .query_map(params![format!("{}%", month_prefix), task_type], |row| {
        row.get(0)
      })
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
      .collect::<Result<HashSet<String>, _>>()
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    Ok(open_ids)
  } else {
    let mut stmt = conn
      .prepare(
        "SELECT DISTINCT open_id
         FROM task_results
         WHERE task_id LIKE ?1",
      )
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    let open_ids = stmt
      .query_map(params![format!("{}%", month_prefix)], |row| row.get(0))
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
      .collect::<Result<HashSet<String>, _>>()
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    Ok(open_ids)
  }
}

fn parse_legacy_submit_read_log_fields(
  response_text: Option<&str>,
) -> (Option<i32>, Option<String>, Option<String>) {
  let Some(text) = response_text else {
    return (None, None, None);
  };

  let parsed = serde_json::from_str::<LegacySubmitReadLogResponseEnvelope>(text).ok();
  let payload = match parsed {
    Some(LegacySubmitReadLogResponseEnvelope::Direct(payload)) => Some(payload),
    Some(LegacySubmitReadLogResponseEnvelope::Wrapped { d }) => {
      serde_json::from_str::<LegacySubmitReadLogPayload>(&d).ok()
    }
    None => None,
  };

  if let Some(payload) = payload {
    (Some(payload.err), Some(payload.rtn_msg), payload.read_id)
  } else {
    (None, None, None)
  }
}

fn infer_submit_err_from_outcome(outcome: &TaskItemOutcome) -> Option<i32> {
  match outcome {
    TaskItemOutcome::Success => Some(0),
    _ => None,
  }
}

fn map_task_result_row(row: &rusqlite::Row<'_>) -> Result<TaskItemResult, rusqlite::Error> {
  let outcome_num: i32 = row.get(9)?;
  let outcome = TaskItemOutcome::from(outcome_num);

  let response_text: Option<String> = row.get(8)?;
  let raw_rtn_msg: Option<String> = row.get(10)?;
  let raw_read_id: Option<String> = row.get(11)?;
  let (fallback_err, fallback_rtn_msg, fallback_read_id) =
    parse_legacy_submit_read_log_fields(response_text.as_deref());

  let rtn_msg = raw_rtn_msg.or(fallback_rtn_msg);
  let read_id = raw_read_id.or(fallback_read_id);
  let submit_err = fallback_err.or_else(|| infer_submit_err_from_outcome(&outcome));

  let micros: Option<i64> = row.get(1)?;
  let executed_date = micros.map(|m| {
    chrono::DateTime::from_timestamp_micros(m)
      .map(|dt| {
        let local_dt: chrono::DateTime<chrono::Local> = chrono::DateTime::from(dt);
        local_dt.format("%Y-%m-%d %H:%M:%S").to_string()
      })
      .unwrap_or_else(|| "".to_string())
  });

  Ok(TaskItemResult {
    result_id: Some(row.get(0)?),
    index: row.get::<_, i64>(2)? as usize,
    executed_date,
    submit_err,
    rtn_msg,
    read_id,
    open_id: row.get(3)?,
    shop_code: row.get(4)?,
    province: row.get(5)?,
    city: row.get(6)?,
    http_status: row.get::<_, Option<i64>>(7)?.map(|s| s as u16),
    response_text,
    outcome,
  })
}
