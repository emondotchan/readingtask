use rusqlite::{Connection, params};

use super::context::{DbContext, get_conn};
use crate::core::error::AppError;

pub fn init_db(db: &DbContext) -> Result<(), AppError> {
  let conn = get_conn(db)?;

  conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = -64000;
         PRAGMA temp_store = MEMORY;
         PRAGMA busy_timeout = 5000;
         CREATE TABLE IF NOT EXISTS open_ids (open_id TEXT PRIMARY KEY, manager_id TEXT);
         CREATE TABLE IF NOT EXISTS shops (shop_code TEXT PRIMARY KEY, province TEXT, city TEXT, shop_name TEXT NOT NULL DEFAULT '', fc TEXT, shop_type INTEGER);
         CREATE TABLE IF NOT EXISTS fcs (name TEXT PRIMARY KEY, manager_id TEXT);
         CREATE TABLE IF NOT EXISTS courses (month TEXT, course_id TEXT, task_type TEXT, PRIMARY KEY(month, course_id, task_type));
         CREATE TABLE IF NOT EXISTS monthly_tasks (id TEXT PRIMARY KEY, fc_name TEXT, s_manager_id TEXT, s_course_id TEXT, reading_url TEXT NOT NULL DEFAULT '', task_type TEXT, total_target INTEGER, target_days INTEGER, created_at TEXT, shopcodes_json TEXT, excluded_open_ids_json TEXT NOT NULL DEFAULT '[]');
         CREATE TABLE IF NOT EXISTS daily_tasks (task_id TEXT, date TEXT, target_count INTEGER, completed_count INTEGER, is_locked INTEGER NOT NULL DEFAULT 0, shopcodes_json TEXT NOT NULL DEFAULT '[]', PRIMARY KEY(task_id, date));
         CREATE TABLE IF NOT EXISTS task_results (id INTEGER PRIMARY KEY AUTOINCREMENT, task_id TEXT, timestamp_micros INTEGER, index_num INTEGER, open_id TEXT, shop_code TEXT, province TEXT, city TEXT, http_status INTEGER, response_text TEXT, outcome INTEGER, rtn_msg TEXT, read_id TEXT);
         CREATE TABLE IF NOT EXISTS sys_metadata (key TEXT PRIMARY KEY, value TEXT);"
    ).map_err(|e| AppError::ResourceUnavailableError(format!("创建表失败: {}", e)))?;

  ensure_monthly_tasks_schema(&conn)?;
  ensure_open_ids_schema(&conn)?;
  ensure_shops_schema(&conn)?;
  ensure_daily_tasks_schema(&conn)?;
  ensure_indexes(&conn)?;
  reconcile_all_daily_task_progress(&conn)?;

  Ok(())
}

fn ensure_indexes(conn: &Connection) -> Result<(), AppError> {
  conn.execute_batch(
    "CREATE INDEX IF NOT EXISTS idx_task_results_task_id ON task_results(task_id);
     CREATE INDEX IF NOT EXISTS idx_task_results_task_id_timestamp ON task_results(task_id, timestamp_micros);
     CREATE INDEX IF NOT EXISTS idx_task_results_shop_code ON task_results(shop_code);
     CREATE INDEX IF NOT EXISTS idx_shops_fc_shop_type ON shops(fc, shop_type);
     CREATE INDEX IF NOT EXISTS idx_open_ids_fc_name ON open_ids(fc_name);",
  )
  .map_err(|e| AppError::ResourceUnavailableError(format!("创建索引失败: {}", e)))?;
  Ok(())
}

