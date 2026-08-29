use chrono::{Local, Timelike};
use std::collections::HashSet;
use tempfile::tempdir;

use crate::core::db::{
  add_monthly_task, add_open_id, get_daily_task, import_shops, init_db_context, save_daily_task,
};
use crate::core::error::AppError;
use crate::core::executor::archive::archive_quick_run_results_for_date;
use crate::core::executor::planner::{
  calculate_monthly_target_bounds, calculate_sleep_secs_at, is_daily_task_completed,
  parse_reading_url,
};
use crate::core::executor::runner::{
  run_daily_task_with_progress_controlled, validate_retry_task_result,
};
use crate::core::executor::selector::{
  GENERATED_OPEN_ID_LEN, GENERATED_OPEN_ID_PREFIX, generate_open_id, select_open_ids,
};
use crate::core::model::{
  AppPaths, DailyTask, MonthlyTask, OpenIdRecord, SHOP_TYPE_KLORANE, ShopRecord, TaskItemOutcome,
  TaskItemResult, TaskRunRequest,
};

fn open_id_record(fc_name: &str, open_id: &str) -> OpenIdRecord {
  OpenIdRecord {
    fc_name: fc_name.to_string(),
    open_id: open_id.to_string(),
  }
}

#[tokio::test]
async fn incomplete_daily_task_can_resume_after_monthly_threshold_reached() {
  let temp_dir = tempdir().expect("create temp dir");
  let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
  let db = init_db_context(&paths).expect("init db");
  let shops = (0..11)
    .map(|index| ShopRecord {
      province: "province".to_string(),
      city: "city".to_string(),
      shop_code: format!("shop-{index:02}"),
      shop_name: String::new(),
      fc: Some("fc-a".to_string()),
      shop_type: SHOP_TYPE_KLORANE,
    })
    .collect::<Vec<_>>();
  import_shops(&db, &shops).expect("import shops");
  add_open_id(&db, &open_id_record("fc-a", "open-id-1")).expect("add open id");

  let task = MonthlyTask {
    id: "task-1".to_string(),
    fc_name: "fc-a".to_string(),
    s_manager_id: "manager-1".to_string(),
    s_course_id: "course-1".to_string(),
    reading_url: String::new(),
    task_type: "Klorane".to_string(),
    total_target: 11,
    target_days: 2,
    created_at: "2026-08-01 00:00:00".to_string(),
    shopcodes: shops.iter().map(|shop| shop.shop_code.clone()).collect(),
    excluded_open_ids: Vec::new(),
  };
  add_monthly_task(&db, &task).expect("add task");
  save_daily_task(
    &db,
    &DailyTask {
      task_id: task.id.clone(),
      date: "2026-08-01".to_string(),
      target_count: 10,
      completed_count: 10,
      is_locked: true,
      run_status: "completed".to_string(),
      shopcodes: shops[..10]
        .iter()
        .map(|shop| shop.shop_code.clone())
        .collect(),
    },
  )
  .expect("save completed day");
  save_daily_task(
    &db,
    &DailyTask {
      task_id: task.id.clone(),
      date: "2026-08-02".to_string(),
      target_count: 1,
      completed_count: 0,
      is_locked: false,
      run_status: "paused".to_string(),
      shopcodes: vec![shops[10].shop_code.clone()],
    },
  )
  .expect("save interrupted day");

  let error = run_daily_task_with_progress_controlled(&db, &task.id, "2026-08-02", |_| {}, || true)
    .await
    .expect_err("the pause signal should stop the resumed task before requests are sent");

  assert!(
    matches!(error, AppError::Paused(_)),
    "unexpected error: {error:?}"
  );
  let pending = get_daily_task(&db, &task.id, "2026-08-02")
    .expect("load interrupted day")
    .expect("interrupted day should still exist");
  assert_eq!(pending.target_count, 1);
  assert_eq!(pending.completed_count, 0);
  assert_eq!(pending.run_status, "paused");
}

