use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
  #[error("{0}")]
  ValidationError(String),

  #[error("{0}")]
  ResourceUnavailableError(String),

  #[error("{0}")]
  Paused(String),

  #[error("{0}")]
  ExecutionError(String),
}
