use tempfile::tempdir;

use super::*;
use crate::core::db::context::get_conn;
use crate::core::model::{
  AppPaths, DailyTask, OpenIdRecord, SHOP_TYPE_AVENE, SHOP_TYPE_AVENE_KLORANE, SHOP_TYPE_KLORANE,
  ShopRecord, TaskItemOutcome, TaskItemResult,
};

#[test]
fn import_shops_saves_shop_name_and_delete_all_clears_records() {
  let temp_dir = tempdir().expect("create temp dir");
  let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
  let db = init_db_context(&paths).expect("init db");
  let shops = vec![ShopRecord {
    province: "广东".to_string(),
    city: "广州".to_string(),
    shop_code: "1001".to_string(),
    shop_name: "广州天河店".to_string(),
    fc: Some("FC1".to_string()),
    shop_type: 1,
  }];

  import_shops(&db, &shops).expect("import shops");
  assert_eq!(get_all_shops(&db).expect("get shops"), shops);

  delete_all_shops(&db).expect("delete all shops");
  assert!(get_all_shops(&db).expect("get shops").is_empty());
}

#[test]
fn update_shop_type_by_codes_updates_existing_shops_only() {
  let temp_dir = tempdir().expect("create temp dir");
  let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
  let db = init_db_context(&paths).expect("init db");
  let shops = vec![
    ShopRecord {
      province: "广东".to_string(),
      city: "广州".to_string(),
      shop_code: "1001".to_string(),
      shop_name: "门店1".to_string(),
      fc: Some("FC1".to_string()),
      shop_type: 1,
    },
    ShopRecord {
      province: "广东".to_string(),
      city: "深圳".to_string(),
      shop_code: "1002".to_string(),
      shop_name: "门店2".to_string(),
      fc: Some("FC1".to_string()),
      shop_type: 1,
    },
  ];

  import_shops(&db, &shops).expect("import shops");
  update_shop_type_by_codes(
    &db,
    &["1001".to_string(), "9999".to_string()],
    SHOP_TYPE_AVENE_KLORANE,
  )
  .expect("update shop types");

  let shops = get_all_shops(&db).expect("get shops");
  let shop1 = shops.iter().find(|s| s.shop_code == "1001").unwrap();
  let shop2 = shops.iter().find(|s| s.shop_code == "1002").unwrap();
  assert_eq!(shop1.shop_type, SHOP_TYPE_AVENE_KLORANE);
  assert_eq!(shop2.shop_type, 1);
}

#[test]
fn test_get_first_pending_daily_task_and_reschedule() {
  let temp_dir = tempdir().expect("create temp dir");
  let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
  let db = init_db_context(&paths).expect("init db");

  save_daily_task(
    &db,
    &DailyTask {
      task_id: "task-1".to_string(),
      date: "2026-08-11".to_string(),
      target_count: 23,
      completed_count: 23,
      is_locked: true,
      run_status: "completed".to_string(),
      shopcodes: vec!["S1".to_string()],
    },
  )
  .expect("save day 1");

  save_daily_task(
    &db,
    &DailyTask {
      task_id: "task-1".to_string(),
      date: "2026-08-12".to_string(),
      target_count: 15,
      completed_count: 14,
      is_locked: false,
      run_status: "paused".to_string(),
      shopcodes: vec!["S2".to_string(), "S3".to_string()],
    },
  )
  .expect("save day 2");

  save_daily_task(
    &db,
    &DailyTask {
      task_id: "task-1".to_string(),
      date: "2026-08-13".to_string(),
      target_count: 16,
      completed_count: 0,
      is_locked: false,
      run_status: "not_started".to_string(),
      shopcodes: vec!["S4".to_string()],
    },
  )
  .expect("save day 3");

  let first_pending = get_first_pending_daily_task(&db, "task-1")
    .expect("get first pending")
    .expect("exists");
  assert_eq!(first_pending.date, "2026-08-12");
  assert_eq!(first_pending.completed_count, 14);

  let rescheduled =
    reschedule_unfinished_daily_tasks(&db, "task-1", "2026-08-18").expect("reschedule unfinished");
  assert_eq!(rescheduled.len(), 3);
  assert_eq!(rescheduled[0].date, "2026-08-11");
  assert_eq!(rescheduled[0].completed_count, 23);

  assert_eq!(rescheduled[1].date, "2026-08-18");
  assert_eq!(rescheduled[1].completed_count, 14);
  assert_eq!(rescheduled[1].shopcodes, vec!["S2", "S3"]);

  assert_eq!(rescheduled[2].date, "2026-08-19");
  assert_eq!(rescheduled[2].completed_count, 0);
  assert_eq!(rescheduled[2].shopcodes, vec!["S4"]);

  // Check that old rows for 08-12 and 08-13 are no longer present
  assert!(
    get_daily_task(&db, "task-1", "2026-08-12")
      .unwrap()
      .is_none()
  );
  assert!(
    get_daily_task(&db, "task-1", "2026-08-13")
      .unwrap()
      .is_none()
  );
}

