use std::path::PathBuf;
use std::sync::Arc;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::core::error::AppError;
use crate::core::model::AppPaths;

pub(crate) type DbPool = Pool<SqliteConnectionManager>;

#[derive(Debug, Clone)]
pub struct DbContext {
  db_path: PathBuf,
  pub(crate) pool: Arc<DbPool>,
}

impl DbContext {
  pub fn from_paths(paths: &AppPaths) -> Result<Self, AppError> {
    Self::new(paths.db_path.clone())
  }

  pub fn new(db_path: PathBuf) -> Result<Self, AppError> {
    let manager = SqliteConnectionManager::file(&db_path).with_init(|conn| {
      conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = -64000;
         PRAGMA temp_store = MEMORY;
         PRAGMA busy_timeout = 5000;",
      )
    });
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
  super::schema::init_db(&db)?;
  Ok(db)
}

pub(crate) fn get_conn(
  db: &DbContext,
) -> Result<r2d2::PooledConnection<SqliteConnectionManager>, AppError> {
  db.pool
    .get()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))
}
