use std::path::PathBuf;
use std::sync::Arc;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;

use super::error::AppError;
use super::model::{
  AppPaths, DailyProgress, FcRecord, MonthlyTask, OpenIdRecord, SHOP_TYPE_AVENE,
  SHOP_TYPE_AVENE_KLORANE, SHOP_TYPE_KLORANE, ShopRecord, TaskItemResult,
};

type DbPool = Pool<SqliteConnectionManager>;

#[derive(Debug, Clone)]
pub struct DbContext {
  db_path: PathBuf,
  pool: Arc<DbPool>,
}

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

impl DbContext {
  pub fn from_paths(paths: &AppPaths) -> Result<Self, AppError> {
    Self::new(paths.db_path.clone())
  }

  pub fn new(db_path: PathBuf) -> Result<Self, AppError> {
    let manager = SqliteConnectionManager::file(&db_path);
    let pool = Pool::builder()
      .build(manager)
      .map_err(|e| AppError::ResourceUnavailableError(format!("无法创建连接池: {}", e)))?;

    Ok(Self {
      db_path,
      pool: Arc::new(pool),
    })
  }

  pub fn db_path(&self) -> &std::path::Path {
    &self.db_path
  }
}

pub fn init_db_context(paths: &AppPaths) -> Result<DbContext, AppError> {
  let db = DbContext::from_paths(paths)?;
  init_db(&db)?;
  Ok(db)
}

pub fn get_pool(db: &DbContext) -> Arc<DbPool> {
  Arc::clone(&db.pool)
}

fn get_conn(db: &DbContext) -> Result<r2d2::PooledConnection<SqliteConnectionManager>, AppError> {
  db.pool
    .get()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))
}

pub fn init_db(db: &DbContext) -> Result<(), AppError> {
  let conn = get_conn(db)?;

  conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS open_ids (open_id TEXT PRIMARY KEY, manager_id TEXT);
         CREATE TABLE IF NOT EXISTS shops (shop_code TEXT PRIMARY KEY, province TEXT, city TEXT, fc TEXT, shop_type INTEGER);
         CREATE TABLE IF NOT EXISTS fcs (name TEXT PRIMARY KEY, manager_id TEXT);
         CREATE TABLE IF NOT EXISTS monthly_tasks (id TEXT PRIMARY KEY, fc_name TEXT, s_manager_id TEXT, s_course_id TEXT, task_type TEXT, total_target INTEGER, target_days INTEGER, created_at TEXT, shopcodes_json TEXT);
         CREATE TABLE IF NOT EXISTS daily_progress (task_id TEXT, date TEXT, target_count INTEGER, completed_count INTEGER, is_locked INTEGER NOT NULL DEFAULT 0, shopcodes_json TEXT NOT NULL DEFAULT '[]', PRIMARY KEY(task_id, date));
         CREATE TABLE IF NOT EXISTS task_results (id INTEGER PRIMARY KEY AUTOINCREMENT, task_id TEXT, timestamp_micros INTEGER, index_num INTEGER, open_id TEXT, shop_code TEXT, province TEXT, city TEXT, http_status INTEGER, response_text TEXT, error_message TEXT, outcome INTEGER, rtn_msg TEXT, read_id TEXT);
         CREATE TABLE IF NOT EXISTS sys_metadata (key TEXT PRIMARY KEY, value TEXT);"
    ).map_err(|e| AppError::ResourceUnavailableError(format!("创建表失败: {}", e)))?;

  ensure_daily_progress_schema(&conn)?;
  ensure_task_results_schema(&conn)?;

  Ok(())
}

