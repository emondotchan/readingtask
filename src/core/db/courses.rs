use rusqlite::params;

use super::context::{DbContext, get_conn};
use crate::core::error::AppError;
use crate::core::model::CourseRecord;

pub fn get_all_courses(db: &DbContext) -> Result<Vec<CourseRecord>, AppError> {
  let conn = get_conn(db)?;
  let mut stmt = conn
    .prepare("SELECT month, course_id, task_type FROM courses")
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  let courses = stmt
    .query_map([], |row| {
      Ok(CourseRecord {
        month: row.get(0)?,
        course_id: row.get(1)?,
        task_type: row.get(2)?,
      })
    })
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(courses)
}

pub fn add_or_update_course(
  db: &DbContext,
  previous: Option<(&str, &str, &str)>,
  course: &CourseRecord,
) -> Result<(), AppError> {
  let mut conn = get_conn(db)?;
  let tx = conn
    .transaction()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  if let Some((prev_month, prev_course_id, prev_task_type)) = previous
    && (prev_month != course.month
      || prev_course_id != course.course_id
      || prev_task_type != course.task_type)
  {
    tx.execute(
      "DELETE FROM courses WHERE month = ?1 AND course_id = ?2 AND task_type = ?3",
      params![prev_month, prev_course_id, prev_task_type],
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  }

  tx.execute(
    "INSERT OR REPLACE INTO courses (month, course_id, task_type) VALUES (?1, ?2, ?3)",
    params![course.month, course.course_id, course.task_type],
  )
  .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;

  tx.commit()
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}

pub fn delete_course(
  db: &DbContext,
  month: &str,
  course_id: &str,
  task_type: &str,
) -> Result<(), AppError> {
  let conn = get_conn(db)?;
  conn
    .execute(
      "DELETE FROM courses WHERE month = ?1 AND course_id = ?2 AND task_type = ?3",
      params![month, course_id, task_type],
    )
    .map_err(|e| AppError::ResourceUnavailableError(e.to_string()))?;
  Ok(())
}