#[test]
fn retry_validation_accepts_only_failed_records_with_complete_request_data() {
  let failed = TaskItemResult {
    result_id: Some(1),
    index: 1,
    executed_date: Some("2026-08-16 20:07:31".to_string()),
    submit_err: None,
    rtn_msg: None,
    read_id: None,
    open_id: "open-id-1".to_string(),
    shop_code: "shop-01".to_string(),
    province: "province".to_string(),
    city: "city".to_string(),
    http_status: None,
    response_text: Some("请求失败".to_string()),
    outcome: TaskItemOutcome::RequestError,
  };

  assert!(validate_retry_task_result(&failed).is_ok());

  let successful = TaskItemResult {
    submit_err: Some(0),
    outcome: TaskItemOutcome::Success,
    ..failed.clone()
  };
  assert!(matches!(
    validate_retry_task_result(&successful),
    Err(AppError::ValidationError(_))
  ));

  let missing_open_id = TaskItemResult {
    open_id: String::new(),
    ..failed
  };
  assert!(matches!(
    validate_retry_task_result(&missing_open_id),
    Err(AppError::ValidationError(_))
  ));
}

#[test]
fn quick_run_archive_credits_results_to_planned_days_by_shopcode() {
  let temp_dir = tempdir().expect("create temp dir");
  let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
  let db = init_db_context(&paths).expect("init db");
  let task = MonthlyTask {
    id: "2608:70:course-1:Klorane".to_string(),
    fc_name: "fc-a".to_string(),
    s_manager_id: "manager-1".to_string(),
    s_course_id: "course-1".to_string(),
    reading_url: String::new(),
    task_type: "Klorane".to_string(),
    total_target: 4,
    target_days: 2,
    created_at: "2026-08-01 00:00:00".to_string(),
    shopcodes: vec![
      "A".to_string(),
      "B".to_string(),
      "C".to_string(),
      "D".to_string(),
    ],
    excluded_open_ids: Vec::new(),
  };
  add_monthly_task(&db, &task).expect("add task");
  for (date, shopcodes) in [
    ("2026-08-10", vec!["A".to_string(), "B".to_string()]),
    ("2026-08-11", vec!["C".to_string(), "D".to_string()]),
  ] {
    save_daily_task(
      &db,
      &DailyTask {
        task_id: task.id.clone(),
        date: date.to_string(),
        target_count: 2,
        completed_count: 0,
        is_locked: false,
        run_status: "not_started".to_string(),
        shopcodes,
      },
    )
    .expect("save daily plan");
  }
  let request = TaskRunRequest {
    s_course_id: task.s_course_id.clone(),
    s_manager_id: task.s_manager_id.clone(),
    reading_url: String::new(),
    fc: task.fc_name.clone(),
    count: 2,
    shopcodes: vec!["B".to_string(), "C".to_string()],
  };
  let items = [
    TaskItemResult {
      result_id: None,
      index: 1,
      executed_date: None,
      submit_err: Some(0),
      rtn_msg: Some("提交成功".to_string()),
      read_id: Some("read-b".to_string()),
      open_id: "open-b".to_string(),
      shop_code: "B".to_string(),
      province: "P".to_string(),
      city: "C".to_string(),
      http_status: Some(200),
      response_text: None,
      outcome: TaskItemOutcome::Success,
    },
    TaskItemResult {
      result_id: None,
      index: 2,
      executed_date: None,
      submit_err: Some(-1),
      rtn_msg: Some("提交失败".to_string()),
      read_id: None,
      open_id: "open-c".to_string(),
      shop_code: "C".to_string(),
      province: "P".to_string(),
      city: "C".to_string(),
      http_status: Some(200),
      response_text: None,
      outcome: TaskItemOutcome::RequestError,
    },
  ];

  archive_quick_run_results_for_date(&db, &request, &items, "2026-08-27")
    .expect("archive quick run");

  assert_eq!(
    get_daily_task(&db, &task.id, "2026-08-10")
      .expect("load first plan")
      .expect("first plan")
      .completed_count,
    1
  );
  assert_eq!(
    get_daily_task(&db, &task.id, "2026-08-11")
      .expect("load second plan")
      .expect("second plan")
      .completed_count,
    1
  );
  assert!(
    get_daily_task(&db, &task.id, "2026-08-27")
      .expect("load archive date")
      .is_none(),
    "quick-run archive must not create an empty daily plan"
  );
}

