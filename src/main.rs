mod app;
mod cli;

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
  let request = cli::Args::parse().into_task_run_request();
  app::run(request).await
}
