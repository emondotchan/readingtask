use crate as reading_task;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use reading_task::AppError;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
  pub category: String,
  pub message: String,
}

impl std::fmt::Display for CommandError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "[{}] {}", self.category, self.message)
  }
}

fn is_completed_message(message: &str) -> bool {
  matches!(
    message,
    "该月度任务已全部完成"
      | "今日任务已经执行完成，请明天再执行"
      | "今日任务已经完成，请明天再执行"
  )
}

impl From<AppError> for CommandError {
  fn from(err: AppError) -> Self {
    let category = match &err {
      AppError::ValidationError(_) => "validation",
      AppError::ResourceUnavailableError(_) => "resource",
      AppError::Paused(_) => "paused",
      AppError::ExecutionError(message) if is_completed_message(message) => "completed",
      AppError::ExecutionError(_) => "execution",
    };

    Self {
      category: category.to_string(),
      message: err.to_string(),
    }
  }
}

impl CommandError {
  fn status_code(&self) -> StatusCode {
    match self.category.as_str() {
      "validation" => StatusCode::BAD_REQUEST,
      "resource" => StatusCode::CONFLICT,
      "paused" | "completed" => StatusCode::CONFLICT,
      _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }
}

impl IntoResponse for CommandError {
  fn into_response(self) -> Response {
    (self.status_code(), Json(self)).into_response()
  }
}