#[test]
fn parse_reading_url_accepts_direct_course_page_link() {
  let link = parse_reading_url(
    "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID=65&UID=BAS-00868",
  )
  .expect("direct course page link should parse");

  assert_eq!(link.s_course_id, "65");
  assert_eq!(link.s_manager_id, "BAS-00868");
  assert_eq!(
    link.referer,
    "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID=65&UID=BAS-00868"
  );
}

#[test]
fn parse_reading_url_accepts_redirect_uri_link() {
  let link = parse_reading_url(
    "https://open.weixin.qq.com/connect/oauth2/authorize?redirect_uri=https%3A%2F%2Fe-learning.eau-thermale-avene.cn%2FCommon%2FQCSCoursePage.aspx%3FCourseID%3D65%26UID%3DBAS-00868",
  )
  .expect("redirect uri link should parse");

  assert_eq!(link.s_course_id, "65");
  assert_eq!(link.s_manager_id, "BAS-00868");
  assert_eq!(
    link.referer,
    "https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID=65&UID=BAS-00868"
  );
}

#[test]
fn generated_open_id_uses_required_format() {
  let open_id = generate_open_id();

  assert!(open_id.starts_with(GENERATED_OPEN_ID_PREFIX));
  assert_eq!(open_id.len(), GENERATED_OPEN_ID_LEN);
}

#[test]
fn select_open_ids_generates_missing_count() {
  let open_ids = vec![
    open_id_record("fc-a", "existing-open-id"),
    open_id_record("fc-b", "other-fc-open-id"),
  ];
  let used_open_ids = HashSet::new();
  let excluded_open_ids = HashSet::new();

  let selected = select_open_ids(open_ids, "fc-a", &used_open_ids, &excluded_open_ids, 3)
    .expect("open ids should be selected");

  assert_eq!(selected.len(), 3);
  assert!(selected.contains(&"existing-open-id".to_string()));
  assert!(!selected.contains(&"other-fc-open-id".to_string()));
  assert_eq!(
    selected
      .iter()
      .filter(|open_id| open_id.starts_with(GENERATED_OPEN_ID_PREFIX))
      .count(),
    2
  );
  assert!(
    selected
      .iter()
      .filter(|open_id| open_id.starts_with(GENERATED_OPEN_ID_PREFIX))
      .all(|open_id| open_id.len() == GENERATED_OPEN_ID_LEN)
  );
}

#[test]
fn test_calculate_monthly_target_bounds() {
  // Avene: 70% ~ 75%
  let (min, max) = calculate_monthly_target_bounds(100, "Avene");
  assert_eq!(min, 70);
  assert_eq!(max, 75);

  // Klorane: 85% ~ 95%
  let (min, max) = calculate_monthly_target_bounds(100, "Klorane");
  assert_eq!(min, 85);
  assert_eq!(max, 95);

  // Other: 100% ~ 100%
  let (min, max) = calculate_monthly_target_bounds(100, "Other");
  assert_eq!(min, 100);
  assert_eq!(max, 100);
}

