use super::core::ApiError;
use crate::api::trigger_api::TriggerUpdateError;

impl From<twerk_infrastructure::datastore::Error> for ApiError {
    fn from(err: twerk_infrastructure::datastore::Error) -> Self {
        match err {
            twerk_infrastructure::datastore::Error::UserNotFound => {
                Self::NotFound("user not found".to_string())
            }
            twerk_infrastructure::datastore::Error::JobNotFound => {
                Self::NotFound("job not found".to_string())
            }
            twerk_infrastructure::datastore::Error::TaskNotFound => {
                Self::NotFound("task not found".to_string())
            }
            twerk_infrastructure::datastore::Error::ScheduledJobNotFound => {
                Self::NotFound("scheduled job not found".to_string())
            }
            twerk_infrastructure::datastore::Error::NodeNotFound => {
                Self::NotFound("node not found".to_string())
            }
            _ => Self::Internal(err.to_string()),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

impl From<twerk_core::id::IdError> for ApiError {
    fn from(err: twerk_core::id::IdError) -> Self {
        Self::Internal(err.to_string())
    }
}

impl From<TriggerUpdateError> for ApiError {
    fn from(err: TriggerUpdateError) -> Self {
        match err {
            TriggerUpdateError::InvalidIdFormat(msg)
            | TriggerUpdateError::UnsupportedContentType(msg)
            | TriggerUpdateError::MalformedJson(msg)
            | TriggerUpdateError::ValidationFailed(msg)
            | TriggerUpdateError::VersionConflict(msg) => Self::BadRequest(msg),
            TriggerUpdateError::IdMismatch { .. } => Self::BadRequest("id mismatch".to_string()),
            TriggerUpdateError::TriggerNotFound(msg) => Self::NotFound(msg),
            TriggerUpdateError::Persistence(msg) | TriggerUpdateError::Serialization(msg) => {
                Self::Internal(msg)
            }
        }
    }
}
