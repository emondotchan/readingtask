use rand::RngExt;
use rand::seq::SliceRandom;
use std::collections::{HashMap, HashSet};

use crate::core::db::DbContext;
use crate::core::error::AppError;
use crate::core::model::{
  OpenIdRecord, SHOP_TYPE_AVENE, SHOP_TYPE_AVENE_KLORANE, SHOP_TYPE_KLORANE, ShopRecord,
  TaskRunRequest,
};

pub(crate) const GENERATED_OPEN_ID_PREFIX: &str = "o-kP6s";
pub(crate) const GENERATED_OPEN_ID_LEN: usize = 28;

pub(crate) fn select_shops(
  db: &DbContext,
  request: &TaskRunRequest,
) -> Result<Vec<ShopRecord>, AppError> {
  if !request.shopcodes.is_empty() {
    let requested_shopcodes = normalize_shopcodes(&request.shopcodes);
    if requested_shopcodes.is_empty() {
      return Err(AppError::ValidationError(
        "至少提供一个有效的 shopcode".to_string(),
      ));
    }

    let shops = crate::core::db::get_shops_by_codes(db, &requested_shopcodes)?;
    let mut shops_by_code: HashMap<String, ShopRecord> = shops
      .into_iter()
      .map(|shop| (shop.shop_code.clone(), shop))
      .collect();
    let mut matched_shops = Vec::with_capacity(requested_shopcodes.len());
    let mut missing_shopcodes = Vec::new();

    for shopcode in requested_shopcodes {
      match shops_by_code.remove(&shopcode) {
        Some(shop) => matched_shops.push(shop),
        None => missing_shopcodes.push(shopcode),
      }
    }

    if !missing_shopcodes.is_empty() {
      return Err(AppError::ResourceUnavailableError(format!(
        "未在 SQLite 门店数据中找到指定的门店代码: {:?}",
        missing_shopcodes
      )));
    }

    return Ok(matched_shops);
  }

  let matched_shops = crate::core::db::get_shops_by_fc(db, &request.fc)?;
  if matched_shops.is_empty() {
    return Err(AppError::ResourceUnavailableError(format!(
      "未在 SQLite 门店数据中找到 FC={} 对应的门店",
      request.fc
    )));
  }

  if request.count > matched_shops.len() {
    return Err(AppError::ResourceUnavailableError(format!(
      "请求数量 {} 超过可用门店数量 {}",
      request.count,
      matched_shops.len()
    )));
  }

  Ok(sample_shops(matched_shops, request.count))
}

pub(crate) fn normalize_shopcodes(shopcodes: &[String]) -> Vec<String> {
  let mut seen = HashSet::new();

  shopcodes
    .iter()
    .map(|shopcode| shopcode.trim())
    .filter(|shopcode| !shopcode.is_empty())
    .filter(|shopcode| seen.insert((*shopcode).to_string()))
    .map(ToOwned::to_owned)
    .collect()
}

pub(crate) fn normalize_open_ids(open_ids: &[String]) -> Vec<String> {
  let mut seen = HashSet::new();

  open_ids
    .iter()
    .map(|open_id| open_id.trim())
    .filter(|open_id| !open_id.is_empty())
    .filter(|open_id| seen.insert((*open_id).to_string()))
    .map(ToOwned::to_owned)
    .collect()
}

pub(crate) fn select_planned_shops(
  shops: Vec<ShopRecord>,
  planned_shopcodes: &[String],
) -> Result<Vec<ShopRecord>, AppError> {
  let shops_by_code: HashMap<&str, &ShopRecord> = shops
    .iter()
    .map(|shop| (shop.shop_code.as_str(), shop))
    .collect();
  let mut selected = Vec::with_capacity(planned_shopcodes.len());

  for shopcode in planned_shopcodes {
    if let Some(&shop) = shops_by_code.get(shopcode.as_str()) {
      selected.push(shop.clone());
    } else {
      log::warn!("计划门店不存在或已被删除，跳过该门店: {}", shopcode);
    }
  }

  Ok(selected)
}