#[test]
fn test_dynamic_daily_task_creation_and_multiday_flow() {
  let temp_dir = tempdir().expect("create temp dir");
  let paths = AppPaths::new_with_db_path(temp_dir.path().join("test.sqlite"));
  let db = init_db_context(&paths).expect("init db");
  let shops = (0..35)
    .map(|index| ShopRecord {
      province: "P".to_string(),
      city: "C".to_string(),
      shop_code: format!("S{index:02}"),
      shop_name: String::new(),
      fc: Some("FC1".to_string()),
      shop_type: SHOP_TYPE_KLORANE,
    })
    .collect::<Vec<_>>();
  import_shops(&db, &shops).expect("import shops");
  let task = MonthlyTask {
    id: "task-dynamic-1".to_string(),
    fc_name: "FC1".to_string(),
    s_manager_id: "m-1".to_string(),
    s_course_id: "c-1".to_string(),
    reading_url: String::new(),
    task_type: "Klorane".to_string(),
    total_target: 35,
    target_days: 0,
    created_at: "2026-08-01 00:00:00".to_string(),
    shopcodes: shops.iter().map(|s| s.shop_code.clone()).collect(),
    excluded_open_ids: Vec::new(),
  };
  add_monthly_task(&db, &task).expect("add monthly task");

  // Day 1 allocation: should allocate 15..=25 shops
  let day1 = crate::core::executor::runner::ensure_daily_task(&db, &task, "2026-08-01")
    .expect("allocate day 1");
  assert!(day1.target_count >= 15 && day1.target_count <= 25);
  assert_eq!(day1.shopcodes.len(), day1.target_count);

  // Simulate Day 1 only completing 5 shops
  for shop_code in &day1.shopcodes[..5] {
    let item = TaskItemResult {
      result_id: None,
      index: 1,
      executed_date: Some("2026-08-01 10:00:00".to_string()),
      submit_err: Some(0),
      rtn_msg: None,
      read_id: Some("rid".to_string()),
      open_id: "oid".to_string(),
      shop_code: shop_code.clone(),
      province: "P".to_string(),
      city: "C".to_string(),
      http_status: Some(200),
      response_text: None,
      outcome: TaskItemOutcome::Success,
    };
    crate::core::db::save_task_result(&db, &task.id, &item).expect("save result");
  }
  let mut day1_progress = day1;
  day1_progress.completed_count = 5;
  day1_progress.run_status = "paused".to_string();
  save_daily_task(&db, &day1_progress).expect("save day 1 progress");

  // Day 2 allocation: should allocate from remaining 30 unused shops
  let day2 = crate::core::executor::runner::ensure_daily_task(&db, &task, "2026-08-02")
    .expect("allocate day 2");
  assert!(day2.target_count >= 15 && day2.target_count <= 25);
  assert_eq!(day2.shopcodes.len(), day2.target_count);
  for shop_code in &day2.shopcodes {
    assert!(
      !day1_progress.shopcodes[..5].contains(shop_code),
      "Day 2 must not contain shops completed on Day 1"
    );
  }
}

#[test]
fn test_is_daily_task_completed_uses_target_count_only() {
  let task = DailyTask {
    task_id: "task-1".to_string(),
    date: "2026-07-06".to_string(),
    target_count: 10,
    completed_count: 9,
    is_locked: true,
    run_status: "completed".to_string(),
    shopcodes: Vec::new(),
  };

  assert!(!is_daily_task_completed(&task));

  let task = DailyTask {
    completed_count: 10,
    is_locked: false,
    run_status: "not_started".to_string(),
    ..task
  };

  assert!(is_daily_task_completed(&task));
}

#[test]
fn test_calculate_sleep_secs_at() {
  let now = Local::now()
    .with_hour(20)
    .and_then(|time| time.with_minute(0))
    .and_then(|time| time.with_second(0))
    .and_then(|time| time.with_nanosecond(0))
    .unwrap();

  let sleep = calculate_sleep_secs_at(now, 10);
  assert_eq!(sleep, 360);

  let after_deadline = now.with_hour(21).unwrap();
  let sleep = calculate_sleep_secs_at(after_deadline, 10);
  assert_eq!(sleep, 0);
}

#[test]
fn select_open_ids_generates_when_all_existing_are_unavailable() {
  let open_ids = vec![
    open_id_record("fc-a", "used-open-id"),
    open_id_record("fc-a", "excluded-open-id"),
    open_id_record("fc-b", "other-fc-open-id"),
  ];
  let used_open_ids = HashSet::from(["used-open-id".to_string()]);
  let excluded_open_ids = HashSet::from(["excluded-open-id".to_string()]);

  let selected = select_open_ids(open_ids, "fc-a", &used_open_ids, &excluded_open_ids, 2)
    .expect("open ids should be generated");

  assert_eq!(selected.len(), 2);
  assert!(selected.iter().all(|open_id| {
    open_id.starts_with(GENERATED_OPEN_ID_PREFIX)
      && open_id.len() == GENERATED_OPEN_ID_LEN
      && !used_open_ids.contains(open_id)
      && !excluded_open_ids.contains(open_id)
  }));
}
