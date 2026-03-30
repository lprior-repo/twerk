//! Task log part record types and conversions to domain types.

use sqlx::FromRow;

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
    #[must_use]
    fn to_task_log_part(&self) -> TaskLogPart;
}

impl TaskLogPartRecordExt for TaskLogPartRecord {
    fn to_task_log_part(&self) -> TaskLogPart {
        TaskLogPart {
            id: Some(self.id.clone()),
            number: self.number_,
            task_id: Some(TaskId::new(self.task_id.clone())),
            contents: Some(self.contents.clone()),
            created_at: Some(self.created_at),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ──────────────────────────────────────────────────────────

    /// Creates a fixed-point timestamp for deterministic tests.
    fn fixed_now() -> time::OffsetDateTime {
        time::OffsetDateTime::new_utc(
            time::Date::from_calendar_date(2026, time::Month::March, 22).unwrap_or_else(|_| {
                time::Date::from_calendar_date(2026, time::Month::January, 1).unwrap()
            }),
            time::Time::from_hms(12, 0, 0).unwrap_or(time::Time::MIDNIGHT),
        )
    }

    // ── TaskLogPartRecord → TaskLogPart conversion tests ────────────────

    #[test]
    fn task_log_part_record_to_task_log_part() {
        let now = fixed_now();
        let record = TaskLogPartRecord {
            id: "log-001".to_string(),
            number_: 1,
            task_id: "task-001".to_string(),
            created_at: now,
            contents: "line 1\nline 2\n".to_string(),
        };
        let part = record.to_task_log_part();

        assert_eq!(part.id.as_deref(), Some("log-001"));
        assert_eq!(part.number, 1);
        assert_eq!(part.task_id.as_deref(), Some("task-001"));
        assert_eq!(part.contents.as_deref(), Some("line 1\nline 2\n"));
        assert!(part.created_at.is_some());
    }
}
