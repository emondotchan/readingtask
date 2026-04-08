use anyhow::{Result, anyhow};
use log::{error, info};
use reading_task::{DbContext, TaskItemOutcome, TaskRunRequest, TaskRunSummary, run_task};

pub async fn run(db: &DbContext, request: TaskRunRequest) -> Result<()> {
  let summary = run_task(db, request)
    .await
    .map_err(|error| anyhow!("{error}"))?;

  render_items(&summary);
  render_final_status(&summary);
  Ok(())
}

fn render_items(summary: &TaskRunSummary) {
  for item in &summary.items {
    match item.outcome {
      TaskItemOutcome::Success => {
        let text = item.response_text.as_deref().unwrap_or("");
        info!(
          "[{}/{}] OpenID={} ShopCode={} {}-{} HTTP {}\n{}\n",
          item.index,
          summary.requested_count,
          item.open_id,
          item.shop_code,
          item.province,
          item.city,
          item.http_status.unwrap_or_default(),
          text
        );
      }
      TaskItemOutcome::RequestError | TaskItemOutcome::ResponseReadError => {
        let error_message = item.error_message.as_deref().unwrap_or("未知错误");
        error!(
          "[{}/{}] OpenID={} ShopCode={} {}",
          item.index, summary.requested_count, item.open_id, item.shop_code, error_message
        );
      }
    }
  }
}

fn render_final_status(summary: &TaskRunSummary) {
  info!(
    "执行完成：请求 {}，处理 {}，成功 {}，失败 {}，开始时间 {}，结束时间 {}",
    summary.requested_count,
    summary.processed_count,
    summary.success_count,
    summary.failure_count,
    summary.started_at,
    summary.finished_at
  );
}
