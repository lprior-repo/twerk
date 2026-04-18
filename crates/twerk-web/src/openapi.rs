//! OpenAPI 3.0 struct definitions for serializing and deserializing API specs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level OpenAPI 3.0 document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApi {
    pub openapi: String,
    pub info: Info,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub paths: HashMap<String, PathItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<Components>,
}

/// OpenAPI info object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    pub title: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A path item holding operations per HTTP method.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PathItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub get: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub put: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<Operation>,
}

/// A single API operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<Parameter>,
    #[serde(default, rename = "requestBody", skip_serializing_if = "Option::is_none")]
    pub request_body: Option<RequestBody>,
    #[serde(default)]
    pub responses: HashMap<String, Response>,
}

/// An operation parameter (path, query, header).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "in")]
    pub location: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<SchemaRef>,
}

/// Reference to a schema — either a `$ref` string or an inline schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SchemaRef {
    Ref { #[serde(rename = "$ref")] reference: String },
    Schema(Schema),
}

/// An inline or component schema definition.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Schema {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub schema_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, SchemaRef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<SchemaRef>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_properties: Option<Box<AdditionalProperties>>,
}

/// Value for `additionalProperties` — either a bool or a nested schema ref.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AdditionalProperties {
    Bool(bool),
    SchemaRef(SchemaRef),
}

/// A request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub content: HashMap<String, MediaType>,
}

/// A media type with an optional schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaType {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<SchemaRef>,
}

/// An HTTP response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub description: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub content: HashMap<String, MediaType>,
}

/// The components object holding reusable schemas.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Components {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub schemas: HashMap<String, SchemaRef>,
}

impl OpenApi {
    pub fn new(title: &str, version: &str) -> Self {
        Self {
            openapi: "3.0.3".to_string(),
            info: Info {
                title: title.to_string(),
                version: version.to_string(),
                description: None,
            },
            paths: HashMap::new(),
            components: None,
        }
    }

    pub fn with_paths(mut self, paths: HashMap<String, PathItem>) -> Self {
        self.paths = paths;
        self
    }

    pub fn with_components(mut self, components: Components) -> Self {
        self.components = Some(components);
        self
    }
}

