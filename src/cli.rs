use clap::Parser;
use reading_task::TaskRunRequest;

#[derive(Debug, Parser)]
#[command(
  author,
  version,
  about = "Submit reading logs with random OpenID values"
)]
pub struct Args {
  #[arg(short = 'c', long)]
  pub s_course_id: String,

  #[arg(short = 'm', long)]
  pub s_manager_id: String,

  #[arg(short = 'f', long)]
  pub fc: String,

  #[arg(short = 'n', long = "count", default_value_t = 1)]
  pub count: usize,

  #[arg(short = 's', long = "shopcodes", value_delimiter = ',')]
  pub shopcodes: Option<Vec<String>>,
}

impl Args {
  pub fn into_task_run_request(self) -> TaskRunRequest {
    TaskRunRequest {
      s_course_id: self.s_course_id,
      s_manager_id: self.s_manager_id,
      fc: self.fc,
      count: self.count,
      shopcodes: self.shopcodes.unwrap_or_default(),
    }
  }
}