#[test]
fn pause_running_daily_tasks_for_task_only_pauses_running_rows() {
  let temp_dir = tempdir().expect("create temp dir");
  let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
  let db = init_db_context(&paths).expect("init db");

  let day1 = DailyTask {
    task_id: "task-1".to_string(),
    date: "2026-08-01".to_string(),
    target_count: 5,
    completed_count: 5,
    is_locked: true,
    shopcodes: vec!["1001".to_string()],
    run_status: "completed".to_string(),
  };
  let day2 = DailyTask {
    task_id: "task-1".to_string(),
    date: "2026-08-02".to_string(),
    target_count: 5,
    completed_count: 2,
    is_locked: false,
    shopcodes: vec!["1002".to_string()],
    run_status: "running".to_string(),
  };
  let other_day = DailyTask {
    task_id: "task-2".to_string(),
    date: "2026-08-02".to_string(),
    target_count: 5,
    completed_count: 2,
    is_locked: false,
    shopcodes: vec!["1003".to_string()],
    run_status: "running".to_string(),
  };

  save_daily_task(&db, &day1).expect("save day1");
  save_daily_task(&db, &day2).expect("save day2");
  save_daily_task(&db, &other_day).expect("save other day");

  let updated = pause_running_daily_tasks_for_task(&db, "task-1").expect("pause running tasks");
  assert_eq!(updated, 1);

  let task1_day2 = get_daily_task(&db, "task-1", "2026-08-02")
    .expect("load day2")
    .expect("day2 exists");
  assert_eq!(task1_day2.run_status, "paused");

  let task1_day1 = get_daily_task(&db, "task-1", "2026-08-01")
    .expect("load day1")
    .expect("day1 exists");
  assert_eq!(task1_day1.run_status, "completed");

  let task2_day = get_daily_task(&db, "task-2", "2026-08-02")
    .expect("load task2 day")
    .expect("task2 day exists");
  assert_eq!(task2_day.run_status, "running");
}

#[test]
fn test_get_task_result_shop_codes() {
  let temp_dir = tempdir().expect("create temp dir");
  let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
  let db = init_db_context(&paths).expect("init db");

  let item1 = TaskItemResult {
    result_id: None,
    index: 1,
    executed_date: Some("2026-08-18 09:00:00".to_string()),
    submit_err: Some(0),
    rtn_msg: None,
    read_id: None,
    open_id: "open1".to_string(),
    shop_code: "9521".to_string(),
    province: "P1".to_string(),
    city: "C1".to_string(),
    http_status: Some(200),
    response_text: None,
    outcome: TaskItemOutcome::Success,
  };
  let item2 = TaskItemResult {
    result_id: None,
    index: 2,
    executed_date: Some("2026-08-18 10:00:00".to_string()),
    submit_err: Some(0),
    rtn_msg: None,
    read_id: None,
    open_id: "open2".to_string(),
    shop_code: "6709".to_string(),
    province: "P1".to_string(),
    city: "C1".to_string(),
    http_status: Some(200),
    response_text: None,
    outcome: TaskItemOutcome::Success,
  };

  let item1_id = save_task_result(&db, "task-1", &item1).expect("save item 1");
  save_task_result(&db, "task-1", &item2).expect("save item 2");

  let stored_item = get_task_result(&db, "task-1", item1_id)
    .expect("get item 1")
    .expect("item 1 should exist");
  assert_eq!(stored_item.result_id, Some(item1_id));
  assert_eq!(stored_item.open_id, item1.open_id);
  assert_eq!(stored_item.shop_code, item1.shop_code);
  assert!(
    get_task_result(&db, "another-task", item1_id)
      .expect("query another task")
      .is_none()
  );

  let codes = get_task_result_shop_codes(&db, "task-1").expect("get codes");
  assert_eq!(codes.len(), 2);
  assert!(codes.contains("9521"));
  assert!(codes.contains("6709"));
}

