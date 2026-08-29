use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;

use crate as reading_task;
use reading_task::{DailyTask, DbContext, TaskRunSummary};

use crate::web::dto::{
  BatchRunDailyTasksInput, BatchRunDailyTasksResponse, DailyTaskQuery, DailyTaskRunSnapshot,
};
use crate::web::error::CommandError;
use crate::web::state::AppState;
use crate::web::utils::{
  log_command, resource_error, run_blocking_db, run_blocking_fn, validation_error,
};

pub async fn get_daily_task(
  State(state): State<AppState>,
  Path(task_id): Path<String>,
  Query(query): Query<DailyTaskQuery>,
) -> Result<Json<Option<DailyTask>>, CommandError> {
  let db = state.resolve_db()?;
  let progress =
    run_blocking_db(move || reading_task::get_daily_task(&db, &task_id, &query.date)).await?;
  Ok(Json(progress))
}

pub async fn get_pending_daily_task(
  State(state): State<AppState>,
  Path(task_id): Path<String>,
) -> Result<Json<Option<DailyTask>>, CommandError> {
  let db = state.resolve_db()?;
  let pending =
    run_blocking_db(move || reading_task::get_first_pending_daily_task(&db, &task_id)).await?;
  Ok(Json(pending))
}

pub async fn get_task_daily_tasks(
  State(state): State<AppState>,
  Path(task_id): Path<String>,
) -> Result<Json<Vec<DailyTask>>, CommandError> {
  let db = state.resolve_db()?;
  let daily_tasks =
    run_blocking_db(move || reading_task::get_all_daily_tasks_for_task(&db, &task_id)).await?;
  Ok(Json(daily_tasks))
}

pub async fn save_daily_task(
  State(state): State<AppState>,
  Path(task_id): Path<String>,
  Json(task): Json<DailyTask>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  run_blocking_fn(move || {
    let existing = reading_task::get_daily_task(&db, &task_id, &task.date)
      .map_err(CommandError::from)?
      .ok_or_else(|| resource_error("未找到对应的每日任务进度"))?;

    if existing.completed_count >= existing.target_count {
      return Err(validation_error("已完成的每日任务不可编辑"));
    }

    let updated = DailyTask {
      shopcodes: task.shopcodes,
      ..existing
    };

    reading_task::save_daily_task(&db, &updated).map_err(CommandError::from)?;
    Ok(())
  })
  .await?;
  Ok(StatusCode::NO_CONTENT)
}

pub async fn run_daily_task(
  State(state): State<AppState>,
  Path(task_id): Path<String>,
  Query(query): Query<DailyTaskQuery>,
) -> Result<Json<TaskRunSummary>, CommandError> {
  let db = state.resolve_db()?;
  let target_date = if query.date == "auto" || query.date.trim().is_empty() {
    chrono::Local::now().format("%Y-%m-%d").to_string()
  } else {
    query.date
  };

  let pause_flag = state.pause_registry.register(&task_id);
  state.run_registry.start(&task_id, &target_date);
  let run_registry = Arc::clone(&state.run_registry);
  let summary = reading_task::run_daily_task_with_progress_controlled(
    &db,
    &task_id,
    &target_date,
    move |progress| {
      run_registry.record_progress(progress);
    },
    move || pause_flag.load(Ordering::SeqCst),
  )
  .await;
  state.pause_registry.clear(&task_id);
  let summary = summary.map_err(|e| {
    let command_error = CommandError::from(e);
    if command_error.category == "completed" {
      log_command(
        log::Level::Warn,
        "run_daily_task",
        format!("任务无需继续执行: {}", command_error.message),
      );
    } else {
      log_command(
        log::Level::Error,
        "run_daily_task",
        format!("执行任务失败: {}", command_error.message),
      );
    }
    state
      .run_registry
      .finish_error(&task_id, &target_date, command_error.clone());
    command_error
  })?;

  state
    .run_registry
    .finish_success(&task_id, &target_date, summary.clone());
  log_command(log::Level::Debug, "run_daily_task", "执行日常任务成功");
  Ok(Json(summary))
}

