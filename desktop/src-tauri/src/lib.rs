mod bootstrap;
mod commands;
mod error;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  reading_task::init_logging();

  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .setup(|app| {
      log::info!(target: "reading_task::tauri::setup", "loading desktop runtime state");
      let runtime_paths = bootstrap::initialize(app)?;
      if let Some(db_path) = &runtime_paths.db_path {
        log::info!(
          target: "reading_task::tauri::setup",
          "sqlite configured db_path={}",
          db_path.display()
        );
      } else {
        log::info!(target: "reading_task::tauri::setup", "sqlite not configured yet");
      }
      app.manage(commands::RuntimeState::new(runtime_paths));
      app.manage(commands::TaskPauseRegistry::default());
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      commands::get_runtime_status,
      commands::set_sqlite_path,
      commands::run_reading_task,
      commands::get_open_ids,
      commands::add_open_id,
      commands::delete_open_id,
      commands::import_open_ids_csv,
      commands::get_shops,
      commands::add_or_update_shop,
      commands::delete_shop,
      commands::get_fcs,
      commands::add_or_update_fc,
      commands::delete_fc,
      commands::get_monthly_tasks,
      commands::preview_monthly_task_plan,
      commands::create_monthly_task,
      commands::delete_monthly_task,
      commands::get_daily_progress,
      commands::run_daily_task,
      commands::pause_daily_task,
      commands::get_task_results,
      commands::get_shop_count,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
