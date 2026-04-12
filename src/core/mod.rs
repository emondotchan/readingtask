pub mod db;
mod error;
mod executor;
mod loader;
pub mod model;

pub use db::{
  DbContext, add_monthly_task, add_open_id, add_or_update_course, add_or_update_fc,
  add_or_update_shop, delete_course, delete_fc, delete_monthly_task, delete_open_id, delete_shop,
  get_all_courses, get_all_daily_tasks_for_task, get_all_fcs, get_all_monthly_tasks,
  get_all_open_id_records, get_all_open_ids, get_all_shops, get_daily_task,
  get_shop_count_by_fc_and_type, get_task_results, get_used_open_ids_for_month, import_open_ids_csv,
  init_db, init_db_context, save_daily_task,
};
pub use error::AppError;
pub use executor::{
  create_monthly_task_with_plan, estimate_target_days, preview_monthly_task_plan,
  run_daily_task_with_progress, run_daily_task_with_progress_controlled, run_task,
  run_task_with_progress,
};
pub use model::{
  AppPaths, CourseRecord, DailyTask, FcRecord, MonthlyTask, MonthlyTaskPlanPreview, OpenIdRecord,
  QuickRunArchiveResult, QuickRunArchiveStatus, ShopRecord, TaskItemOutcome, TaskItemResult,
  TaskProgress, TaskRunRequest, TaskRunSummary,
};
