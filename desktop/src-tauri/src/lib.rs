mod bootstrap;
mod commands;
mod error;

use tauri::{
  ActivationPolicy, Manager, WindowEvent,
  menu::{Menu, MenuItem},
  tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "reading-task-tray";
const TRAY_MENU_SHOW_ID: &str = "show-main-window";
const TRAY_MENU_QUIT_ID: &str = "quit-app";

fn show_main_window(app: &tauri::AppHandle) {
  #[cfg(target_os = "macos")]
  let _ = app.set_activation_policy(ActivationPolicy::Regular);

  if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
  }
}

fn hide_main_window(app: &tauri::AppHandle) {
  if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
    let _ = window.hide();
  }

  #[cfg(target_os = "macos")]
  let _ = app.set_activation_policy(ActivationPolicy::Accessory);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  reading_task::init_logging();

  tauri::Builder::default()
    .on_window_event(|window, event| {
      if window.label() != MAIN_WINDOW_LABEL {
        return;
      }

      if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        hide_main_window(&window.app_handle());
      }
    })
    .plugin(tauri_plugin_dialog::init())
    .setup(|app| {
      log::info!(target: "reading_task::tauri::setup", "loading desktop runtime state");
      let runtime_paths = bootstrap::initialize(app)?;
      let db = runtime_paths
        .db_path
        .as_ref()
        .map(|db_path| {
          reading_task::init_db_context(&reading_task::AppPaths::new_with_db_path(db_path.clone()))
        })
        .transpose()?;
      if let Some(db_path) = &runtime_paths.db_path {
        log::info!(
          target: "reading_task::tauri::setup",
          "sqlite configured db_path={}",
          db_path.display()
        );
      } else {
        log::info!(target: "reading_task::tauri::setup", "sqlite not configured yet");
      }
      app.manage(commands::RuntimeState::new(runtime_paths, db));
      app.manage(commands::TaskPauseRegistry::default());

      let show_item = MenuItem::with_id(app, TRAY_MENU_SHOW_ID, "显示主窗口", true, None::<&str>)?;
      let quit_item = MenuItem::with_id(app, TRAY_MENU_QUIT_ID, "退出", true, None::<&str>)?;
      let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;
      let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-icon@2x.png"))
        .map_err(|e| e.to_string())?;

      TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_icon)
        .tooltip("reading_task")
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .icon_as_template(true)
        .on_menu_event(|app, event| match event.id() {
          id if id == TRAY_MENU_SHOW_ID => show_main_window(app),
          id if id == TRAY_MENU_QUIT_ID => app.exit(0),
          _ => {}
        })
        .on_tray_icon_event(|tray, event| {
          if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
          } = event
          {
            show_main_window(tray.app_handle());
          }
        })
        .build(app)?;

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
      commands::get_courses,
      commands::add_or_update_course,
      commands::delete_course,
      commands::get_monthly_tasks,
      commands::preview_monthly_task_plan,
      commands::create_monthly_task,
      commands::delete_monthly_task,
      commands::get_daily_task,
      commands::get_task_daily_tasks,
      commands::save_daily_task,
      commands::run_daily_task,
      commands::pause_daily_task,
      commands::get_task_results,
      commands::get_shop_count,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
