use rusqlite::params;

use super::context::{DbContext, get_conn};
use crate::core::error::AppError;
use crate::core::model::FcRecord;

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
  let mut conn = get_conn(db)?;
  let tx = conn
    .transaction()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if let Some(prev) = previous_name
    && prev != fc.name
  {
    tx.execute("DELETE FROM fcs WHERE name = ?1", params![prev])
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  }

  tx.execute(
    "INSERT OR REPLACE INTO fcs (name) VALUES (?1)",
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
