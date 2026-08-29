use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use log::Level;

use crate as reading_task;
use reading_task::ShopRecord;

use crate::web::dto::{ShopCountQuery, UpdateShopTypesInput};
use crate::web::error::CommandError;
use crate::web::state::AppState;
use crate::web::utils::{log_command, run_blocking_db, validation_error};

pub async fn get_shops(
  State(state): State<AppState>,
) -> Result<Json<Vec<ShopRecord>>, CommandError> {
  let db = state.resolve_db()?;
  let shops = run_blocking_db(move || reading_task::get_all_shops(&db)).await?;
  Ok(Json(shops))
}

pub async fn import_shops(
  State(state): State<AppState>,
  Json(shops): Json<Vec<ShopRecord>>,
) -> Result<Json<usize>, CommandError> {
  if shops.is_empty() {
    return Err(validation_error("导入门店不能为空"));
  }

  if shops.iter().any(|shop| shop.shop_code.trim().is_empty()) {
    return Err(validation_error("导入门店存在空的 ShopCode"));
  }

  let db = state.resolve_db()?;
  let imported = run_blocking_db(move || reading_task::import_shops(&db, &shops)).await?;
  Ok(Json(imported))
}

pub async fn update_shop_types(
  State(state): State<AppState>,
  Json(input): Json<UpdateShopTypesInput>,
) -> Result<Json<usize>, CommandError> {
  let shop_codes = input
    .shop_codes
    .iter()
    .map(|shop_code| shop_code.trim().to_string())
    .filter(|shop_code| !shop_code.is_empty())
    .collect::<Vec<_>>();

  if shop_codes.is_empty() {
    return Err(validation_error("更新门店类型不能为空"));
  }

  let db = state.resolve_db()?;
  let updated = run_blocking_db(move || {
    reading_task::update_shop_type_by_codes(&db, &shop_codes, input.shop_type)
  })
  .await?;
  Ok(Json(updated))
}

pub async fn delete_all_shops(State(state): State<AppState>) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  run_blocking_db(move || reading_task::delete_all_shops(&db)).await?;
  Ok(StatusCode::NO_CONTENT)
}

pub async fn get_shop_count(
  State(state): State<AppState>,
  Query(query): Query<ShopCountQuery>,
) -> Result<Json<usize>, CommandError> {
  let db = state.resolve_db()?;
  log_command(
    Level::Debug,
    "get_shop_count",
    format!("fc_name={} task_type={}", query.fc_name, query.task_type),
  );
  let count = run_blocking_db(move || {
    reading_task::get_shop_count_by_fc_and_type(&db, &query.fc_name, &query.task_type)
  })
  .await?;
  log_command(Level::Debug, "get_shop_count", format!("count={count}"));
  Ok(Json(count))
}