#[test]
fn retrying_result_reuses_original_row_when_task_shop_is_unique() {
  let temp_dir = tempdir().expect("create temp dir");
  let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
  let db = init_db_context(&paths).expect("init db");
  get_conn(&db)
    .expect("get connection")
    .execute(
      "CREATE UNIQUE INDEX uk_task_shop ON task_results (task_id, shop_code)",
      [],
    )
    .expect("create legacy unique index");
  let failed = TaskItemResult {
    result_id: None,
    index: 1,
    executed_date: None,
    submit_err: None,
    rtn_msg: None,
    read_id: None,
    open_id: "open-1".to_string(),
    shop_code: "3373".to_string(),
    province: "P1".to_string(),
    city: "C1".to_string(),
    http_status: None,
    response_text: Some("请求失败".to_string()),
    outcome: TaskItemOutcome::RequestError,
  };
  let result_id = save_task_result(&db, "task-1", &failed).expect("save failed result");
  let retried = TaskItemResult {
    submit_err: Some(0),
    response_text: Some("重做成功".to_string()),
    outcome: TaskItemOutcome::Success,
    ..failed
  };

  let saved_id = save_retried_task_result(&db, "task-1", result_id, &retried)
    .expect("retry should update the original row without violating the unique index");

  assert_eq!(saved_id, result_id);
  let stored = get_task_result(&db, "task-1", result_id)
    .expect("load retried result")
    .expect("retried result should exist");
  assert_eq!(stored.outcome, TaskItemOutcome::Success);
}

#[test]
fn test_targeted_shop_and_openid_queries() {
  let temp_dir = tempdir().expect("create temp dir");
  let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
  let db = init_db_context(&paths).expect("init db");

  let shops = vec![
    ShopRecord {
      province: "P1".to_string(),
      city: "C1".to_string(),
      shop_code: "1001".to_string(),
      shop_name: "Shop 1".to_string(),
      fc: Some("FC_A".to_string()),
      shop_type: SHOP_TYPE_AVENE,
    },
    ShopRecord {
      province: "P2".to_string(),
      city: "C2".to_string(),
      shop_code: "1002".to_string(),
      shop_name: "Shop 2".to_string(),
      fc: Some("FC_A".to_string()),
      shop_type: SHOP_TYPE_KLORANE,
    },
    ShopRecord {
      province: "P3".to_string(),
      city: "C3".to_string(),
      shop_code: "1003".to_string(),
      shop_name: "Shop 3".to_string(),
      fc: Some("FC_B".to_string()),
      shop_type: SHOP_TYPE_AVENE_KLORANE,
    },
  ];
  import_shops(&db, &shops).expect("import shops");

  add_open_id(
    &db,
    &OpenIdRecord {
      open_id: "oid_a1".to_string(),
      fc_name: "FC_A".to_string(),
    },
  )
  .expect("add oid 1");
  add_open_id(
    &db,
    &OpenIdRecord {
      open_id: "oid_b1".to_string(),
      fc_name: "FC_B".to_string(),
    },
  )
  .expect("add oid 2");

  // Test get_shops_by_fc_and_type
  let avene_shops_a = get_shops_by_fc_and_type(&db, "FC_A", "Avene").expect("get avene shops a");
  assert_eq!(avene_shops_a.len(), 1);
  assert_eq!(avene_shops_a[0].shop_code, "1001");

  // Test get_shops_by_fc
  let fc_a_shops = get_shops_by_fc(&db, "FC_A").expect("get fc a shops");
  assert_eq!(fc_a_shops.len(), 2);

  // Test get_shops_by_codes
  let codes = vec!["1001".to_string(), "1003".to_string()];
  let queried_shops = get_shops_by_codes(&db, &codes).expect("get by codes");
  assert_eq!(queried_shops.len(), 2);

  // Test get_open_ids_by_fc
  let fc_a_oids = get_open_ids_by_fc(&db, "FC_A").expect("get oids");
  assert_eq!(fc_a_oids.len(), 1);
  assert_eq!(fc_a_oids[0].open_id, "oid_a1");
}
