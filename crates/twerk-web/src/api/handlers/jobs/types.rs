use serde::Deserialize;

/// Whether the create-job endpoint should block until the job completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, utoipa::ToSchema)]
pub enum WaitMode {
    #[default]
    Detached,
    Blocking,
}

impl<'de> Deserialize<'de> for WaitMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum WaitModeHelper {
            Bool(bool),
            String(String),
        }

        match WaitModeHelper::deserialize(deserializer)? {
            WaitModeHelper::Bool(true) => Ok(Self::Blocking),
            WaitModeHelper::Bool(false) => Ok(Self::Detached),
            WaitModeHelper::String(value) => match value.to_lowercase().as_str() {
                "blocking" | "true" | "1" | "yes" => Ok(Self::Blocking),
                _ => Ok(Self::Detached),
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default, utoipa::ToSchema)]
pub struct CreateJobQuery {
    pub wait: Option<WaitMode>,
}
