use std::path::PathBuf;
use std::sync::Arc;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;

use super::error::AppError;
use super::model::{
  AppPaths, CourseRecord, DailyTask, FcRecord, MonthlyTask, OpenIdRecord, SHOP_TYPE_AVENE,
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
         CREATE TABLE IF NOT EXISTS shops (shop_code TEXT PRIMARY KEY, province TEXT, city TEXT, shop_name TEXT NOT NULL DEFAULT '', fc TEXT, shop_type INTEGER);
         CREATE TABLE IF NOT EXISTS fcs (name TEXT PRIMARY KEY, manager_id TEXT);
         CREATE TABLE IF NOT EXISTS courses (month TEXT, course_id TEXT, task_type TEXT, PRIMARY KEY(month, course_id, task_type));
         CREATE TABLE IF NOT EXISTS monthly_tasks (id TEXT PRIMARY KEY, fc_name TEXT, s_manager_id TEXT, s_course_id TEXT, task_type TEXT, total_target INTEGER, target_days INTEGER, created_at TEXT, shopcodes_json TEXT, excluded_open_ids_json TEXT NOT NULL DEFAULT '[]');
         CREATE TABLE IF NOT EXISTS daily_tasks (task_id TEXT, date TEXT, target_count INTEGER, completed_count INTEGER, is_locked INTEGER NOT NULL DEFAULT 0, shopcodes_json TEXT NOT NULL DEFAULT '[]', PRIMARY KEY(task_id, date));
         CREATE TABLE IF NOT EXISTS task_results (id INTEGER PRIMARY KEY AUTOINCREMENT, task_id TEXT, timestamp_micros INTEGER, index_num INTEGER, open_id TEXT, shop_code TEXT, province TEXT, city TEXT, http_status INTEGER, response_text TEXT, outcome INTEGER, rtn_msg TEXT, read_id TEXT);
         CREATE TABLE IF NOT EXISTS sys_metadata (key TEXT PRIMARY KEY, value TEXT);"
    ).map_err(|e| AppError::ResourceUnavailableError(format!("创建表失败: {}", e)))?;

  ensure_monthly_tasks_schema(&conn)?;
  ensure_open_ids_schema(&conn)?;
  ensure_shops_schema(&conn)?;
  ensure_daily_tasks_schema(&conn)?;

  Ok(())
}

