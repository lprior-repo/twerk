//! Task log part record types and conversions to domain types.

use sqlx::FromRow;

use crate::datastore::Error as DatastoreError;
use twerk_core::{id::TaskId, task::TaskLogPart};

/// Task log part record from the database
#[derive(Debug, Clone, FromRow)]
pub struct TaskLogPartRecord {
    pub id: String,
    pub number_: i64,
    pub task_id: String,
    pub created_at: time::OffsetDateTime,
    pub contents: String,
}

/// Extension trait for TaskLogPartRecord conversions
pub trait TaskLogPartRecordExt {
    /// Converts the database record to a `TaskLogPart` domain object.
    fn to_task_log_part(&self) -> Result<TaskLogPart, DatastoreError>;
}

impl TaskLogPartRecordExt for TaskLogPartRecord {
    fn to_task_log_part(&self) -> Result<TaskLogPart, DatastoreError> {
        Ok(TaskLogPart {
            id: Some(self.id.clone()),
            number: self.number_,
            task_id: Some(TaskId::new(self.task_id.clone())?),
            contents: Some(self.contents.clone()),
            created_at: Some(self.created_at),
        })
    }
}