fn ensure_daily_progress_schema(conn: &rusqlite::Connection) -> Result<(), AppError> {
  let mut stmt = conn
    .prepare("PRAGMA table_info(daily_progress)")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let columns = stmt
    .query_map([], |row| row.get::<_, String>(1))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if !columns.iter().any(|column| column == "is_locked") {
    conn
      .execute(
        "ALTER TABLE daily_progress ADD COLUMN is_locked INTEGER NOT NULL DEFAULT 0",
        [],
      )
      .map_err(|e| {
        AppError::ResourceUnavailableError(format!("更新 daily_progress 表失败: {}", e))
      })?;
  }

  if !columns.iter().any(|column| column == "shopcodes_json") {
    conn
      .execute(
        "ALTER TABLE daily_progress ADD COLUMN shopcodes_json TEXT NOT NULL DEFAULT '[]'",
        [],
      )
      .map_err(|e| {
        AppError::ResourceUnavailableError(format!("更新 daily_progress 表失败: {}", e))
      })?;
  }

  Ok(())
}

fn ensure_task_results_schema(conn: &rusqlite::Connection) -> Result<(), AppError> {
  let mut stmt = conn
    .prepare("PRAGMA table_info(task_results)")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let columns = stmt
    .query_map([], |row| row.get::<_, String>(1))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if !columns.iter().any(|column| column == "rtn_msg") {
    conn
      .execute("ALTER TABLE task_results ADD COLUMN rtn_msg TEXT", [])
      .map_err(|e| {
        AppError::ResourceUnavailableError(format!("更新 task_results 表失败: {}", e))
      })?;
  }

  if !columns.iter().any(|column| column == "read_id") {
    conn
      .execute("ALTER TABLE task_results ADD COLUMN read_id TEXT", [])
      .map_err(|e| {
        AppError::ResourceUnavailableError(format!("更新 task_results 表失败: {}", e))
      })?;
  }

  Ok(())
}

