use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const BUNDLED_DB_BYTES: &[u8] = include_bytes!("../../resources/bundled.reading.sqlite");
const SQLITE_SETTINGS_FILE: &str = ".reading-task/runtime-settings.json";
const DEFAULT_DB_FILE: &str = ".reading.sqlite";

#[derive(Debug, Clone)]
pub struct RuntimePaths {
  pub sqlite_settings_path: PathBuf,
  pub db_path: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedRuntimeSettings {
  sqlite_path: String,
}

pub fn initialize() -> Result<RuntimePaths, Box<dyn std::error::Error>> {
  let runtime_dir = runtime_dir()?;
  let sqlite_settings_path = runtime_dir.join(SQLITE_SETTINGS_FILE);
  let db_path = match load_sqlite_path(&sqlite_settings_path)? {
    Some(path) => Some(path),
    None => {
      let default_db_path = home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DEFAULT_DB_FILE);
      copy_bundled_db_if_needed(&default_db_path)?;
      save_sqlite_path(&sqlite_settings_path, &default_db_path)?;
      Some(default_db_path)
    }
  };

  if let Some(path) = &db_path {
    copy_bundled_db_if_needed(path)?;
  }

  Ok(RuntimePaths {
    sqlite_settings_path,
    db_path,
  })
}

pub fn save_sqlite_path(
  sqlite_settings_path: &Path,
  db_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
  if let Some(parent) = sqlite_settings_path.parent() {
    fs::create_dir_all(parent)?;
  }

  let settings = PersistedRuntimeSettings {
    sqlite_path: db_path.to_string_lossy().to_string(),
  };
  fs::write(sqlite_settings_path, serde_json::to_vec_pretty(&settings)?)?;
  Ok(())
}

fn load_sqlite_path(
  sqlite_settings_path: &Path,
) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
  if !sqlite_settings_path.is_file() {
    return Ok(None);
  }

  let content = fs::read(sqlite_settings_path)?;
  let settings: PersistedRuntimeSettings = serde_json::from_slice(&content)?;
  let trimmed = settings.sqlite_path.trim();
  if trimmed.is_empty() {
    return Ok(None);
  }

  Ok(Some(PathBuf::from(trimmed)))
}

fn runtime_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
  let home = home_dir().ok_or("未找到用户主目录")?;
  Ok(home.join(".reading-task"))
}

fn home_dir() -> Option<PathBuf> {
  std::env::var_os("HOME")
    .map(PathBuf::from)
    .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

fn copy_bundled_db_if_needed(target_db_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
  if target_db_path.is_file() {
    return Ok(());
  }

  if let Some(parent) = target_db_path.parent() {
    fs::create_dir_all(parent)?;
  }

  fs::write(target_db_path, BUNDLED_DB_BYTES)?;
  Ok(())
}
