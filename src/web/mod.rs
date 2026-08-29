pub mod bootstrap;
pub mod dto;
pub mod error;
pub mod handlers;
pub mod state;
pub mod static_files;
pub mod utils;

use std::net::SocketAddr;

use axum::Router;
use tokio::net::TcpListener;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;

use crate as reading_task;
use reading_task::AppPaths;

pub use dto::*;
pub use error::CommandError;
pub use state::AppState;
pub use utils::default_bind_addr;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
  reading_task::init_logging();

  let runtime_paths = bootstrap::initialize()?;
  let db = runtime_paths
    .db_path
    .as_ref()
    .map(|db_path| reading_task::init_db_context(&AppPaths::new_with_db_path(db_path.clone())))
    .transpose()?;
  let state = AppState::new(runtime_paths, db);

  let api_router = handlers::build_api_router(state.clone());

  let app = Router::new()
    .nest("/api", api_router)
    .fallback(static_files::serve_embedded_static)
    .layer(CompressionLayer::new())
    .layer(CorsLayer::permissive())
    .with_state(state);

  let addr: SocketAddr = std::env::var("READING_TASK_BIND")
    .ok()
    .and_then(|value| value.parse().ok())
    .unwrap_or_else(default_bind_addr);
  let listener = TcpListener::bind(addr).await?;
  log::info!("web server listening on http://{}", listener.local_addr()?);
  axum::serve(listener, app).await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_bind_addr_listens_on_all_interfaces() {
    assert_eq!(default_bind_addr(), SocketAddr::from(([0, 0, 0, 0], 10086)));
  }
}
