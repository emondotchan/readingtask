use rusqlite::{OptionalExtension, params};

use super::context::{DbContext, get_conn};
use super::schema::reconcile_daily_task_progress_with_conn;
use crate::core::error::AppError;
use crate::core::model::{DailyTask, MonthlyTask, add_days_to_date};

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

pub fn get_all_monthly_tasks(db: &DbContext) -> Result<Vec<MonthlyTask>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn.prepare("SELECT id, fc_name, s_manager_id, s_course_id, reading_url, task_type, total_target, target_days, created_at, shopcodes_json, excluded_open_ids_json FROM monthly_tasks ORDER BY created_at DESC").map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
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
  let shopcodes_json =
    serde_json::to_string(&task.shopcodes).map_err(|e| AppError::ValidationError(e.to_string()))?;
  let excluded_open_ids_json = serde_json::to_string(&task.excluded_open_ids)
    .map_err(|e| AppError::ValidationError(e.to_string()))?;
  conn.execute(
        "INSERT OR REPLACE INTO monthly_tasks (id, fc_name, s_manager_id, s_course_id, reading_url, task_type, total_target, target_days, created_at, shopcodes_json, excluded_open_ids_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![task.id, task.fc_name, task.s_manager_id, task.s_course_id, task.reading_url, task.task_type, task.total_target as i64, task.target_days as i64, task.created_at, shopcodes_json, excluded_open_ids_json],
    ).map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn delete_monthly_task(db: &DbContext, task_id: &str) -> Result<(), AppError> {
  let mut conn = get_conn(db)?;
  let tx = conn
    .transaction()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  tx.execute(
    "DELETE FROM task_results WHERE task_id = ?1",
    params![task_id],
  )
  .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  tx.execute(
    "DELETE FROM daily_tasks WHERE task_id = ?1",
    params![task_id],
  )
  .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  tx.execute("DELETE FROM monthly_tasks WHERE id = ?1", params![task_id])
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  tx.commit()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn get_daily_task(
  db: &DbContext,
  task_id: &str,
  date: &str,
) -> Result<Option<DailyTask>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare(
      "SELECT task_id, date, target_count, completed_count, is_locked, shopcodes_json, run_status
       FROM daily_tasks
       WHERE task_id = ?1 AND date = ?2",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let mut rows = stmt
    .query_map(params![task_id, date], |row| {
      let shopcodes_json: String = row.get(5)?;
      let shopcodes = serde_json::from_str(&shopcodes_json).unwrap_or_default();
      Ok(DailyTask {
        task_id: row.get(0)?,
        date: row.get(1)?,
        target_count: row.get::<_, i64>(2)? as usize,
        completed_count: row.get::<_, i64>(3)? as usize,
        is_locked: row.get::<_, i64>(4)? != 0,
        shopcodes,
        run_status: row.get(6)?,
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if let Some(row) = rows.next() {
    Ok(Some(row.map_err(|e| {
      AppError::ResourceUnavailableError(e.to_string())
    })?))
  } else {
    Ok(None)
  }
}

pub fn get_all_daily_tasks_for_task(
  db: &DbContext,
  task_id: &str,
) -> Result<Vec<DailyTask>, AppError> {
  let conn = get_conn(db)?;
  reconcile_daily_task_progress_with_conn(&conn, task_id)?;
  let mut stmt = conn
    .prepare(
      "SELECT task_id, date, target_count, completed_count, is_locked, shopcodes_json, run_status
       FROM daily_tasks
       WHERE task_id = ?1
       ORDER BY date ASC",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let rows = stmt
    .query_map(params![task_id], |row| {
      let shopcodes_json: String = row.get(5)?;
      let shopcodes = serde_json::from_str(&shopcodes_json).unwrap_or_default();
      Ok(DailyTask {
        task_id: row.get(0)?,
        date: row.get(1)?,
        target_count: row.get::<_, i64>(2)? as usize,
        completed_count: row.get::<_, i64>(3)? as usize,
        is_locked: row.get::<_, i64>(4)? != 0,
        shopcodes,
        run_status: row.get(6)?,
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  Ok(rows)
}

pub fn get_first_pending_daily_task(
  db: &DbContext,
  task_id: &str,
) -> Result<Option<DailyTask>, AppError> {
  let conn = get_conn(db)?;
  reconcile_daily_task_progress_with_conn(&conn, task_id)?;
  let mut stmt = conn
    .prepare(
      "SELECT task_id, date, target_count, completed_count, is_locked, shopcodes_json, run_status
       FROM daily_tasks
       WHERE task_id = ?1 AND completed_count < target_count
       ORDER BY date ASC
       LIMIT 1",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let mut rows = stmt
    .query_map(params![task_id], |row| {
      let shopcodes_json: String = row.get(5)?;
      let shopcodes = serde_json::from_str(&shopcodes_json).unwrap_or_default();
      Ok(DailyTask {
        task_id: row.get(0)?,
        date: row.get(1)?,
        target_count: row.get::<_, i64>(2)? as usize,
        completed_count: row.get::<_, i64>(3)? as usize,
        is_locked: row.get::<_, i64>(4)? != 0,
        shopcodes,
        run_status: row.get(6)?,
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if let Some(row) = rows.next() {
    Ok(Some(row.map_err(|e| {
      AppError::ResourceUnavailableError(e.to_string())
    })?))
  } else {
    Ok(None)
  }
}

pub fn save_daily_task(db: &DbContext, task: &DailyTask) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  let shopcodes_json =
    serde_json::to_string(&task.shopcodes).map_err(|e| AppError::ValidationError(e.to_string()))?;
  conn.execute(
        "INSERT INTO daily_tasks (task_id, date, target_count, completed_count, is_locked, shopcodes_json, run_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(task_id, date) DO UPDATE SET
             target_count = excluded.target_count,
             completed_count = excluded.completed_count,
             is_locked = excluded.is_locked,
             shopcodes_json = excluded.shopcodes_json,
             run_status = excluded.run_status",
        params![
            task.task_id,
            task.date,
            task.target_count as i64,
            task.completed_count as i64,
            if task.is_locked { 1 } else { 0 },
            shopcodes_json,
            task.run_status,
        ],
    ).map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
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

pub fn pause_running_daily_tasks_for_task(
  db: &DbContext,
  task_id: &str,
) -> Result<usize, AppError> {
  let conn = get_conn(db)?;
  let updated = conn
    .execute(
      "UPDATE daily_tasks
       SET run_status = 'paused'
       WHERE task_id = ?1 AND run_status = 'running'",
      params![task_id],
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(updated)
}

pub fn reschedule_unfinished_daily_tasks(
  db: &DbContext,
  task_id: &str,
  start_date: &str,
) -> Result<Vec<DailyTask>, AppError> {
  let mut conn = get_conn(db)?;
  reconcile_daily_task_progress_with_conn(&conn, task_id)?;
  let tx = conn
    .transaction()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let mut stmt = tx
    .prepare(
      "SELECT task_id, date, target_count, completed_count, is_locked, shopcodes_json, run_status
       FROM daily_tasks
       WHERE task_id = ?1 AND completed_count < target_count
       ORDER BY date ASC",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let unfinished_tasks = stmt
    .query_map(params![task_id], |row| {
      let shopcodes_json: String = row.get(5)?;
      let shopcodes = serde_json::from_str(&shopcodes_json).unwrap_or_default();
      Ok(DailyTask {
        task_id: row.get(0)?,
        date: row.get(1)?,
        target_count: row.get::<_, i64>(2)? as usize,
        completed_count: row.get::<_, i64>(3)? as usize,
        is_locked: row.get::<_, i64>(4)? != 0,
        shopcodes,
        run_status: row.get(6)?,
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  drop(stmt);

  if unfinished_tasks.is_empty() {
    return get_all_daily_tasks_for_task(db, task_id);
  }

  let mut current_date = start_date.to_string();

  for task in &unfinished_tasks {
    tx.execute(
      "DELETE FROM daily_tasks WHERE task_id = ?1 AND date = ?2",
      params![task.task_id, task.date],
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  }

  for mut task in unfinished_tasks {
    while tx
      .query_row(
        "SELECT 1 FROM daily_tasks WHERE task_id = ?1 AND date = ?2",
        params![task.task_id, current_date],
        |_| Ok(()),
      )
      .optional()
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
      .is_some()
    {
      current_date = add_days_to_date(&current_date, 1);
    }

    task.date = current_date.clone();
    task.is_locked = false;
    task.run_status = "not_started".to_string();
    let shopcodes_json = serde_json::to_string(&task.shopcodes)
      .map_err(|e| AppError::ValidationError(e.to_string()))?;

    tx.execute(
      "INSERT INTO daily_tasks (task_id, date, target_count, completed_count, is_locked, shopcodes_json, run_status)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
      params![
        task.task_id,
        task.date,
        task.target_count as i64,
        task.completed_count as i64,
        if task.is_locked { 1 } else { 0 },
        shopcodes_json,
        task.run_status,
      ],
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

    current_date = add_days_to_date(&current_date, 1);
  }

  tx.commit()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  get_all_daily_tasks_for_task(db, task_id)
}
