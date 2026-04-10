use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri::path::BaseDirectory;

const BUNDLED_DB_FILE: &str = "resources/bundled.reading.sqlite";
const SQLITE_SETTINGS_FILE: &str = "runtime-settings.json";

#[derive(Debug, Clone)]
pub struct RuntimePaths {
  pub sqlite_settings_path: PathBuf,
  pub db_path: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedRuntimeSettings {
  sqlite_path: String,
}

pub fn initialize(app: &tauri::App) -> Result<RuntimePaths, Box<dyn std::error::Error>> {
  let app_data_dir = app.path().app_data_dir()?;
  let sqlite_settings_path = app_data_dir.join(SQLITE_SETTINGS_FILE);
  let db_path = match load_sqlite_path(&sqlite_settings_path)? {
    Some(path) => Some(path),
    None => {
      let default_db_path = app_data_dir.join("reading.sqlite");
      copy_bundled_db_if_needed(&app.handle(), &default_db_path)?;
      save_sqlite_path(&sqlite_settings_path, &default_db_path)?;
      Some(default_db_path)
    }
  };

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

fn bundled_db_path(app: &tauri::AppHandle) -> Result<PathBuf, Box<dyn std::error::Error>> {
  Ok(
    app
      .path()
      .resolve(BUNDLED_DB_FILE, BaseDirectory::Resource)?,
  )
}

fn copy_bundled_db_if_needed(
  app: &tauri::AppHandle,
  target_db_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
  if target_db_path.is_file() {
    return Ok(());
  }

  if let Some(parent) = target_db_path.parent() {
    fs::create_dir_all(parent)?;
  }

  let source = bundled_db_path(app)?;
  if !source.is_file() {
    return Err(format!("未找到内置 SQLite 模板 {}", source.display()).into());
  }

  fs::copy(source, target_db_path)?;
  Ok(())
}
