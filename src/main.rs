#[tokio::main]
async fn main() {
  if let Err(error) = reading_task::web::run().await {
    eprintln!("{error}");
    std::process::exit(1);
  }
}