fn ensure_shops_schema(conn: &rusqlite::Connection) -> Result<(), AppError> {
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

fn ensure_open_ids_schema(conn: &rusqlite::Connection) -> Result<(), AppError> {
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

fn ensure_monthly_tasks_schema(conn: &rusqlite::Connection) -> Result<(), AppError> {
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

fn ensure_daily_tasks_schema(conn: &rusqlite::Connection) -> Result<(), AppError> {
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
      "UPDATE daily_tasks SET run_status = 'completed' WHERE is_locked != 0 OR completed_count >= target_count",
      [],
    )
    .map_err(|e| AppError::ResourceUnavailableError(format!("更新 daily_tasks 状态失败: {}", e)))?;
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

pub fn get_all_open_id_records(db: &DbContext) -> Result<Vec<OpenIdRecord>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare("SELECT open_id, fc_name FROM open_ids")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let records = stmt
    .query_map([], |row| {
      Ok(OpenIdRecord {
        open_id: row.get(0)?,
        fc_name: row.get(1)?,
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
      "INSERT OR REPLACE INTO open_ids (open_id, manager_id, fc_name) VALUES (?1, '', ?2)",
      params![record.open_id, record.fc_name],
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

pub fn import_shops(db: &DbContext, shops: &[ShopRecord]) -> Result<usize, AppError> {
  let mut conn = get_conn(db)?;
  let tx = conn
    .transaction()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  for shop in shops {
    tx.execute("INSERT OR REPLACE INTO shops (shop_code, province, city, shop_name, fc, shop_type) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![shop.shop_code, shop.province, shop.city, shop.shop_name, shop.fc, shop.shop_type as i64]).map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  }

  tx.commit()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(shops.len())
}

pub fn delete_all_shops(db: &DbContext) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  conn
    .execute("DELETE FROM shops", [])
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn update_shop_type_by_codes(
  db: &DbContext,
  shop_codes: &[String],
  shop_type: u8,
) -> Result<usize, AppError> {
  let mut conn = get_conn(db)?;
  let tx = conn
    .transaction()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let mut updated = 0_usize;

  for shop_code in shop_codes {
    updated += tx
      .execute(
        "UPDATE shops SET shop_type = ?1 WHERE shop_code = ?2",
        params![shop_type as i64, shop_code],
      )
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  }

  tx.commit()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(updated)
}

pub fn get_all_fcs(db: &DbContext) -> Result<Vec<FcRecord>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare("SELECT name FROM fcs")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let fcs = stmt
    .query_map([], |row| Ok(FcRecord { name: row.get(0)? }))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(fcs)
}

pub fn add_or_update_fc(
  db: &DbContext,
  previous_name: Option<&str>,
  fc: &FcRecord,
) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  let tx = conn
    .unchecked_transaction()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let previous_name = previous_name.map(str::trim).filter(|name| !name.is_empty());

  if let Some(previous_name) = previous_name
    && previous_name != fc.name
  {
    tx.execute("DELETE FROM fcs WHERE name = ?1", params![previous_name])
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  }

  tx.execute(
    "INSERT OR REPLACE INTO fcs (name, manager_id) VALUES (?1, '')",
    params![fc.name],
  )
  .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  tx.commit()
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

pub fn get_all_courses(db: &DbContext) -> Result<Vec<CourseRecord>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare("SELECT month, course_id, task_type FROM courses ORDER BY month DESC, task_type ASC, course_id ASC")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let courses = stmt
    .query_map([], |row| {
      Ok(CourseRecord {
        month: row.get(0)?,
        course_id: row.get(1)?,
        task_type: row.get(2)?,
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(courses)
}

pub fn add_or_update_course(
  db: &DbContext,
  previous: Option<(&str, &str, &str)>,
  course: &CourseRecord,
) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  let tx = conn
    .unchecked_transaction()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if let Some((previous_month, previous_course_id, previous_task_type)) = previous
    && (previous_month != course.month
      || previous_course_id != course.course_id
      || previous_task_type != course.task_type)
  {
    tx.execute(
      "DELETE FROM courses WHERE month = ?1 AND course_id = ?2 AND task_type = ?3",
      params![previous_month, previous_course_id, previous_task_type],
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  }

  tx.execute(
    "INSERT OR REPLACE INTO courses (month, course_id, task_type) VALUES (?1, ?2, ?3)",
    params![course.month, course.course_id, course.task_type],
  )
  .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  tx.commit()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn delete_course(
  db: &DbContext,
  month: &str,
  course_id: &str,
  task_type: &str,
) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  conn
    .execute(
      "DELETE FROM courses WHERE month = ?1 AND course_id = ?2 AND task_type = ?3",
      params![month, course_id, task_type],
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn get_all_monthly_tasks(db: &DbContext) -> Result<Vec<MonthlyTask>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn.prepare("SELECT id, fc_name, s_manager_id, s_course_id, reading_url, task_type, total_target, target_days, created_at, shopcodes_json, excluded_open_ids_json FROM monthly_tasks").map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let tasks = stmt
    .query_map([], |row| {
      let shopcodes_json: String = row.get(9)?;
      let excluded_open_ids_json: String = row.get(10)?;
      Ok(MonthlyTask {
        id: row.get(0)?,
        fc_name: row.get(1)?,
        s_manager_id: row.get(2)?,
        s_course_id: row.get(3)?,
        reading_url: row.get(4)?,
        task_type: row.get(5)?,
        total_target: row.get::<_, i64>(6)? as usize,
        target_days: row.get::<_, i64>(7)? as usize,
        created_at: row.get(8)?,
        shopcodes: serde_json::from_str(&shopcodes_json).unwrap_or_default(),
        excluded_open_ids: serde_json::from_str(&excluded_open_ids_json).unwrap_or_default(),
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
  let excluded_open_ids_json = serde_json::to_string(&task.excluded_open_ids)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  conn.execute("INSERT INTO monthly_tasks (id, fc_name, s_manager_id, s_course_id, reading_url, task_type, total_target, target_days, created_at, shopcodes_json, excluded_open_ids_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![task.id, task.fc_name, task.s_manager_id, task.s_course_id, task.reading_url, task.task_type, task.total_target as i64, task.target_days as i64, task.created_at, shopcodes_json, excluded_open_ids_json]).map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn delete_monthly_task(db: &DbContext, id: &str) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  conn
    .execute("DELETE FROM monthly_tasks WHERE id = ?1", params![id])
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn get_daily_task(
  db: &DbContext,
  task_id: &str,
  date: &str,
) -> Result<Option<DailyTask>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn.prepare("SELECT task_id, date, target_count, completed_count, is_locked, shopcodes_json, run_status FROM daily_tasks WHERE task_id = ?1 AND date = ?2").map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let progress = stmt
    .query_row(params![task_id, date], |row| {
      let shopcodes_json: String = row.get(5)?;
      Ok(DailyTask {
        task_id: row.get(0)?,
        date: row.get(1)?,
        target_count: row.get::<_, i64>(2)? as usize,
        completed_count: row.get::<_, i64>(3)? as usize,
        is_locked: row.get::<_, i64>(4)? != 0,
        run_status: row.get(6)?,
        shopcodes: serde_json::from_str(&shopcodes_json).unwrap_or_default(),
      })
    })
    .optional()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(progress)
}

pub fn get_all_daily_tasks_for_task(
  db: &DbContext,
  task_id: &str,
) -> Result<Vec<DailyTask>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare(
      "SELECT task_id, date, target_count, completed_count, is_locked, shopcodes_json, run_status FROM daily_tasks WHERE task_id = ?1 ORDER BY date ASC",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let progress = stmt
    .query_map(params![task_id], |row| {
      let shopcodes_json: String = row.get(5)?;
      Ok(DailyTask {
        task_id: row.get(0)?,
        date: row.get(1)?,
        target_count: row.get::<_, i64>(2)? as usize,
        completed_count: row.get::<_, i64>(3)? as usize,
        is_locked: row.get::<_, i64>(4)? != 0,
        run_status: row.get(6)?,
        shopcodes: serde_json::from_str(&shopcodes_json).unwrap_or_default(),
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(progress)
}

pub fn save_daily_task(db: &DbContext, progress: &DailyTask) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  let shopcodes_json = serde_json::to_string(&progress.shopcodes)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  conn.execute("INSERT OR REPLACE INTO daily_tasks (task_id, date, target_count, completed_count, is_locked, shopcodes_json, run_status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![progress.task_id, progress.date, progress.target_count as i64, progress.completed_count as i64, if progress.is_locked { 1_i64 } else { 0_i64 }, shopcodes_json, progress.run_status]).map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn update_daily_task_run_status(
  db: &DbContext,
  task_id: &str,
  date: &str,
  run_status: &str,
) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  conn
    .execute(
      "UPDATE daily_tasks SET run_status = ?1 WHERE task_id = ?2 AND date = ?3",
      params![run_status, task_id, date],
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
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
  conn.execute("INSERT INTO task_results (task_id, timestamp_micros, index_num, open_id, shop_code, province, city, http_status, response_text, outcome, rtn_msg, read_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![task_id, now, result.index as i64, result.open_id, result.shop_code, result.province, result.city, result.http_status, result.response_text, result.outcome as i32, result.rtn_msg, result.read_id]).map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
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
  let mut stmt = conn.prepare(
    "SELECT
       strftime('%Y-%m-%d %H:%M:%S', tr.timestamp_micros / 1000000.0, 'unixepoch', 'localtime') AS executed_date,
       tr.index_num,
       tr.open_id,
       tr.shop_code,
       tr.province,
       tr.city,
       tr.http_status,
       tr.response_text,
       tr.outcome,
       tr.rtn_msg,
       tr.read_id
     FROM task_results tr
     JOIN monthly_tasks mt ON mt.id = tr.task_id
     LEFT JOIN shops s ON s.shop_code = tr.shop_code
     WHERE tr.task_id = ?1
       AND (
         s.shop_type IS NULL
         OR (mt.task_type = 'Avene' AND s.shop_type IN (?2, ?3))
         OR (mt.task_type = 'Klorane' AND s.shop_type IN (?4, ?3))
       )
     ORDER BY tr.timestamp_micros DESC",
  )
  .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let results = stmt
    .query_map(
      params![
        task_id,
        SHOP_TYPE_AVENE as i64,
        SHOP_TYPE_AVENE_KLORANE as i64,
        SHOP_TYPE_KLORANE as i64
      ],
      |row| {
        let response_text: Option<String> = row.get(7)?;
        let stored_rtn_msg: Option<String> = row.get(9)?;
        let stored_read_id: Option<String> = row.get(10)?;
        let (parsed_submit_err, parsed_rtn_msg, parsed_read_id) =
          parse_legacy_submit_read_log_fields(response_text.as_deref());
        let outcome = row.get::<_, i32>(8)?.into();

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
          outcome,
        })
      },
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(results)
}

pub fn get_task_result_shop_codes_for_date(
  db: &DbContext,
  task_id: &str,
  date: &str,
) -> Result<std::collections::HashSet<String>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare(
      "SELECT DISTINCT shop_code
       FROM task_results
       WHERE task_id = ?1
         AND date(timestamp_micros / 1000000.0, 'unixepoch', 'localtime') = ?2",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let shop_codes = stmt
    .query_map(params![task_id, date], |row| row.get::<_, String>(0))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<std::collections::HashSet<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(shop_codes)
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
  task_type: Option<&str>,
) -> Result<std::collections::HashSet<String>, AppError> {
  let conn = get_conn(db)?;

  if let Some(task_type) = task_type {
    // Only return open_ids used by tasks of a specific task_type within the month
    let mut stmt = conn
      .prepare("SELECT tr.open_id FROM task_results tr JOIN monthly_tasks mt ON tr.task_id = mt.id WHERE tr.task_id LIKE ?1 AND mt.task_type = ?2")
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    let open_ids = stmt
      .query_map(params![format!("{}%", month_prefix), task_type], |row| {
        row.get(0)
      })
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
      .collect::<Result<std::collections::HashSet<String>, _>>()
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    Ok(open_ids)
  } else {
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
}

pub fn find_monthly_tasks_by_month_fc_course(
  db: &DbContext,
  month_prefix: &str,
  fc_name: &str,
  course_id: &str,
) -> Result<Vec<MonthlyTask>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn.prepare("SELECT id, fc_name, s_manager_id, s_course_id, reading_url, task_type, total_target, target_days, created_at, shopcodes_json, excluded_open_ids_json FROM monthly_tasks WHERE id LIKE ?1 AND fc_name = ?2 AND s_course_id = ?3").map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let tasks = stmt
    .query_map(
      params![format!("{}%", month_prefix), fc_name, course_id],
      |row| {
        let shopcodes_json: String = row.get(9)?;
        let excluded_open_ids_json: String = row.get(10)?;
        Ok(MonthlyTask {
          id: row.get(0)?,
          fc_name: row.get(1)?,
          s_manager_id: row.get(2)?,
          s_course_id: row.get(3)?,
          reading_url: row.get(4)?,
          task_type: row.get(5)?,
          total_target: row.get::<_, i64>(6)? as usize,
          target_days: row.get::<_, i64>(7)? as usize,
          created_at: row.get(8)?,
          shopcodes: serde_json::from_str(&shopcodes_json).unwrap_or_default(),
          excluded_open_ids: serde_json::from_str(&excluded_open_ids_json).unwrap_or_default(),
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
    .prepare("SELECT province, city, shop_code, shop_name, fc, shop_type FROM shops")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let shops = stmt
    .query_map([], |row| {
      Ok(ShopRecord {
        province: row.get(0)?,
        city: row.get(1)?,
        shop_code: row.get(2)?,
        shop_name: row.get(3)?,
        fc: row.get(4)?,
        shop_type: row.get::<_, i64>(5)? as u8,
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
    "Avene" => (eligible_count * 4).div_ceil(5),
    "Klorane" => eligible_count,
    _ => eligible_count,
  };

  Ok(target_count)
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

  use super::*;

  #[test]
  fn import_shops_saves_shop_name_and_delete_all_clears_records() {
    let temp_dir = tempdir().expect("create temp dir");
    let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
    let db = init_db_context(&paths).expect("init db");
    let shops = vec![ShopRecord {
      province: "安徽".to_string(),
      city: "滁州".to_string(),
      shop_code: "8970".to_string(),
      shop_name: "CZ南谯99广场".to_string(),
      fc: Some("汪艳利".to_string()),
      shop_type: SHOP_TYPE_AVENE,
    }];

    let imported = import_shops(&db, &shops).expect("import shops");

    assert_eq!(imported, 1);
    assert_eq!(get_all_shops(&db).expect("get shops"), shops);

    delete_all_shops(&db).expect("delete all shops");

    assert!(get_all_shops(&db).expect("get shops").is_empty());
  }

  #[test]
  fn update_shop_type_by_codes_updates_existing_shops_only() {
    let temp_dir = tempdir().expect("create temp dir");
    let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
    let db = init_db_context(&paths).expect("init db");
    let shops = vec![
      ShopRecord {
        province: "安徽".to_string(),
        city: "滁州".to_string(),
        shop_code: "8970".to_string(),
        shop_name: "CZ南谯99广场".to_string(),
        fc: Some("汪艳利".to_string()),
        shop_type: SHOP_TYPE_AVENE,
      },
      ShopRecord {
        province: "安徽".to_string(),
        city: "合肥".to_string(),
        shop_code: "10113".to_string(),
        shop_name: "HF天鹅湖万达".to_string(),
        fc: Some("汪艳利".to_string()),
        shop_type: SHOP_TYPE_AVENE,
      },
    ];
    import_shops(&db, &shops).expect("import shops");

    let updated = update_shop_type_by_codes(
      &db,
      &["8970".to_string(), "missing".to_string()],
      SHOP_TYPE_AVENE_KLORANE,
    )
    .expect("update shop types");
    let shops = get_all_shops(&db).expect("get shops");

    assert_eq!(updated, 1);
    assert_eq!(
      shops
        .iter()
        .find(|shop| shop.shop_code == "8970")
        .expect("updated shop")
        .shop_type,
      SHOP_TYPE_AVENE_KLORANE
    );
    assert_eq!(
      shops
        .iter()
        .find(|shop| shop.shop_code == "10113")
        .expect("unchanged shop")
        .shop_type,
      SHOP_TYPE_AVENE
    );
  }
}
