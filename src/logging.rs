pub fn init_logging() {
  let env = env_logger::Env::default().default_filter_or("info");

  let _ = env_logger::Builder::from_env(env)
    .format_timestamp_secs()
    .try_init();
}
