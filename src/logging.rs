use std::io::Write;

pub fn init_logging() {
  let env = env_logger::Env::default().default_filter_or("info");

  let _ = env_logger::Builder::from_env(env)
    .format(|buf, record| {
      let tz_offset = chrono::FixedOffset::east_opt(8 * 3600).unwrap();
      let now = chrono::Utc::now().with_timezone(&tz_offset);
      writeln!(
        buf,
        "[{time} {level:<5} {target}] {args}",
        time = now.format("%Y-%m-%d %H:%M"),
        level = record.level(),
        target = record.target(),
        args = record.args()
      )
    })
    .try_init();
}
