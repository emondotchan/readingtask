use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use rocksdb::{DB, Options};

use super::error::AppError;
use super::model::{AppPaths, OpenIdRecord, ShopRecord};

fn log_db(level: &str, message: impl AsRef<str>) {
  eprintln!("[reading_task::db][{level}] {}", message.as_ref());
}

fn db_cache() -> &'static Mutex<HashMap<PathBuf, Arc<DB>>> {
  static DB_CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<DB>>>> = OnceLock::new();
  DB_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn open_db(paths: &AppPaths, create_if_missing: bool) -> Result<Arc<DB>, AppError> {
  if let Some(db) = db_cache()
    .lock()
    .map_err(|_| AppError::ResourceUnavailableError("数据库缓存锁已损坏".to_string()))?
    .get(&paths.db_path)
    .cloned()
  {
    log_db(
      "INFO",
      format!("reusing cached rocksdb path={}", paths.db_path.display()),
    );
    return Ok(db);
  }

  log_db(
    "INFO",
    format!(
      "opening rocksdb path={} create_if_missing={}",
      paths.db_path.display(),
      create_if_missing
    ),
  );
  let mut opts = Options::default();
  opts.create_if_missing(create_if_missing);
  let db = Arc::new(DB::open(&opts, &paths.db_path).map_err(|e| {
    log_db(
      "ERROR",
      format!(
        "failed to open rocksdb path={} error={e}",
        paths.db_path.display()
      ),
    );
    AppError::ResourceUnavailableError(format!("无法打开 RocksDB: {}", e))
  })?);

  db_cache()
    .lock()
    .map_err(|_| AppError::ResourceUnavailableError("数据库缓存锁已损坏".to_string()))?
    .insert(paths.db_path.clone(), Arc::clone(&db));

  Ok(db)
}

pub(crate) fn open_existing_db(paths: &AppPaths) -> Result<Arc<DB>, AppError> {
  open_db(paths, false)
}

pub fn init_db(paths: &AppPaths) -> Result<(), AppError> {
  log_db(
    "INFO",
    format!(
      "initializing db config_dir={} db_path={}",
      paths.config_dir.display(),
      paths.db_path.display()
    ),
  );
  if paths.db_path.exists() {
    let db = open_existing_db(paths)?;
    if is_initialized(&db)? {
      log_db("INFO", "db already initialized");
      return Ok(());
    }
  }

  let db = open_db(paths, true)?;
  if is_initialized(&db)? {
    return Ok(());
  }

  bootstrap_db_from_toml(paths, &db)?;
  seed_default_fc(&db)?;
  db.put(b"sys:initialized", b"true")
    .map_err(|e| AppError::ResourceUnavailableError(format!("数据库写入错误: {}", e)))?;
  log_db("INFO", "db initialization completed");

  Ok(())
}

pub fn get_all_open_ids(paths: &AppPaths) -> Result<Vec<String>, AppError> {
  Ok(
    get_all_open_id_records(paths)?
      .into_iter()
      .map(|record| record.open_id)
      .collect(),
  )
}

pub fn get_all_open_id_records(paths: &AppPaths) -> Result<Vec<OpenIdRecord>, AppError> {
  let db = open_existing_db(paths)?;
  let mut open_ids = Vec::new();
  let iter = db.iterator(rocksdb::IteratorMode::Start);
  for item in iter {
    let (key, value) = item.map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    let key_str = String::from_utf8_lossy(&key);
    if key_str.starts_with("openid:") {
      open_ids.push(decode_open_id_record(&key_str, &value)?);
    }
  }
  open_ids.sort_by(|a, b| {
    a.manager_id
      .cmp(&b.manager_id)
      .then_with(|| a.open_id.cmp(&b.open_id))
  });
  Ok(open_ids)
}

