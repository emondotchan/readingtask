use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;

use crate as reading_task;
use reading_task::CourseRecord;

use crate::web::dto::{DeleteCourseQuery, UpsertCourseInput};
use crate::web::error::CommandError;
use crate::web::state::AppState;
use crate::web::utils::run_blocking_db;

pub async fn get_courses(
  State(state): State<AppState>,
) -> Result<Json<Vec<CourseRecord>>, CommandError> {
  let db = state.resolve_db()?;
  let courses = run_blocking_db(move || reading_task::get_all_courses(&db)).await?;
  Ok(Json(courses))
}

pub async fn add_or_update_course(
  State(state): State<AppState>,
  Json(course): Json<UpsertCourseInput>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  let prev = match (
    course.previous_month.as_deref(),
    course.previous_course_id.as_deref(),
    course.previous_task_type.as_deref(),
  ) {
    (Some(month), Some(course_id), Some(task_type)) => Some((
      month.to_string(),
      course_id.to_string(),
      task_type.to_string(),
    )),
    _ => None,
  };
  run_blocking_db(move || {
    reading_task::add_or_update_course(
      &db,
      prev
        .as_ref()
        .map(|(m, c, t)| (m.as_str(), c.as_str(), t.as_str())),
      &course.course,
    )
  })
  .await?;
  Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_course(
  State(state): State<AppState>,
  Query(query): Query<DeleteCourseQuery>,
) -> Result<StatusCode, CommandError> {
  let db = state.resolve_db()?;
  run_blocking_db(move || {
    reading_task::delete_course(&db, &query.month, &query.course_id, &query.task_type)
  })
  .await?;
  Ok(StatusCode::NO_CONTENT)
}