pub(crate) fn select_custom_monthly_shops(
  shops: Vec<ShopRecord>,
  requested_shopcodes: &[String],
) -> Result<Vec<ShopRecord>, AppError> {
  let shops_by_code: HashMap<&str, &ShopRecord> = shops
    .iter()
    .map(|shop| (shop.shop_code.as_str(), shop))
    .collect();
  let mut selected = Vec::with_capacity(requested_shopcodes.len());
  let mut missing = Vec::new();

  for shopcode in requested_shopcodes {
    if let Some(&shop) = shops_by_code.get(shopcode.as_str()) {
      selected.push(shop.clone());
    } else {
      missing.push(shopcode.clone());
    }
  }

  if !missing.is_empty() {
    return Err(AppError::ValidationError(format!(
      "以下 shopcode 不存在、已删除，或不属于当前 FC/任务类型: {}",
      missing.join(", ")
    )));
  }

  Ok(selected)
}

pub(crate) fn select_open_ids(
  open_ids: Vec<OpenIdRecord>,
  fc_name: &str,
  used_open_ids: &HashSet<String>,
  excluded_open_ids: &HashSet<String>,
  count: usize,
) -> Result<Vec<String>, AppError> {
  let matched_open_ids = open_ids
    .into_iter()
    .filter(|item| item.fc_name.trim() == fc_name.trim())
    .map(|item| item.open_id)
    .filter(|open_id| !used_open_ids.contains(open_id))
    .filter(|open_id| !excluded_open_ids.contains(open_id))
    .collect::<Vec<_>>();

  let selected = if count <= matched_open_ids.len() {
    sample_open_ids(matched_open_ids, count)
  } else {
    let mut combined = matched_open_ids;
    let missing_count = count - combined.len();
    let generated = generate_open_ids(missing_count, used_open_ids, excluded_open_ids);
    combined.extend(generated);
    combined
  };

  Ok(selected)
}

pub(crate) fn sample_open_ids(mut open_ids: Vec<String>, count: usize) -> Vec<String> {
  let mut rng = rand::rng();
  open_ids.shuffle(&mut rng);
  open_ids.into_iter().take(count).collect()
}

pub(crate) fn generate_open_ids(
  count: usize,
  used_open_ids: &HashSet<String>,
  excluded_open_ids: &HashSet<String>,
) -> Vec<String> {
  let mut generated = Vec::with_capacity(count);
  let mut seen = HashSet::new();

  while generated.len() < count {
    let open_id = generate_open_id();
    if !used_open_ids.contains(&open_id)
      && !excluded_open_ids.contains(&open_id)
      && seen.insert(open_id.clone())
    {
      generated.push(open_id);
    }
  }

  generated
}

pub(crate) fn generate_open_id() -> String {
  let mut rng = rand::rng();
  let suffix_len = GENERATED_OPEN_ID_LEN - GENERATED_OPEN_ID_PREFIX.len();
  let suffix = generate_open_id_suffix(&mut rng, suffix_len);
  format!("{GENERATED_OPEN_ID_PREFIX}{suffix}")
}

pub(crate) fn generate_open_id_suffix(rng: &mut impl rand::Rng, len: usize) -> String {
  const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
  (0..len)
    .map(|_| {
      let idx = rng.random_range(0..CHARSET.len());
      CHARSET[idx] as char
    })
    .collect()
}

pub(crate) fn sample_shops(mut shops: Vec<ShopRecord>, count: usize) -> Vec<ShopRecord> {
  let mut rng = rand::rng();
  shops.shuffle(&mut rng);
  shops.into_iter().take(count).collect()
}

pub(crate) fn filter_task_shops(
  shops: Vec<ShopRecord>,
  fc_name: &str,
  task_type: &str,
) -> Vec<ShopRecord> {
  shops
    .into_iter()
    .filter(|shop| shop.fc.as_deref() == Some(fc_name))
    .filter(|shop| task_type_matches_shop(task_type, shop.shop_type))
    .collect()
}

pub(crate) fn task_type_matches_shop(task_type: &str, shop_type: u8) -> bool {
  match task_type {
    "Avene" => shop_type == SHOP_TYPE_AVENE || shop_type == SHOP_TYPE_AVENE_KLORANE,
    "Klorane" => shop_type == SHOP_TYPE_KLORANE || shop_type == SHOP_TYPE_AVENE_KLORANE,
    _ => true,
  }
}
