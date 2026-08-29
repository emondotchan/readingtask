use std::sync::LazyLock;
use std::time::Duration;

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};

use super::runner::current_datetime_string;
use crate::core::error::AppError;
use crate::core::model::{ShopRecord, TaskItemOutcome, TaskItemResult};

pub(crate) const SUBMIT_READ_LOG_URL: &str =
  "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx/SubmitReadLog";

#[derive(Debug, Serialize)]
pub(crate) struct SubmitReadLogBody<'a> {
  #[serde(rename = "sCourseID")]
  pub s_course_id: &'a str,
  #[serde(rename = "sManagerID")]
  pub s_manager_id: &'a str,
  #[serde(rename = "OpenID")]
  pub open_id: &'a str,
  #[serde(rename = "Province")]
  pub province: &'a str,
  #[serde(rename = "City")]
  pub city: &'a str,
  #[serde(rename = "ShopCode")]
  pub shop_code: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum SubmitReadLogResponseEnvelope {
  Wrapped { d: String },
  Direct(SubmitReadLogPayload),
}

#[derive(Debug, Deserialize)]
pub(crate) struct SubmitReadLogPayload {
  pub err: i32,
  #[serde(rename = "RtnMsg")]
  pub rtn_msg: String,
  #[serde(rename = "ReadID")]
  pub read_id: Option<String>,
}

static SHARED_HTTP_CLIENT: LazyLock<Result<reqwest::Client, String>> =
  LazyLock::new(|| build_http_client_inner().map_err(|e| e.to_string()));

pub fn get_http_client() -> Result<&'static reqwest::Client, AppError> {
  match &*SHARED_HTTP_CLIENT {
    Ok(client) => Ok(client),
    Err(e) => Err(AppError::ResourceUnavailableError(format!(
      "初始化 HTTP 客户端失败: {e}"
    ))),
  }
}

fn build_http_client_inner() -> Result<reqwest::Client, AppError> {
  reqwest::Client::builder()
    .timeout(Duration::from_secs(15))
    .connect_timeout(Duration::from_secs(10))
    .pool_idle_timeout(Duration::from_secs(90))
    .pool_max_idle_per_host(10)
    .tcp_keepalive(Duration::from_secs(60))
    .build()
    .map_err(|error| AppError::ResourceUnavailableError(format!("创建 HTTP 客户端失败: {error}")))
}

#[derive(Debug, Clone)]
pub(crate) struct ClassifiedSubmitReadLogResponse {
  pub response_text: String,
  pub submit_err: Option<i32>,
  pub rtn_msg: Option<String>,
  pub read_id: Option<String>,
  pub outcome: TaskItemOutcome,
}

pub(crate) fn build_task_item_result(
  index: usize,
  open_id: &str,
  shop: &ShopRecord,
  http_status: Option<u16>,
  classified: ClassifiedSubmitReadLogResponse,
) -> TaskItemResult {
  TaskItemResult {
    result_id: None,
    index: index + 1,
    executed_date: Some(current_datetime_string()),
    submit_err: classified.submit_err,
    rtn_msg: classified.rtn_msg,
    read_id: classified.read_id,
    open_id: open_id.to_string(),
    shop_code: shop.shop_code.clone(),
    province: shop.province.clone(),
    city: shop.city.clone(),
    http_status,
    response_text: Some(classified.response_text),
    outcome: classified.outcome,
  }
}

pub(crate) fn classify_submit_read_log_response(text: &str) -> ClassifiedSubmitReadLogResponse {
  if let Some(payload) = parse_submit_read_log_payload(text) {
    let outcome = if payload.err == 0 {
      TaskItemOutcome::Success
    } else {
      TaskItemOutcome::RequestError
    };

    return ClassifiedSubmitReadLogResponse {
      response_text: text.to_string(),
      submit_err: Some(payload.err),
      rtn_msg: Some(payload.rtn_msg),
      read_id: payload.read_id,
      outcome,
    };
  }

  ClassifiedSubmitReadLogResponse {
    response_text: text.to_string(),
    submit_err: None,
    rtn_msg: None,
    read_id: None,
    outcome: TaskItemOutcome::Success,
  }
}

pub(crate) fn parse_submit_read_log_payload(text: &str) -> Option<SubmitReadLogPayload> {
  let envelope = serde_json::from_str::<SubmitReadLogResponseEnvelope>(text).ok()?;
  match envelope {
    SubmitReadLogResponseEnvelope::Direct(payload) => Some(payload),
    SubmitReadLogResponseEnvelope::Wrapped { d } => {
      serde_json::from_str::<SubmitReadLogPayload>(&d).ok()
    }
  }
}

pub(crate) async fn execute_single_request(
  client: &reqwest::Client,
  body: &SubmitReadLogBody<'_>,
  referer: String,
  index: usize,
  open_id: &str,
  shop: &ShopRecord,
) -> TaskItemResult {
  let mut headers = HeaderMap::new();
  headers.insert(
    CONTENT_TYPE,
    HeaderValue::from_static("application/json; charset=UTF-8"),
  );
  headers.insert(
    USER_AGENT,
    HeaderValue::from_static(
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko)        Chrome/122.0.0.0 Safari/537.36 MicroMessenger/7.0.20.1781(0x6700143B)        NetType/WIFI MiniProgramEnv/Windows WindowsWechat/WMPF WindowsWechat(0x6309092b)        XWEB/11275",
    ),
  );
  if let Ok(referer_header) = HeaderValue::from_str(&referer) {
    headers.insert(REFERER, referer_header);
  }

  match client
    .post(SUBMIT_READ_LOG_URL)
    .headers(headers)
    .json(body)
    .send()
    .await
  {
    Ok(response) => {
      let status = response.status().as_u16();
      match response.text().await {
        Ok(text) => {
          let classified = classify_submit_read_log_response(&text);
          build_task_item_result(index, open_id, shop, Some(status), classified)
        }
        Err(error) => TaskItemResult {
          result_id: None,
          index: index + 1,
          executed_date: Some(current_datetime_string()),
          submit_err: None,
          rtn_msg: None,
          read_id: None,
          open_id: open_id.to_string(),
          shop_code: shop.shop_code.clone(),
          province: shop.province.clone(),
          city: shop.city.clone(),
          http_status: Some(status),
          response_text: Some(format!("读取响应失败: {error}")),
          outcome: TaskItemOutcome::ResponseReadError,
        },
      }
    }
    Err(error) => TaskItemResult {
      result_id: None,
      index: index + 1,
      executed_date: Some(current_datetime_string()),
      submit_err: None,
      rtn_msg: None,
      read_id: None,
      open_id: open_id.to_string(),
      shop_code: shop.shop_code.clone(),
      province: shop.province.clone(),
      city: shop.city.clone(),
      http_status: None,
      response_text: Some(format!("请求失败: {error}")),
      outcome: TaskItemOutcome::RequestError,
    },
  }
}
