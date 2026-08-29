pub mod context;
pub mod courses;
pub mod fcs;
pub mod open_ids;
pub mod results;
pub mod schema;
pub mod shops;
pub mod tasks;

#[cfg(test)]
mod tests;

pub use context::{DbContext, init_db_context};
pub use courses::{add_or_update_course, delete_course, get_all_courses};
pub use fcs::{add_or_update_fc, delete_fc, get_all_fcs};
pub use open_ids::{
  add_open_id, delete_open_id, get_all_open_id_records, get_all_open_ids, get_open_ids_by_fc,
};
pub use results::{
  get_task_result, get_task_result_shop_codes, get_task_result_shop_codes_for_date,
  get_task_results, get_used_open_ids_for_month, save_retried_task_result, save_task_result,
  save_task_results,
};
pub use schema::{init_db, reconcile_daily_task_progress_for_task};
pub use shops::{
  delete_all_shops, get_all_shops, get_shop_count_by_fc_and_type, get_shops_by_codes,
  get_shops_by_fc, get_shops_by_fc_and_type, import_shops, update_shop_type_by_codes,
};
pub use tasks::{
  add_monthly_task, delete_monthly_task, find_monthly_tasks_by_month_fc_course,
  get_all_daily_tasks_for_task, get_all_monthly_tasks, get_daily_task,
  get_first_pending_daily_task, pause_running_daily_tasks_for_task,
  reschedule_unfinished_daily_tasks, save_daily_task, update_daily_task_run_status,
};
