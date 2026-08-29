use rusqlite::params;

use super::context::{DbContext, get_conn};
use crate::core::error::AppError;
use crate::core::model::{SHOP_TYPE_AVENE, SHOP_TYPE_AVENE_KLORANE, SHOP_TYPE_KLORANE, ShopRecord};

pub fn get_all_shops(db: &DbContext) -> Result<Vec<ShopRecord>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare("SELECT province, city, shop_code, shop_name, fc, shop_type FROM shops")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let shops = stmt
    .query_map([], |row| {
      Ok(ShopRecord {
        province: row.get(0)?,
        city: row.get(1)?,
        shop_code: row.get(2)?,
        shop_name: row.get(3)?,
        fc: row.get(4)?,
        shop_type: row.get::<_, i64>(5)? as u8,
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  Ok(shops)
}

pub fn get_shops_by_fc_and_type(
  db: &DbContext,
  fc_name: &str,
  task_type: &str,
) -> Result<Vec<ShopRecord>, AppError> {
  let conn = get_conn(db)?;
  let shop_type = match task_type {
    "Avene" => SHOP_TYPE_AVENE,
    "Klorane" => SHOP_TYPE_KLORANE,
    _ => SHOP_TYPE_AVENE,
  };
  let mut stmt = conn
    .prepare(
      "SELECT province, city, shop_code, shop_name, fc, shop_type
       FROM shops
       WHERE fc = ?1 AND (shop_type = ?2 OR shop_type = ?3)",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let shops = stmt
    .query_map(
      params![fc_name, shop_type as i64, SHOP_TYPE_AVENE_KLORANE as i64],
      |row| {
        Ok(ShopRecord {
          province: row.get(0)?,
          city: row.get(1)?,
          shop_code: row.get(2)?,
          shop_name: row.get(3)?,
          fc: row.get(4)?,
          shop_type: row.get::<_, i64>(5)? as u8,
        })
      },
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(shops)
}

pub fn get_shops_by_fc(db: &DbContext, fc_name: &str) -> Result<Vec<ShopRecord>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare(
      "SELECT province, city, shop_code, shop_name, fc, shop_type
       FROM shops
       WHERE fc = ?1",
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let shops = stmt
    .query_map(params![fc_name], |row| {
      Ok(ShopRecord {
        province: row.get(0)?,
        city: row.get(1)?,
        shop_code: row.get(2)?,
        shop_name: row.get(3)?,
        fc: row.get(4)?,
        shop_type: row.get::<_, i64>(5)? as u8,
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(shops)
}

pub fn get_shops_by_codes(
  db: &DbContext,
  shop_codes: &[String],
) -> Result<Vec<ShopRecord>, AppError> {
  if shop_codes.is_empty() {
    return Ok(Vec::new());
  }
  let conn = get_conn(db)?;
  let mut results = Vec::with_capacity(shop_codes.len());

  for chunk in shop_codes.chunks(500) {
    let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
      "SELECT province, city, shop_code, shop_name, fc, shop_type FROM shops WHERE shop_code IN ({placeholders})"
    );
    let mut stmt = conn
      .prepare(&sql)
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    let params = rusqlite::params_from_iter(chunk);
    let chunk_shops = stmt
      .query_map(params, |row| {
        Ok(ShopRecord {
          province: row.get(0)?,
          city: row.get(1)?,
          shop_code: row.get(2)?,
          shop_name: row.get(3)?,
          fc: row.get(4)?,
          shop_type: row.get::<_, i64>(5)? as u8,
        })
      })
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
    results.extend(chunk_shops);
  }

  Ok(results)
}

pub fn get_shop_count_by_fc_and_type(
  db: &DbContext,
  fc_name: &str,
  task_type: &str,
) -> Result<usize, AppError> {
  let conn = get_conn(db)?;
  let shop_type = match task_type {
    "Avene" => SHOP_TYPE_AVENE,
    "Klorane" => SHOP_TYPE_KLORANE,
    _ => SHOP_TYPE_AVENE,
  };

  let sql = format!(
    "SELECT COUNT(*) FROM shops WHERE fc = ?1 AND (shop_type = ?2 OR shop_type = {})",
    SHOP_TYPE_AVENE_KLORANE
  );
  log::debug!(
    "[DEBUG] get_shop_count_by_fc_and_type: SQL={sql} fc_name={fc_name} shop_type={shop_type}"
  );
  let mut stmt = conn
    .prepare(&sql)
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let count: i64 = stmt
    .query_row(params![fc_name, shop_type], |row| row.get(0))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  log::debug!("[DEBUG] get_shop_count_by_fc_and_type: result count={count}");

  let eligible_count = count as usize;
  let target_count = match task_type {
    "Avene" => (eligible_count * 4).div_ceil(5),
    "Klorane" => eligible_count,
    _ => eligible_count,
  };

  Ok(target_count)
}

pub fn import_shops(db: &DbContext, shops: &[ShopRecord]) -> Result<usize, AppError> {
  let mut conn = get_conn(db)?;
  let tx = conn
    .transaction()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  for shop in shops {
    tx.execute(
      "INSERT OR REPLACE INTO shops (province, city, shop_code, shop_name, fc, shop_type) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
      params![
        shop.province,
        shop.city,
        shop.shop_code,
        shop.shop_name,
        shop.fc,
        shop.shop_type as i64
      ],
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  }

  tx.commit()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(shops.len())
}

pub fn update_shop_type_by_codes(
  db: &DbContext,
  shop_codes: &[String],
  shop_type: u8,
) -> Result<usize, AppError> {
  if shop_codes.is_empty() {
    return Ok(0);
  }

  let mut conn = get_conn(db)?;
  let tx = conn
    .transaction()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  let placeholders: Vec<String> = (1..=shop_codes.len())
    .map(|i| format!("?{}", i + 1))
    .collect();
  let sql = format!(
    "UPDATE shops SET shop_type = ?1 WHERE shop_code IN ({})",
    placeholders.join(", ")
  );

  let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
  params_vec.push(Box::new(shop_type as i64));
  for code in shop_codes {
    params_vec.push(Box::new(code.clone()));
  }

  let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();

  let updated = tx
    .execute(&sql, rusqlite::params_from_iter(params_refs))
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  tx.commit()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(updated)
}

pub fn delete_all_shops(db: &DbContext) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  conn
    .execute("DELETE FROM shops", [])
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}
