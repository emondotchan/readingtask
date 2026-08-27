use std::process::exit;

#[tokio::main]
async fn main() {
  if let Err(error) = reading_task::web::run().await {
    eprintln!("{error}");
    exit(1);
  }
}