pub async fn batch_run_daily_tasks(
  State(state): State<AppState>,
  Json(input): Json<BatchRunDailyTasksInput>,
) -> Result<Json<BatchRunDailyTasksResponse>, CommandError> {
  let db = state.resolve_db()?;
  let mut accepted_count = 0_usize;
  let mut skipped_count = 0_usize;
  let today_str = chrono::Local::now().format("%Y-%m-%d").to_string();

  for task_id in input.task_ids {
    if task_id.trim().is_empty() || state.run_registry.is_running(&task_id) {
      skipped_count += 1;
      continue;
    }

    let date = if input.date == "auto" || input.date.trim().is_empty() {
      today_str.clone()
    } else {
      input.date.clone()
    };

    accepted_count += 1;
    state.run_registry.start(&task_id, &date);
    let state_for_task = state.clone();
    let db_for_task = db.clone();
    tokio::spawn(async move {
      run_daily_task_background(state_for_task, db_for_task, task_id, date).await;
    });
  }

  Ok(Json(BatchRunDailyTasksResponse {
    accepted_count,
    skipped_count,
  }))
}

pub async fn run_daily_task_background(
  state: AppState,
  db: DbContext,
  task_id: String,
  date: String,
) {
  let pause_flag = state.pause_registry.register(&task_id);
  let run_registry = Arc::clone(&state.run_registry);
  let result = reading_task::run_daily_task_with_progress_controlled(
    &db,
    &task_id,
    &date,
    move |progress| {
      run_registry.record_progress(progress);
    },
    move || pause_flag.load(Ordering::SeqCst),
  )
  .await;
  state.pause_registry.clear(&task_id);

  match result {
    Ok(summary) => {
      state
        .run_registry
        .finish_success(&task_id, &date, summary.clone());
      log_command(
        log::Level::Debug,
        "batch_run_daily_tasks",
        format!("任务 {} 执行完成", task_id),
      );
    }
    Err(error) => {
      let command_error = CommandError::from(error);
      if command_error.category == "completed" {
        log_command(
          log::Level::Warn,
          "batch_run_daily_tasks",
          format!("任务 {} 无需继续执行: {}", task_id, command_error.message),
        );
      } else {
        log_command(
          log::Level::Error,
          "batch_run_daily_tasks",
          format!("任务 {} 执行失败: {}", task_id, command_error.message),
        );
      }
      state
        .run_registry
        .finish_error(&task_id, &date, command_error);
    }
  }
}

pub async fn get_daily_task_run_status(
  State(state): State<AppState>,
) -> Json<Vec<DailyTaskRunSnapshot>> {
  Json(state.run_registry.snapshots())
}

pub async fn pause_daily_task(
  State(state): State<AppState>,
  Path(task_id): Path<String>,
) -> Result<Json<bool>, CommandError> {
  let signaled_running_task = state.pause_registry.pause(&task_id);
  let marked_running_snapshot = state.run_registry.mark_paused(&task_id);
  let marked_running_snapshot_exists = marked_running_snapshot.is_some();

  let db = state.resolve_db()?;
  let task_id_clone = task_id.clone();
  let paused_stale_rows = run_blocking_fn(move || {
    if let Some(date) = marked_running_snapshot.as_deref()
      && let Err(error) =
        reading_task::update_daily_task_run_status(&db, &task_id_clone, date, "paused")
    {
      log_command(
        log::Level::Error,
        "pause_daily_task",
        format!("更新任务暂停状态失败: {}", error),
      );
    }

    // Always ensure any running daily task rows in the database are paused.
    // This guarantees consistency and immediate responsiveness in the UI.
    let updated = reading_task::pause_running_daily_tasks_for_task(&db, &task_id_clone)
      .map_err(CommandError::from)?;
    Ok(updated > 0)
  })
  .await?;

  let paused = signaled_running_task || marked_running_snapshot_exists || paused_stale_rows;
  Ok(Json(paused))
}
