//! Pure calculation types for job state transitions.

/// Represents a job state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStateTransition {
    /// Job is being started
    Start,
    /// Job is being scheduled
    Schedule,
    /// Job is running
    Run,
    /// Job is being cancelled
    Cancel,
    /// Job has completed
    Complete,
    /// Job has failed
    Fail,
    /// Job is being restarted
    Restart,
}

impl std::fmt::Display for JobStateTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStateTransition::Start => write!(f, "START"),
            JobStateTransition::Schedule => write!(f, "SCHEDULE"),
            JobStateTransition::Run => write!(f, "RUN"),
            JobStateTransition::Cancel => write!(f, "CANCEL"),
            JobStateTransition::Complete => write!(f, "COMPLETE"),
            JobStateTransition::Fail => write!(f, "FAIL"),
            JobStateTransition::Restart => write!(f, "RESTART"),
        }
    }
}