pub fn get_all_open_id_records(db: &DbContext) -> Result<Vec<OpenIdRecord>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare("SELECT open_id, manager_id FROM open_ids")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let records = stmt
    .query_map([], |row| {
      Ok(OpenIdRecord {
        open_id: row.get(0)?,
        manager_id: row.get(1)?,
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(records)
}

pub fn get_all_open_ids(db: &DbContext) -> Result<Vec<String>, AppError> {
  Ok(
    get_all_open_id_records(db)?
      .into_iter()
      .map(|r| r.open_id)
      .collect(),
  )
}

pub fn add_open_id(db: &DbContext, record: &OpenIdRecord) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  conn
    .execute(
      "INSERT OR REPLACE INTO open_ids (open_id, manager_id) VALUES (?1, ?2)",
      params![record.open_id, record.manager_id],
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn delete_open_id(db: &DbContext, open_id: &str) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  conn
    .execute("DELETE FROM open_ids WHERE open_id = ?1", params![open_id])
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn import_open_ids_csv(db: &DbContext, csv_text: &str) -> Result<usize, AppError> {
  let records = parse_open_id_csv(csv_text)?;
  for record in &records {
    add_open_id(db, record)?;
  }
  Ok(records.len())
}

pub fn add_or_update_shop(db: &DbContext, shop: &ShopRecord) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  conn.execute("INSERT OR REPLACE INTO shops (shop_code, province, city, fc, shop_type) VALUES (?1, ?2, ?3, ?4, ?5)", params![shop.shop_code, shop.province, shop.city, shop.fc, shop.shop_type as i64]).map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn delete_shop(db: &DbContext, shop_code: &str) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  conn
    .execute("DELETE FROM shops WHERE shop_code = ?1", params![shop_code])
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn get_all_fcs(db: &DbContext) -> Result<Vec<FcRecord>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare("SELECT name, manager_id FROM fcs")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let fcs = stmt
    .query_map([], |row| {
      Ok(FcRecord {
        name: row.get(0)?,
        manager_id: row.get(1)?,
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(fcs)
}

pub fn add_or_update_fc(db: &DbContext, fc: &FcRecord) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  conn
    .execute(
      "INSERT OR REPLACE INTO fcs (name, manager_id) VALUES (?1, ?2)",
      params![fc.name, fc.manager_id],
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn delete_fc(db: &DbContext, name: &str) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  conn
    .execute("DELETE FROM fcs WHERE name = ?1", params![name])
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn get_all_monthly_tasks(db: &DbContext) -> Result<Vec<MonthlyTask>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn.prepare("SELECT id, fc_name, s_manager_id, s_course_id, task_type, total_target, target_days, created_at, shopcodes_json FROM monthly_tasks").map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let tasks = stmt
    .query_map([], |row| {
      let shopcodes_json: String = row.get(8)?;
      Ok(MonthlyTask {
        id: row.get(0)?,
        fc_name: row.get(1)?,
        s_manager_id: row.get(2)?,
        s_course_id: row.get(3)?,
        task_type: row.get(4)?,
        total_target: row.get::<_, i64>(5)? as usize,
        target_days: row.get::<_, i64>(6)? as usize,
        created_at: row.get(7)?,
        shopcodes: serde_json::from_str(&shopcodes_json).unwrap_or_default(),
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(tasks)
}

pub fn add_monthly_task(db: &DbContext, task: &MonthlyTask) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  let shopcodes_json = serde_json::to_string(&task.shopcodes)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  conn.execute("INSERT INTO monthly_tasks (id, fc_name, s_manager_id, s_course_id, task_type, total_target, target_days, created_at, shopcodes_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![task.id, task.fc_name, task.s_manager_id, task.s_course_id, task.task_type, task.total_target as i64, task.target_days as i64, task.created_at, shopcodes_json]).map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn delete_monthly_task(db: &DbContext, id: &str) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  conn
    .execute("DELETE FROM monthly_tasks WHERE id = ?1", params![id])
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn get_daily_progress(
  db: &DbContext,
  task_id: &str,
  date: &str,
) -> Result<Option<DailyProgress>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn.prepare("SELECT task_id, date, target_count, completed_count, is_locked, shopcodes_json FROM daily_progress WHERE task_id = ?1 AND date = ?2").map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let progress = stmt
    .query_row(params![task_id, date], |row| {
      let shopcodes_json: String = row.get(5)?;
      Ok(DailyProgress {
        task_id: row.get(0)?,
        date: row.get(1)?,
        target_count: row.get::<_, i64>(2)? as usize,
        completed_count: row.get::<_, i64>(3)? as usize,
        is_locked: row.get::<_, i64>(4)? != 0,
        shopcodes: serde_json::from_str(&shopcodes_json).unwrap_or_default(),
      })
    })
    .optional()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(progress)
}

pub fn get_all_progress_for_task(
  db: &DbContext,
  task_id: &str,
) -> Result<Vec<DailyProgress>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare(
      "SELECT task_id, date, target_count, completed_count, is_locked, shopcodes_json FROM daily_progress WHERE task_id = ?1 ORDER BY date ASC",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let progress = stmt
    .query_map(params![task_id], |row| {
      let shopcodes_json: String = row.get(5)?;
      Ok(DailyProgress {
        task_id: row.get(0)?,
        date: row.get(1)?,
        target_count: row.get::<_, i64>(2)? as usize,
        completed_count: row.get::<_, i64>(3)? as usize,
        is_locked: row.get::<_, i64>(4)? != 0,
        shopcodes: serde_json::from_str(&shopcodes_json).unwrap_or_default(),
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(progress)
}

pub fn save_daily_progress(db: &DbContext, progress: &DailyProgress) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  let shopcodes_json = serde_json::to_string(&progress.shopcodes)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  conn.execute("INSERT OR REPLACE INTO daily_progress (task_id, date, target_count, completed_count, is_locked, shopcodes_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![progress.task_id, progress.date, progress.target_count as i64, progress.completed_count as i64, if progress.is_locked { 1_i64 } else { 0_i64 }, shopcodes_json]).map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn save_task_result(
  db: &DbContext,
  task_id: &str,
  result: &TaskItemResult,
) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_micros() as i64;
  conn.execute("INSERT INTO task_results (task_id, timestamp_micros, index_num, open_id, shop_code, province, city, http_status, response_text, error_message, outcome, rtn_msg, read_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![task_id, now, result.index as i64, result.open_id, result.shop_code, result.province, result.city, result.http_status, result.response_text, result.error_message, result.outcome as i32, result.rtn_msg, result.read_id]).map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn save_task_results(
  db: &DbContext,
  task_id: &str,
  results: &[TaskItemResult],
) -> Result<(), AppError> {
  for result in results {
    save_task_result(db, task_id, result)?;
  }
  Ok(())
}

pub fn get_task_results(db: &DbContext, task_id: &str) -> Result<Vec<TaskItemResult>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn.prepare("SELECT strftime('%Y-%m-%d', timestamp_micros / 1000000.0, 'unixepoch', 'localtime') AS executed_date, index_num, open_id, shop_code, province, city, http_status, response_text, error_message, outcome, COALESCE(rtn_msg, response_text, error_message), read_id FROM task_results WHERE task_id = ?1 ORDER BY timestamp_micros DESC").map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let results = stmt
    .query_map(params![task_id], |row| {
      let response_text: Option<String> = row.get(7)?;
      let error_message: Option<String> = row.get(8)?;
      let stored_rtn_msg: Option<String> = row.get(10)?;
      let stored_read_id: Option<String> = row.get(11)?;
      let (parsed_submit_err, parsed_rtn_msg, parsed_read_id) =
        parse_legacy_submit_read_log_fields(response_text.as_deref());
      let outcome = row.get::<_, i32>(9)?.into();

      Ok(TaskItemResult {
        index: row.get::<_, i64>(1)? as usize,
        executed_date: row.get(0)?,
        submit_err: parsed_submit_err.or_else(|| infer_submit_err_from_outcome(outcome)),
        rtn_msg: stored_rtn_msg.or(parsed_rtn_msg),
        read_id: stored_read_id.or(parsed_read_id),
        open_id: row.get(2)?,
        shop_code: row.get(3)?,
        province: row.get(4)?,
        city: row.get(5)?,
        http_status: row.get(6)?,
        response_text,
        error_message,
        outcome,
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(results)
}

fn parse_legacy_submit_read_log_fields(
  text: Option<&str>,
) -> (Option<i32>, Option<String>, Option<String>) {
  let Some(text) = text else {
    return (None, None, None);
  };

  let parsed = serde_json::from_str::<LegacySubmitReadLogResponseEnvelope>(text).ok();
  match parsed {
    Some(LegacySubmitReadLogResponseEnvelope::Wrapped { d }) => {
      let payload = serde_json::from_str::<LegacySubmitReadLogPayload>(&d).ok();
      match payload {
        Some(payload) => (Some(payload.err), Some(payload.rtn_msg), payload.read_id),
        None => (None, None, None),
      }
    }
    Some(LegacySubmitReadLogResponseEnvelope::Direct(payload)) => {
      (Some(payload.err), Some(payload.rtn_msg), payload.read_id)
    }
    None => (None, None, None),
  }
}

fn infer_submit_err_from_outcome(outcome: super::model::TaskItemOutcome) -> Option<i32> {
  match outcome {
    super::model::TaskItemOutcome::Success => Some(0),
    super::model::TaskItemOutcome::RequestError => Some(-1),
    super::model::TaskItemOutcome::ResponseReadError => None,
  }
}

pub fn get_used_open_ids_for_month(
  db: &DbContext,
  month_prefix: &str,
) -> Result<std::collections::HashSet<String>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare("SELECT open_id FROM task_results WHERE task_id LIKE ?1")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let open_ids = stmt
    .query_map(params![format!("{}%", month_prefix)], |row| row.get(0))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<std::collections::HashSet<String>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(open_ids)
}

fn parse_open_id_csv(csv_text: &str) -> Result<Vec<OpenIdRecord>, AppError> {
  let mut records = Vec::new();
  for (index, raw_line) in csv_text.lines().enumerate() {
    let line = if index == 0 {
      raw_line.trim_start_matches('\u{feff}').trim()
    } else {
      raw_line.trim()
    };
    if line.is_empty() {
      continue;
    }
    let columns = line
      .split(',')
      .map(|c| c.trim().trim_matches('"'))
      .collect::<Vec<_>>();
    if index == 0
      && columns.len() >= 2
      && columns[0].to_ascii_lowercase().contains("manager")
      && columns[1].to_ascii_lowercase().contains("openid")
    {
      continue;
    }
    if columns.len() < 2 {
      continue;
    }
    records.push(OpenIdRecord {
      manager_id: columns[0].to_string(),
      open_id: columns[1].to_string(),
    });
  }
  Ok(records)
}

pub fn find_monthly_tasks_by_month_fc_course(
  db: &DbContext,
  month_prefix: &str,
  fc_name: &str,
  course_id: &str,
) -> Result<Vec<MonthlyTask>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn.prepare("SELECT id, fc_name, s_manager_id, s_course_id, task_type, total_target, target_days, created_at, shopcodes_json FROM monthly_tasks WHERE id LIKE ?1 AND fc_name = ?2 AND s_course_id = ?3").map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let tasks = stmt
    .query_map(
      params![format!("{}%", month_prefix), fc_name, course_id],
      |row| {
        let shopcodes_json: String = row.get(8)?;
        Ok(MonthlyTask {
          id: row.get(0)?,
          fc_name: row.get(1)?,
          s_manager_id: row.get(2)?,
          s_course_id: row.get(3)?,
          task_type: row.get(4)?,
          total_target: row.get::<_, i64>(5)? as usize,
          target_days: row.get::<_, i64>(6)? as usize,
          created_at: row.get(7)?,
          shopcodes: serde_json::from_str(&shopcodes_json).unwrap_or_default(),
        })
      },
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(tasks)
}

pub fn get_all_shops(db: &DbContext) -> Result<Vec<ShopRecord>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare("SELECT province, city, shop_code, fc, shop_type FROM shops")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let shops = stmt
    .query_map([], |row| {
      Ok(ShopRecord {
        province: row.get(0)?,
        city: row.get(1)?,
        shop_code: row.get(2)?,
        fc: row.get(3)?,
        shop_type: row.get::<_, i64>(4)? as u8,
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  Ok(shops)
}

pub fn get_shop_count_by_fc_and_type(
  db: &DbContext,
  fc_name: &str,
  task_type: &str,
) -> Result<usize, AppError> {
  let conn = get_conn(db)?;
  let shop_type = match task_type {
    "Avene" => SHOP_TYPE_AVENE,
    "Klorane" => SHOP_TYPE_KLORANE,
    _ => SHOP_TYPE_AVENE,
  };

  let sql = format!(
    "SELECT COUNT(*) FROM shops WHERE fc = ?1 AND (shop_type = ?2 OR shop_type = {})",
    SHOP_TYPE_AVENE_KLORANE
  );
  log::debug!(
    "[DEBUG] get_shop_count_by_fc_and_type: SQL={sql} fc_name={fc_name} shop_type={shop_type}"
  );
  let mut stmt = conn
    .prepare(&sql)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let count: i64 = stmt
    .query_row(params![fc_name, shop_type], |row| row.get(0))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  log::debug!("[DEBUG] get_shop_count_by_fc_and_type: result count={count}");

  let eligible_count = count as usize;
  let target_count = match task_type {
    "Avene" => ((eligible_count * 4) + 4) / 5,
    "Klorane" => eligible_count,
    _ => eligible_count,
  };

  Ok(target_count)
}
