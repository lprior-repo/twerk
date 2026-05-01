use thiserror::Error;

#[derive(Debug, Error)]
pub struct Timeout {
    pub task_id: String,
}

impl std::fmt::Display for Timeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "task {} timed out", self.task_id)
    }
}

impl PartialEq for Timeout {
    fn eq(&self, other: &Self) -> bool {
        self.task_id == other.task_id
    }
}