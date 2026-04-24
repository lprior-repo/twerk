use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct OpenApiSpec {
    pub openapi: String,
    pub info: Info,
    pub paths: HashMap<String, PathItem>,
    #[serde(default)]
    pub components: Components,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Info {
    pub title: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathItem {
    #[serde(default)]
    pub get: Option<Operation>,
    #[serde(default)]
    pub post: Option<Operation>,
    #[serde(default)]
    pub put: Option<Operation>,
    #[serde(default)]
    pub delete: Option<Operation>,
    #[serde(default)]
    pub patch: Option<Operation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Operation {
    #[serde(default, rename = "operationId")]
    pub operation_id: Option<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    #[serde(default, rename = "requestBody")]
    pub request_body: Option<RequestBody>,
    pub responses: HashMap<String, Response>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "in")]
    pub location: String,
    pub required: Option<bool>,
    pub schema: Option<Schema>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestBody {
    #[serde(default)]
    pub content: HashMap<String, MediaType>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MediaType {
    pub schema: Option<Schema>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Response {
    pub description: String,
    #[serde(default)]
    pub content: HashMap<String, MediaType>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Components {
    #[serde(default)]
    pub schemas: HashMap<String, Schema>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Schema {
    #[serde(default)]
    pub schema_type: Option<String>,
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub properties: HashMap<String, Self>,
    #[serde(default)]
    pub items: Option<Box<Self>>,
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub minimum: Option<i64>,
    #[serde(default)]
    pub maximum: Option<i64>,
    #[serde(default)]
    pub min_length: Option<u64>,
    #[serde(default)]
    pub max_length: Option<u64>,
    #[serde(default)]
    pub enum_values: Option<Vec<serde_json::Value>>,
}

impl Schema {
    #[must_use]
    pub fn get_type(&self) -> Option<&str> {
        self.schema_type.as_deref().or(self.type_field.as_deref())
    }
}

#[derive(Debug, Clone)]
pub struct TestCase {
    pub endpoint: String,
    pub method: String,
    pub content_type: Option<String>,
    pub operation_id: String,
    pub description: String,
    pub input_variation: InputVariation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputVariation {
    ValidMinimal,
    ValidFull,
    InvalidEmpty,
    InvalidMalformed,
    InvalidMissingRequired,
    InvalidBoundaryMin,
    InvalidBoundaryMax,
    InvalidEnum,
}

impl std::fmt::Display for InputVariation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::ValidMinimal => "valid_minimal",
            Self::ValidFull => "valid_full",
            Self::InvalidEmpty => "invalid_empty",
            Self::InvalidMalformed => "invalid_malformed",
            Self::InvalidMissingRequired => "invalid_missing_required",
            Self::InvalidBoundaryMin => "invalid_boundary_min",
            Self::InvalidBoundaryMax => "invalid_boundary_max",
            Self::InvalidEnum => "invalid_enum",
        };
        write!(formatter, "{label}")
    }
}
