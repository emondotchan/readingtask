use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use super::error::AppError;
use super::model::{AppPaths, OpenIdRecord, ShopRecord};

#[derive(Debug)]
pub(crate) struct RuntimeData {
  pub(crate) open_ids: Vec<OpenIdRecord>,
  pub(crate) shops: Vec<ShopRecord>,
}

#[derive(Debug, Deserialize)]
struct OpenIdsFile {
  openids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ShopsFile {
  shops: Vec<ShopEntry>,
}

#[derive(Debug, Deserialize)]
struct ShopEntry {
  #[serde(rename = "Province")]
  province: String,
  #[serde(rename = "City")]
  city: String,
  #[serde(rename = "ShopCode")]
  shop_code: String,
  #[serde(rename = "FC")]
  fc: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProvincesFile {
  provinces: Vec<ProvinceEntry>,
}

#[derive(Debug, Deserialize)]
struct ProvinceEntry {
  #[serde(rename = "ProvinceName")]
  _province_name: String,
  #[serde(rename = "CityName")]
  _city_name: String,
}

pub(crate) fn load_runtime_data(paths: &AppPaths) -> Result<RuntimeData, AppError> {
  super::db::init_db(paths)?;
  let db = super::db::open_existing_db(paths)?;

  let mut open_ids = Vec::new();
  let mut shops = Vec::new();

  let iter = db.iterator(rocksdb::IteratorMode::Start);
  for item in iter {
    let (key, value) =
      item.map_err(|e| AppError::ResourceUnavailableError(format!("数据库迭代错误: {}", e)))?;
    let key_str = String::from_utf8_lossy(&key);

    if key_str.starts_with("openid:") {
      let record: OpenIdRecord = serde_json::from_slice(&value).unwrap_or_else(|_| OpenIdRecord {
        manager_id: String::new(),
        open_id: key_str.strip_prefix("openid:").unwrap().to_string(),
      });
      open_ids.push(record);
    } else if key_str.starts_with("shop:") {
      let shop: ShopRecord = serde_json::from_slice(&value)
        .map_err(|e| AppError::ResourceUnavailableError(format!("数据库解析错误: {}", e)))?;
      shops.push(shop);
    }
  }

  let provinces_path = paths.config_dir.join("province.toml");
  load_provinces(&provinces_path)?;

  if open_ids.is_empty() {
    return Err(AppError::ResourceUnavailableError(
      "OpenID 列表为空".to_string(),
    ));
  }

  Ok(RuntimeData { open_ids, shops })
}

pub(crate) fn load_open_ids_from_toml(path: &Path) -> Result<Vec<String>, AppError> {
  let parsed: OpenIdsFile = parse_toml_file(path)?;
  let mut seen = HashSet::new();
  let deduped = parsed
    .openids
    .into_iter()
    .filter(|open_id| seen.insert(open_id.clone()))
    .collect::<Vec<_>>();

  if deduped.is_empty() {
    return Err(AppError::ResourceUnavailableError(
      "OpenID 列表为空".to_string(),
    ));
  }

  Ok(deduped)
}

pub(crate) fn load_shops_from_toml(path: &Path) -> Result<Vec<ShopRecord>, AppError> {
  let parsed: ShopsFile = parse_toml_file(path)?;
  let shops = parsed
    .shops
    .into_iter()
    .map(|shop| ShopRecord {
      province: shop.province,
      city: shop.city,
      shop_code: shop.shop_code,
      fc: shop.fc,
    })
    .collect();
  Ok(shops)
}

fn load_provinces(path: &Path) -> Result<(), AppError> {
  let parsed: ProvincesFile = parse_toml_file(path)?;
  if parsed.provinces.is_empty() {
    return Err(AppError::ResourceUnavailableError(
      "province.toml 中没有可用省市数据".to_string(),
    ));
  }
  Ok(())
}

fn parse_toml_file<T>(path: &Path) -> Result<T, AppError>
where
  T: DeserializeOwned,
{
  let content = fs::read_to_string(path).map_err(|error| AppError::config_read(path, error))?;
  toml::from_str(&content).map_err(|error| AppError::config_parse(path, error.to_string()))
}