fn ensure_monthly_tasks_schema(conn: &Connection) -> Result<(), AppError> {
  let mut stmt = conn
    .prepare("PRAGMA table_info(monthly_tasks)")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let columns = stmt
    .query_map([], |row| row.get::<_, String>(1))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if !columns
    .iter()
    .any(|column| column == "excluded_open_ids_json")
  {
    conn
      .execute(
        "ALTER TABLE monthly_tasks ADD COLUMN excluded_open_ids_json TEXT NOT NULL DEFAULT '[]'",
        [],
      )
      .map_err(|e| {
        AppError::ResourceUnavailableError(format!("更新 monthly_tasks 表失败: {}", e))
      })?;
  }

  if !columns.iter().any(|column| column == "reading_url") {
    conn
      .execute(
        "ALTER TABLE monthly_tasks ADD COLUMN reading_url TEXT NOT NULL DEFAULT ''",
        [],
      )
      .map_err(|e| {
        AppError::ResourceUnavailableError(format!("更新 monthly_tasks 表失败: {}", e))
      })?;
  }

  Ok(())
}

fn ensure_open_ids_schema(conn: &Connection) -> Result<(), AppError> {
  let mut stmt = conn
    .prepare("PRAGMA table_info(open_ids)")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let columns = stmt
    .query_map([], |row| row.get::<_, String>(1))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if !columns.iter().any(|column| column == "fc_name") {
    conn
      .execute(
        "ALTER TABLE open_ids ADD COLUMN fc_name TEXT NOT NULL DEFAULT ''",
        [],
      )
      .map_err(|e| AppError::ResourceUnavailableError(format!("更新 open_ids 表失败: {}", e)))?;
  }

  Ok(())
}

fn ensure_shops_schema(conn: &Connection) -> Result<(), AppError> {
  let mut stmt = conn
    .prepare("PRAGMA table_info(shops)")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let columns = stmt
    .query_map([], |row| row.get::<_, String>(1))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if !columns.iter().any(|column| column == "shop_name") {
    conn
      .execute(
        "ALTER TABLE shops ADD COLUMN shop_name TEXT NOT NULL DEFAULT ''",
        [],
      )
      .map_err(|e| AppError::ResourceUnavailableError(format!("更新 shops 表失败: {}", e)))?;
  }

  Ok(())
}

fn ensure_daily_tasks_schema(conn: &Connection) -> Result<(), AppError> {
  let mut stmt = conn
    .prepare("PRAGMA table_info(daily_tasks)")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let columns = stmt
    .query_map([], |row| row.get::<_, String>(1))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if !columns.iter().any(|column| column == "is_locked") {
    conn
      .execute(
        "ALTER TABLE daily_tasks ADD COLUMN is_locked INTEGER NOT NULL DEFAULT 0",
        [],
      )
      .map_err(|e| AppError::ResourceUnavailableError(format!("更新 daily_tasks 表失败: {}", e)))?;
  }

  if !columns.iter().any(|column| column == "shopcodes_json") {
    conn
      .execute(
        "ALTER TABLE daily_tasks ADD COLUMN shopcodes_json TEXT NOT NULL DEFAULT '[]'",
        [],
      )
      .map_err(|e| AppError::ResourceUnavailableError(format!("更新 daily_tasks 表失败: {}", e)))?;
  }

  if !columns.iter().any(|column| column == "run_status") {
    conn
      .execute(
        "ALTER TABLE daily_tasks ADD COLUMN run_status TEXT NOT NULL DEFAULT 'not_started'",
        [],
      )
      .map_err(|e| AppError::ResourceUnavailableError(format!("更新 daily_tasks 表失败: {}", e)))?;
  }
  conn
    .execute(
      "UPDATE daily_tasks SET run_status = 'completed' WHERE completed_count >= target_count",
      [],
    )
    .map_err(|e| AppError::ResourceUnavailableError(format!("更新 daily_tasks 状态失败: {}", e)))?;
  conn
    .execute(
      "UPDATE daily_tasks SET run_status = 'not_started', is_locked = 0 WHERE run_status = 'completed' AND completed_count < target_count",
      [],
    )
    .map_err(|e| AppError::ResourceUnavailableError(format!("更新 daily_tasks 未完成状态失败: {}", e)))?;
  conn
    .execute(
      "UPDATE daily_tasks SET run_status = 'paused' WHERE run_status = 'running'",
      [],
    )
    .map_err(|e| {
      AppError::ResourceUnavailableError(format!("重置 daily_tasks 运行状态失败: {}", e))
    })?;

  Ok(())
}

