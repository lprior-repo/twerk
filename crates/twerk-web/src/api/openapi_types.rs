use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use twerk_core::job::{Job, JobSummary};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MessageResponse {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StatusResponse {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum CreateJobResponse {
    Summary(JobSummary),
    Job(Job),
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TriggerErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_id: Option<String>,
}