/// Build the OpenAPI spec for all twerk-web routes.
pub fn create_openapi_spec(version: &str) -> OpenApi {
    let mut paths = HashMap::new();

    paths.insert(
        "/health".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("healthCheck".to_string()),
                summary: Some("Health check".to_string()),
                parameters: vec![],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "OK".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/tasks/{id}".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("getTask".to_string()),
                summary: Some("Get task by ID".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([
                    (
                        "200".to_string(),
                        Response {
                            description: "Task found".to_string(),
                            content: HashMap::new(),
                        },
                    ),
                    (
                        "404".to_string(),
                        Response {
                            description: "Not Found".to_string(),
                            content: HashMap::new(),
                        },
                    ),
                ]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/tasks/{id}/log".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("getTaskLog".to_string()),
                summary: Some("Get task log".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Task log".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/jobs".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("listJobs".to_string()),
                summary: Some("List jobs".to_string()),
                parameters: vec![],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "List of jobs".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            post: Some(Operation {
                operation_id: Some("createJob".to_string()),
                summary: Some("Create job".to_string()),
                parameters: vec![],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Job created".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/jobs/{id}".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("getJob".to_string()),
                summary: Some("Get job by ID".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Job found".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/jobs/{id}/log".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("getJobLog".to_string()),
                summary: Some("Get job log".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Job log".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/jobs/{id}/cancel".to_string(),
        PathItem {
            put: Some(Operation {
                operation_id: Some("cancelJob".to_string()),
                summary: Some("Cancel job".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Job cancelled".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            post: Some(Operation {
                operation_id: Some("cancelJobPost".to_string()),
                summary: Some("Cancel job".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Job cancelled".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/jobs/{id}/restart".to_string(),
        PathItem {
            put: Some(Operation {
                operation_id: Some("restartJob".to_string()),
                summary: Some("Restart job".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Job restarted".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/scheduled-jobs".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("listScheduledJobs".to_string()),
                summary: Some("List scheduled jobs".to_string()),
                parameters: vec![],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "List of scheduled jobs".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            post: Some(Operation {
                operation_id: Some("createScheduledJob".to_string()),
                summary: Some("Create scheduled job".to_string()),
                parameters: vec![],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Scheduled job created".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/scheduled-jobs/{id}".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("getScheduledJob".to_string()),
                summary: Some("Get scheduled job by ID".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([
                    (
                        "200".to_string(),
                        Response {
                            description: "Scheduled job found".to_string(),
                            content: HashMap::new(),
                        },
                    ),
                    (
                        "404".to_string(),
                        Response {
                            description: "Not Found".to_string(),
                            content: HashMap::new(),
                        },
                    ),
                ]),
            }),
            delete: Some(Operation {
                operation_id: Some("deleteScheduledJob".to_string()),
                summary: Some("Delete scheduled job".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Scheduled job deleted".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/scheduled-jobs/{id}/pause".to_string(),
        PathItem {
            put: Some(Operation {
                operation_id: Some("pauseScheduledJob".to_string()),
                summary: Some("Pause scheduled job".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Scheduled job paused".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/scheduled-jobs/{id}/resume".to_string(),
        PathItem {
            put: Some(Operation {
                operation_id: Some("resumeScheduledJob".to_string()),
                summary: Some("Resume scheduled job".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Scheduled job resumed".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/queues".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("listQueues".to_string()),
                summary: Some("List queues".to_string()),
                parameters: vec![],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "List of queues".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/queues/{name}".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("getQueue".to_string()),
                summary: Some("Get queue by name".to_string()),
                parameters: vec![Parameter {
                    name: "name".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Queue found".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            delete: Some(Operation {
                operation_id: Some("deleteQueue".to_string()),
                summary: Some("Delete queue".to_string()),
                parameters: vec![Parameter {
                    name: "name".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Queue deleted".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/nodes".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("listNodes".to_string()),
                summary: Some("List nodes".to_string()),
                parameters: vec![],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "List of nodes".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/metrics".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("getMetrics".to_string()),
                summary: Some("Get metrics".to_string()),
                parameters: vec![],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Metrics data".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/users".to_string(),
        PathItem {
            post: Some(Operation {
                operation_id: Some("createUser".to_string()),
                summary: Some("Create user".to_string()),
                parameters: vec![],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "User created".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/api/v1/triggers".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("listTriggers".to_string()),
                summary: Some("List triggers".to_string()),
                parameters: vec![],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "List of triggers".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            post: Some(Operation {
                operation_id: Some("createTrigger".to_string()),
                summary: Some("Create trigger".to_string()),
                parameters: vec![],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Trigger created".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    paths.insert(
        "/api/v1/triggers/{id}".to_string(),
        PathItem {
            get: Some(Operation {
                operation_id: Some("getTrigger".to_string()),
                summary: Some("Get trigger by ID".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Trigger found".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            put: Some(Operation {
                operation_id: Some("updateTrigger".to_string()),
                summary: Some("Update trigger".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Trigger updated".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            delete: Some(Operation {
                operation_id: Some("deleteTrigger".to_string()),
                summary: Some("Delete trigger".to_string()),
                parameters: vec![Parameter {
                    name: "id".to_string(),
                    location: "path".to_string(),
                    required: Some(true),
                    schema: Some(SchemaRef::Schema(Schema {
                        schema_type: Some("string".to_string()),
                        ..Default::default()
                    })),
                }],
                request_body: None,
                responses: HashMap::from([(
                    "200".to_string(),
                    Response {
                        description: "Trigger deleted".to_string(),
                        content: HashMap::new(),
                    },
                )]),
            }),
            ..Default::default()
        },
    );

    OpenApi::new("Twerk API", version).with_paths(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_roundtrips_through_json() {
        let spec = create_openapi_spec("0.1.0");
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: OpenApi = serde_json::from_str(&json).unwrap();
        assert_eq!(spec.openapi, deserialized.openapi);
        assert_eq!(spec.info.title, deserialized.info.title);
        assert_eq!(spec.paths.len(), deserialized.paths.len());
    }

    #[test]
    fn spec_contains_all_endpoints() {
        let spec = create_openapi_spec("0.1.0");
        let expected = [
            "/health",
            "/tasks/{id}",
            "/tasks/{id}/log",
            "/jobs",
            "/jobs/{id}",
            "/jobs/{id}/log",
            "/jobs/{id}/cancel",
            "/jobs/{id}/restart",
            "/scheduled-jobs",
            "/scheduled-jobs/{id}",
            "/scheduled-jobs/{id}/pause",
            "/scheduled-jobs/{id}/resume",
            "/queues",
            "/queues/{name}",
            "/nodes",
            "/metrics",
            "/users",
            "/api/v1/triggers",
            "/api/v1/triggers/{id}",
        ];
        for path in &expected {
            assert!(spec.paths.contains_key(*path), "missing path: {path}");
        }
    }

    #[test]
    fn path_parameters_have_correct_location() {
        let spec = create_openapi_spec("0.1.0");
        let task_path = &spec.paths["/tasks/{id}"].get.as_ref().unwrap();
        let param = &task_path.parameters[0];
        assert_eq!(param.name, "id");
        assert_eq!(param.location, "path");
        assert_eq!(param.required, Some(true));
    }

    #[test]
    fn schema_ref_deserializes_dollar_ref() {
        let json = r##"{"$ref": "#/components/schemas/Task"}"##;
        let schema_ref: SchemaRef = serde_json::from_str(json).unwrap();
        match schema_ref {
            SchemaRef::Ref { reference } => {
                assert_eq!(reference, "#/components/schemas/Task");
            }
            SchemaRef::Schema(_) => panic!("expected Ref variant"),
        }
    }

    #[test]
    fn schema_ref_deserializes_inline_schema() {
        let json = r#"{"type": "string", "format": "uuid"}"#;
        let schema_ref: SchemaRef = serde_json::from_str(json).unwrap();
        match schema_ref {
            SchemaRef::Schema(s) => {
                assert_eq!(s.schema_type.as_deref(), Some("string"));
                assert_eq!(s.format.as_deref(), Some("uuid"));
            }
            SchemaRef::Ref { .. } => panic!("expected Schema variant"),
        }
    }
}
