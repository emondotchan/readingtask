use rusqlite::params;

use super::context::{DbContext, get_conn};
use crate::core::error::AppError;
use crate::core::model::OpenIdRecord;

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

pub fn get_open_ids_by_fc(db: &DbContext, fc_name: &str) -> Result<Vec<OpenIdRecord>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare("SELECT open_id, fc_name FROM open_ids WHERE fc_name = ?1")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let records = stmt
    .query_map(params![fc_name], |row| {
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
      "INSERT OR REPLACE INTO open_ids (open_id, fc_name) VALUES (?1, ?2)",
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
