use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
  #[error("{0}")]
  ValidationError(String),

  #[error("读取配置文件失败: {path}")]
  ConfigReadError {
    path: PathBuf,
    #[source]
    source: io::Error,
  },

  #[error("解析配置文件失败: {path}: {message}")]
  ConfigParseError { path: PathBuf, message: String },

  #[error("{0}")]
  ResourceUnavailableError(String),

  #[error("{0}")]
  ExecutionError(String),
}

impl AppError {
  pub(crate) fn config_read(path: &Path, source: io::Error) -> Self {
    Self::ConfigReadError {
      path: path.to_path_buf(),
      source,
    }
  }

  pub(crate) fn config_parse(path: &Path, message: impl Into<String>) -> Self {
    Self::ConfigParseError {
      path: path.to_path_buf(),
      message: message.into(),
    }
  }
}
