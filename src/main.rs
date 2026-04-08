mod app;
mod cli;

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
  reading_task::init_logging();
  let db = reading_task::init_db_context(&reading_task::AppPaths::new())?;
  let request = cli::Args::parse().into_task_run_request();
  app::run(&db, request).await
}