pub fn add_open_id(paths: &AppPaths, record: &OpenIdRecord) -> Result<(), AppError> {
  let db = open_existing_db(paths)?;
  let key = format!("openid:{}", record.open_id);
  let value = serde_json::to_vec(record).unwrap();
  db.put(key.as_bytes(), value)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn delete_open_id(paths: &AppPaths, open_id: &str) -> Result<(), AppError> {
  let db = open_existing_db(paths)?;
  let key = format!("openid:{}", open_id);
  db.delete(key.as_bytes())
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn import_open_ids_csv(paths: &AppPaths, csv_text: &str) -> Result<usize, AppError> {
  let records = parse_open_id_csv(csv_text)?;
  for record in &records {
    add_open_id(paths, record)?;
  }
  Ok(records.len())
}

pub fn get_all_shops(paths: &AppPaths) -> Result<Vec<ShopRecord>, AppError> {
  let db = open_existing_db(paths)?;
  let mut shops = Vec::new();
  let iter = db.iterator(rocksdb::IteratorMode::Start);
  for item in iter {
    let (key, value) = item.map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    let key_str = String::from_utf8_lossy(&key);
    if key_str.starts_with("shop:") {
      let shop: ShopRecord = serde_json::from_slice(&value)
        .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
      shops.push(shop);
    }
  }
  Ok(shops)
}

pub fn add_or_update_shop(paths: &AppPaths, shop: &ShopRecord) -> Result<(), AppError> {
  let db = open_existing_db(paths)?;
  let key = format!("shop:{}", shop.shop_code);
  let value = serde_json::to_vec(shop).unwrap();
  db.put(key.as_bytes(), &value)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn delete_shop(paths: &AppPaths, shop_code: &str) -> Result<(), AppError> {
  let db = open_existing_db(paths)?;
  let key = format!("shop:{}", shop_code);
  db.delete(key.as_bytes())
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn get_all_fcs(paths: &AppPaths) -> Result<Vec<super::model::FcRecord>, AppError> {
  log_db(
    "INFO",
    format!("loading fc list from db_path={}", paths.db_path.display()),
  );
  let db = open_existing_db(paths)?;
  let mut fcs = Vec::new();
  let iter = db.iterator(rocksdb::IteratorMode::Start);
  for item in iter {
    let (key, value) = item.map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    let key_str = String::from_utf8_lossy(&key);
    if key_str.starts_with("fc:") {
      let fc: super::model::FcRecord = serde_json::from_slice(&value).map_err(|e| {
        log_db(
          "ERROR",
          format!("failed to decode fc record key={key_str} error={e}"),
        );
        AppError::ResourceUnavailableError(e.to_string())
      })?;
      fcs.push(fc);
    }
  }
  log_db("INFO", format!("loaded {} fc records", fcs.len()));
  Ok(fcs)
}

pub fn add_or_update_fc(paths: &AppPaths, fc: &super::model::FcRecord) -> Result<(), AppError> {
  let db = open_existing_db(paths)?;
  let key = format!("fc:{}", fc.name);
  let value = serde_json::to_vec(fc).unwrap();
  db.put(key.as_bytes(), &value)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn delete_fc(paths: &AppPaths, name: &str) -> Result<(), AppError> {
  let db = open_existing_db(paths)?;
  let key = format!("fc:{}", name);
  db.delete(key.as_bytes())
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn get_all_monthly_tasks(paths: &AppPaths) -> Result<Vec<super::model::MonthlyTask>, AppError> {
  log_db(
    "INFO",
    format!(
      "loading monthly tasks from db_path={}",
      paths.db_path.display()
    ),
  );
  let db = open_existing_db(paths)?;
  let mut tasks = Vec::new();
  let iter = db.iterator(rocksdb::IteratorMode::Start);
  for item in iter {
    let (key, value) = item.map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    let key_str = String::from_utf8_lossy(&key);
    if key_str.starts_with("task:") {
      let task: super::model::MonthlyTask = serde_json::from_slice(&value).map_err(|e| {
        log_db(
          "ERROR",
          format!("failed to decode monthly task key={key_str} error={e}"),
        );
        AppError::ResourceUnavailableError(e.to_string())
      })?;
      tasks.push(task);
    }
  }
  log_db("INFO", format!("loaded {} monthly tasks", tasks.len()));
  Ok(tasks)
}

pub fn add_monthly_task(
  paths: &AppPaths,
  task: &super::model::MonthlyTask,
) -> Result<(), AppError> {
  let db = open_existing_db(paths)?;
  let key = format!("task:{}", task.id);
  if db
    .get(key.as_bytes())
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .is_some()
  {
    return Err(AppError::ValidationError(format!(
      "月度任务已存在: {}",
      task.id
    )));
  }
  let value = serde_json::to_vec(task).unwrap();
  db.put(key.as_bytes(), &value)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn find_monthly_tasks_by_month_fc_course(
  paths: &AppPaths,
  month_prefix: &str,
  fc_name: &str,
  course_id: &str,
) -> Result<Vec<super::model::MonthlyTask>, AppError> {
  let db = open_existing_db(paths)?;
  let mut tasks = Vec::new();
  let task_prefix = format!("task:{month_prefix}:");
  let iter = db.iterator(rocksdb::IteratorMode::Start);
  for item in iter {
    let (key, value) = item.map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    let key_str = String::from_utf8_lossy(&key);
    if key_str.starts_with(&task_prefix) {
      let task: super::model::MonthlyTask = serde_json::from_slice(&value)
        .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
      if task.fc_name == fc_name && task.s_course_id == course_id {
        tasks.push(task);
      }
    }
  }
  Ok(tasks)
}

pub fn delete_monthly_task(paths: &AppPaths, id: &str) -> Result<(), AppError> {
  let db = open_existing_db(paths)?;
  let key = format!("task:{}", id);
  db.delete(key.as_bytes())
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn get_daily_progress(
  paths: &AppPaths,
  task_id: &str,
  date: &str,
) -> Result<Option<super::model::DailyProgress>, AppError> {
  let db = open_existing_db(paths)?;
  let key = format!("progress:{}:{}", task_id, date);
  if let Some(value) = db
    .get(key.as_bytes())
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
  {
    let progress: super::model::DailyProgress = serde_json::from_slice(&value)
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    Ok(Some(progress))
  } else {
    Ok(None)
  }
}

pub fn get_all_progress_for_task(
  paths: &AppPaths,
  task_id: &str,
) -> Result<Vec<super::model::DailyProgress>, AppError> {
  let db = open_existing_db(paths)?;
  let mut progresses = Vec::new();
  let prefix = format!("progress:{}:", task_id);
  let iter = db.iterator(rocksdb::IteratorMode::Start);
  for item in iter {
    let (key, value) = item.map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    let key_str = String::from_utf8_lossy(&key);
    if key_str.starts_with(&prefix) {
      let progress: super::model::DailyProgress = serde_json::from_slice(&value)
        .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
      progresses.push(progress);
    }
  }
  Ok(progresses)
}

pub fn save_daily_progress(
  paths: &AppPaths,
  progress: &super::model::DailyProgress,
) -> Result<(), AppError> {
  let db = open_existing_db(paths)?;
  let key = format!("progress:{}:{}", progress.task_id, progress.date);
  let value = serde_json::to_vec(progress).unwrap();
  db.put(key.as_bytes(), &value)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn save_task_result(
  paths: &AppPaths,
  task_id: &str,
  result: &super::model::TaskItemResult,
) -> Result<(), AppError> {
  let db = open_existing_db(paths)?;
  // Use a composite key to allow sorting by index or timestamp.
  // format: result:{task_id}:{timestamp_micros}
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_micros();
  let key = format!("result:{}:{}", task_id, now);
  let value = serde_json::to_vec(result).unwrap();
  db.put(key.as_bytes(), &value)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn save_task_results(
  paths: &AppPaths,
  task_id: &str,
  results: &[super::model::TaskItemResult],
) -> Result<(), AppError> {
  for result in results {
    save_task_result(paths, task_id, result)?;
  }
  Ok(())
}

pub fn get_task_results(
  paths: &AppPaths,
  task_id: &str,
) -> Result<Vec<super::model::TaskItemResult>, AppError> {
  let db = open_existing_db(paths)?;
  let mut results = Vec::new();
  let prefix = format!("result:{}:", task_id);
  let iter = db.iterator(rocksdb::IteratorMode::Start);
  for item in iter {
    let (key, value) = item.map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    let key_str = String::from_utf8_lossy(&key);
    if key_str.starts_with(&prefix) {
      let result: super::model::TaskItemResult = serde_json::from_slice(&value)
        .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
      results.push(result);
    }
  }
  // Reverse to show latest first
  results.reverse();
  Ok(results)
}

pub fn get_used_open_ids_for_month(
  paths: &AppPaths,
  month_prefix: &str,
) -> Result<std::collections::HashSet<String>, AppError> {
  let db = open_existing_db(paths)?;
  let mut open_ids = std::collections::HashSet::new();
  let result_prefix = b"result:";
  let task_prefix = format!("{month_prefix}:");
  let iter = db.iterator(rocksdb::IteratorMode::Start);
  for item in iter {
    let (key, value) = item.map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    if !key.starts_with(result_prefix) {
      continue;
    }

    let key_str = String::from_utf8_lossy(&key);
    let Some(rest) = key_str.strip_prefix("result:") else {
      continue;
    };
    let Some((task_id, _timestamp)) = rest.rsplit_once(':') else {
      continue;
    };
    if !task_id.starts_with(&task_prefix) {
      continue;
    }

    let result: super::model::TaskItemResult = serde_json::from_slice(&value)
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    open_ids.insert(result.open_id);
  }

  Ok(open_ids)
}

fn is_initialized(db: &DB) -> Result<bool, AppError> {
  db.get(b"sys:initialized")
    .map(|value| value.is_some())
    .map_err(|e| AppError::ResourceUnavailableError(format!("数据库读取错误: {}", e)))
}

fn bootstrap_db_from_toml(paths: &AppPaths, db: &DB) -> Result<(), AppError> {
  let open_ids_path = paths.config_dir.join("open_ids.toml");
  let shops_path = paths.config_dir.join("shop.toml");

  if open_ids_path.exists() && shops_path.exists() {
    let toml_open_ids = super::loader::load_open_ids_from_toml(&open_ids_path)?;
    let toml_shops = super::loader::load_shops_from_toml(&shops_path)?;

    for open_id in toml_open_ids {
      let record = OpenIdRecord {
        manager_id: String::new(),
        open_id,
      };
      let key = format!("openid:{}", record.open_id);
      db.put(key.as_bytes(), serde_json::to_vec(&record).unwrap())
        .map_err(|e| AppError::ResourceUnavailableError(format!("数据库写入错误: {}", e)))?;
    }

    for shop in toml_shops {
      let key = format!("shop:{}", shop.shop_code);
      let value = serde_json::to_vec(&shop).unwrap();
      db.put(key.as_bytes(), &value)
        .map_err(|e| AppError::ResourceUnavailableError(format!("数据库写入错误: {}", e)))?;
    }
  }

  Ok(())
}

fn decode_open_id_record(key_str: &str, value: &[u8]) -> Result<OpenIdRecord, AppError> {
  match serde_json::from_slice::<OpenIdRecord>(value) {
    Ok(record) => Ok(record),
    Err(_) => Ok(OpenIdRecord {
      manager_id: String::new(),
      open_id: key_str
        .strip_prefix("openid:")
        .unwrap_or_default()
        .to_string(),
    }),
  }
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
      .map(|column| column.trim().trim_matches('"'))
      .collect::<Vec<_>>();

    if index == 0
      && columns.len() >= 2
      && normalize_csv_header(columns[0]).contains("manager")
      && normalize_csv_header(columns[1]).contains("openid")
    {
      continue;
    }

    if columns.len() < 2 {
      return Err(AppError::ValidationError(format!(
        "CSV 第 {} 行格式错误，至少需要两列：ManagerID, OpenID",
        index + 1
      )));
    }

    let manager_id = columns[0].trim();
    let open_id = columns[1].trim();

    if manager_id.is_empty() || open_id.is_empty() {
      return Err(AppError::ValidationError(format!(
        "CSV 第 {} 行存在空值，ManagerID 和 OpenID 都不能为空",
        index + 1
      )));
    }

    records.push(OpenIdRecord {
      manager_id: manager_id.to_string(),
      open_id: open_id.to_string(),
    });
  }

  if records.is_empty() {
    return Err(AppError::ValidationError(
      "CSV 中没有可导入的 OpenID 记录".to_string(),
    ));
  }

  Ok(records)
}

fn normalize_csv_header(header: &str) -> String {
  header
    .chars()
    .filter(|ch| ch.is_ascii_alphanumeric())
    .collect::<String>()
    .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
  use std::fs;

  use tempfile::TempDir;

  use super::*;

  #[test]
  fn import_open_ids_csv_accepts_header_and_persists_manager_id() {
    let (_temp_dir, paths) = create_paths();
    init_db(&paths).expect("init db");

    let imported =
      import_open_ids_csv(&paths, "ManagerID,OpenID\n11318,openid-a\n11319,openid-b\n")
        .expect("import csv");

    assert_eq!(imported, 2);
    let records = get_all_open_id_records(&paths).expect("get open id records");
    assert!(
      records
        .iter()
        .any(|record| record.manager_id == "11318" && record.open_id == "openid-a")
    );
    assert!(
      records
        .iter()
        .any(|record| record.manager_id == "11319" && record.open_id == "openid-b")
    );
  }

  #[test]
  fn get_all_open_id_records_supports_legacy_key_only_records() {
    let (_temp_dir, paths) = create_paths();
    init_db(&paths).expect("init db");
    let db = open_existing_db(&paths).expect("open db");
    db.put(b"openid:legacy-openid", b"1")
      .expect("write legacy openid");

    let records = get_all_open_id_records(&paths).expect("get open id records");
    let legacy = records
      .into_iter()
      .find(|record| record.open_id == "legacy-openid")
      .expect("find legacy openid");

    assert_eq!(legacy.manager_id, "");
  }

  fn create_paths() -> (TempDir, AppPaths) {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config_dir = temp_dir.path().join("config");
    let db_path = temp_dir.path().join(".reading.db");
    fs::create_dir_all(&config_dir).expect("create config dir");
    fs::write(
      config_dir.join("open_ids.toml"),
      r#"openids = ["seed-openid"]"#,
    )
    .expect("write open_ids");
    fs::write(
      config_dir.join("shop.toml"),
      r#"
[[shops]]
Province = "安徽"
City = "安庆"
ShopCode = "100"
FC = "fc-a"
"#,
    )
    .expect("write shop");
    fs::write(
      config_dir.join("province.toml"),
      r#"
[[provinces]]
ProvinceName = "安徽"
CityName = "安庆"
"#,
    )
    .expect("write province");

    (temp_dir, AppPaths::new_with_db_path(config_dir, db_path))
  }
}

fn seed_default_fc(db: &DB) -> Result<(), AppError> {
  let default_fc = super::model::FcRecord {
    name: "周凡琪".to_string(),
    manager_id: "11318".to_string(),
  };

  db.put(
    format!("fc:{}", default_fc.name).as_bytes(),
    &serde_json::to_vec(&default_fc).unwrap(),
  )
  .map_err(|e| AppError::ResourceUnavailableError(format!("数据库写入错误: {}", e)))
}
