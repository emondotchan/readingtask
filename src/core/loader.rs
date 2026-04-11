use super::db::DbContext;
use super::error::AppError;
use super::model::{OpenIdRecord, ShopRecord};

#[derive(Debug)]
pub(crate) struct RuntimeData {
  pub(crate) open_ids: Vec<OpenIdRecord>,
  pub(crate) shops: Vec<ShopRecord>,
}

pub(crate) fn load_runtime_data(db: &DbContext) -> Result<RuntimeData, AppError> {
  let pool = super::db::get_pool(db);
  let conn = pool
    .get()
    .map_err(|e: r2d2::Error| AppError::ResourceUnavailableError(e.to_string()))?;

  let mut stmt = conn
    .prepare("SELECT open_id, manager_id FROM open_ids")
    .map_err(|e: rusqlite::Error| AppError::ResourceUnavailableError(e.to_string()))?;
  let open_ids = stmt
    .query_map([], |row: &rusqlite::Row| {
      Ok(OpenIdRecord {
        open_id: row.get(0)?,
        manager_id: row.get(1)?,
      })
    })
    .map_err(|e: rusqlite::Error| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, rusqlite::Error>>()
    .map_err(|e: rusqlite::Error| AppError::ResourceUnavailableError(e.to_string()))?;

  if open_ids.is_empty() {
    return Err(AppError::ResourceUnavailableError(
      "OpenID 列表为空".to_string(),
    ));
  }

  let shops = super::db::get_all_shops(db)?;

  Ok(RuntimeData { open_ids, shops })
}