pub(crate) fn reconcile_all_daily_task_progress(conn: &Connection) -> Result<(), AppError> {
  let mut stmt = conn
    .prepare("SELECT DISTINCT task_id FROM daily_tasks")
    .map_err(|e| AppError::ResourceUnavailableError(format!("读取待对齐任务失败: {}", e)))?;
  let task_ids = stmt
    .query_map([], |row| row.get::<_, String>(0))
    .map_err(|e| AppError::ResourceUnavailableError(format!("读取待对齐任务列表失败: {}", e)))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(format!("解析待对齐任务列表失败: {}", e)))?;

  for task_id in task_ids {
    reconcile_daily_task_progress_with_conn(conn, &task_id)?;
  }

  Ok(())
}

pub(crate) fn reconcile_daily_task_progress_with_conn(
  conn: &Connection,
  task_id: &str,
) -> Result<(), AppError> {
  let mut daily_stmt = conn
    .prepare(
      "SELECT task_id, date, target_count, completed_count, is_locked, shopcodes_json, run_status
       FROM daily_tasks
       WHERE task_id = ?1
       ORDER BY date ASC",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let daily_tasks = daily_stmt
    .query_map(params![task_id], |row| {
      let shopcodes_json: String = row.get(5)?;
      Ok(crate::core::model::DailyTask {
        task_id: row.get(0)?,
        date: row.get(1)?,
        target_count: row.get::<_, i64>(2)? as usize,
        completed_count: row.get::<_, i64>(3)? as usize,
        is_locked: row.get::<_, i64>(4)? != 0,
        shopcodes: serde_json::from_str(&shopcodes_json).unwrap_or_default(),
        run_status: row.get(6)?,
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  if !daily_tasks.iter().any(|task| !task.shopcodes.is_empty()) {
    return Ok(());
  }

  let mut result_stmt = conn
    .prepare("SELECT DISTINCT shop_code FROM task_results WHERE task_id = ?1")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let requested_shopcodes = result_stmt
    .query_map(params![task_id], |row| row.get::<_, String>(0))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<std::collections::HashSet<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if requested_shopcodes.is_empty() {
    return Ok(());
  }

  conn
    .execute(
      "DELETE FROM daily_tasks
       WHERE task_id = ?1 AND COALESCE(json_array_length(shopcodes_json), 0) = 0",
      params![task_id],
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  for task in daily_tasks
    .into_iter()
    .filter(|task| !task.shopcodes.is_empty())
  {
    let completed_count = task
      .shopcodes
      .iter()
      .collect::<std::collections::HashSet<_>>()
      .into_iter()
      .filter(|shopcode| requested_shopcodes.contains(shopcode.as_str()))
      .count()
      .min(task.target_count);
    let is_completed = completed_count >= task.target_count;
    let run_status = if is_completed {
      "completed"
    } else if task.run_status == "running" {
      "running"
    } else if completed_count > 0 {
      "paused"
    } else {
      "not_started"
    };
    conn
      .execute(
        "UPDATE daily_tasks
         SET completed_count = ?1, is_locked = ?2, run_status = ?3
         WHERE task_id = ?4 AND date = ?5",
        params![
          completed_count as i64,
          if is_completed { 1_i64 } else { 0_i64 },
          run_status,
          task_id,
          task.date
        ],
      )
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  }

  Ok(())
}

pub fn reconcile_daily_task_progress_for_task(
  db: &DbContext,
  task_id: &str,
) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  reconcile_daily_task_progress_with_conn(&conn, task_id)
}
