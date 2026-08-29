use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate as reading_task;
use reading_task::{DbContext, TaskProgress, TaskRunSummary};

use super::bootstrap::RuntimePaths;
use super::dto::DailyTaskRunSnapshot;
use super::error::CommandError;
use super::utils::resource_error;

#[derive(Debug)]
pub struct RuntimeStateInner {
  pub paths: RuntimePaths,
  pub db: Option<DbContext>,
}

#[derive(Debug, Default)]
pub struct TaskPauseRegistry {
  flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl TaskPauseRegistry {
  pub fn register(&self, task_id: &str) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    if let Ok(mut flags) = self.flags.lock() {
      flags.insert(task_id.to_string(), Arc::clone(&flag));
    }
    flag
  }

  pub fn pause(&self, task_id: &str) -> bool {
    if let Ok(flags) = self.flags.lock()
      && let Some(flag) = flags.get(task_id)
    {
      flag.store(true, Ordering::SeqCst);
      return true;
    }

    false
  }

  pub fn clear(&self, task_id: &str) {
    if let Ok(mut flags) = self.flags.lock() {
      flags.remove(task_id);
    }
  }
}

#[derive(Debug, Default)]
pub struct TaskRunRegistry {
  snapshots: Mutex<HashMap<String, DailyTaskRunSnapshot>>,
}

impl TaskRunRegistry {
  pub fn is_running(&self, task_id: &str) -> bool {
    self
      .snapshots
      .lock()
      .ok()
      .and_then(|snapshots| snapshots.get(task_id).cloned())
      .is_some_and(|snapshot| snapshot.run_state == "running")
  }

  pub fn start(&self, task_id: &str, date: &str) {
    if let Ok(mut snapshots) = self.snapshots.lock() {
      snapshots.insert(
        task_id.to_string(),
        DailyTaskRunSnapshot {
          task_id: task_id.to_string(),
          date: date.to_string(),
          run_state: "running".to_string(),
          processed_count: 0,
          requested_count: 0,
          items: Vec::new(),
          summary: None,
          error: None,
        },
      );
    }
  }

  pub fn record_progress(&self, progress: TaskProgress) {
    let Some(task_id) = progress.task_id else {
      return;
    };

    if let Ok(mut snapshots) = self.snapshots.lock() {
      let snapshot = snapshots
        .entry(task_id.clone())
        .or_insert_with(|| DailyTaskRunSnapshot {
          task_id: task_id.clone(),
          date: String::new(),
          run_state: "running".to_string(),
          processed_count: 0,
          requested_count: 0,
          items: Vec::new(),
          summary: None,
          error: None,
        });
      snapshot.run_state = "running".to_string();
      snapshot.processed_count = progress.processed_count;
      snapshot.requested_count = progress.requested_count;
      snapshot.items.push(progress.latest_item);
      snapshot.error = None;
    }
  }

  pub fn finish_success(&self, task_id: &str, date: &str, summary: TaskRunSummary) {
    if let Ok(mut snapshots) = self.snapshots.lock() {
      snapshots.insert(
        task_id.to_string(),
        DailyTaskRunSnapshot {
          task_id: task_id.to_string(),
          date: date.to_string(),
          run_state: "completed".to_string(),
          processed_count: summary.processed_count,
          requested_count: summary.requested_count,
          items: summary.items.clone(),
          summary: Some(summary),
          error: None,
        },
      );
    }
  }

  pub fn finish_error(&self, task_id: &str, date: &str, error: CommandError) {
    let run_state = if error.category == "paused" {
      "paused"
    } else if error.category == "completed" && error.message.contains("该月度任务已全部完成")
    {
      "monthly-completed"
    } else if error.category == "completed" {
      "completed"
    } else {
      "error"
    };

    if let Ok(mut snapshots) = self.snapshots.lock() {
      let snapshot = snapshots
        .entry(task_id.to_string())
        .or_insert_with(|| DailyTaskRunSnapshot {
          task_id: task_id.to_string(),
          date: date.to_string(),
          run_state: run_state.to_string(),
          processed_count: 0,
          requested_count: 0,
          items: Vec::new(),
          summary: None,
          error: None,
        });
      snapshot.date = date.to_string();
      snapshot.run_state = run_state.to_string();
      snapshot.error = if error.category == "completed" {
        None
      } else {
        Some(error)
      };
    }
  }

  pub fn mark_paused(&self, task_id: &str) -> Option<String> {
    if let Ok(mut snapshots) = self.snapshots.lock()
      && let Some(snapshot) = snapshots.get_mut(task_id)
      && snapshot.run_state == "running"
    {
      snapshot.run_state = "paused".to_string();
      return Some(snapshot.date.clone());
    }
    None
  }

  pub fn snapshots(&self) -> Vec<DailyTaskRunSnapshot> {
    self
      .snapshots
      .lock()
      .map(|snapshots| snapshots.values().cloned().collect())
      .unwrap_or_default()
  }
}

#[derive(Debug, Clone)]
pub struct AppState {
  inner: Arc<Mutex<RuntimeStateInner>>,
  pub pause_registry: Arc<TaskPauseRegistry>,
  pub run_registry: Arc<TaskRunRegistry>,
}

impl AppState {
  pub fn new(paths: RuntimePaths, db: Option<DbContext>) -> Self {
    Self {
      inner: Arc::new(Mutex::new(RuntimeStateInner { paths, db })),
      pause_registry: Arc::new(TaskPauseRegistry::default()),
      run_registry: Arc::new(TaskRunRegistry::default()),
    }
  }

  pub fn snapshot_paths(&self) -> Result<RuntimePaths, CommandError> {
    self
      .inner
      .lock()
      .map(|state| state.paths.clone())
      .map_err(|_| resource_error("运行时路径锁已损坏"))
  }

  pub fn snapshot_db(&self) -> Result<Option<DbContext>, CommandError> {
    self
      .inner
      .lock()
      .map(|state| state.db.clone())
      .map_err(|_| resource_error("运行时路径锁已损坏"))
  }

  pub fn replace_db(&self, db_path: PathBuf, db: DbContext) -> Result<RuntimePaths, CommandError> {
    let mut state = self
      .inner
      .lock()
      .map_err(|_| resource_error("运行时路径锁已损坏"))?;
    state.paths.db_path = Some(db_path);
    state.db = Some(db);
    Ok(state.paths.clone())
  }

  pub fn resolve_db(&self) -> Result<DbContext, CommandError> {
    self
      .snapshot_db()?
      .ok_or_else(|| resource_error("请先在首页配置 SQLite 存储文件路径"))
  }
}
