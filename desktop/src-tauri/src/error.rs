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

impl From<AppError> for CommandError {
  fn from(err: AppError) -> Self {
    let category = match &err {
      AppError::ValidationError(_) => "validation",
      AppError::ResourceUnavailableError(_) => "resource",
      AppError::Paused(_) => "paused",
      AppError::ExecutionError(_) => "execution",
    };
    Self {
      category: category.to_string(),
      message: err.to_string(),
    }
  }
}
