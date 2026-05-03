use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct OpenApi {
    pub openapi: String,
    pub info: Info,
    pub paths: std::collections::HashMap<String, PathItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Info {
    pub title: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PathItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub get: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub put: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete: Option<Operation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Operation {
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responses: Option<Responses>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Responses {
    #[serde(rename = "200", skip_serializing_if = "Option::is_none")]
    pub status_200: Option<Response>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub description: String,
}

impl OpenApi {
    pub fn new(title: &str, version: &str) -> Self {
        Self {
            openapi: "3.0.0".to_string(),
            info: Info {
                title: title.to_string(),
                version: version.to_string(),
            },
            paths: std::collections::HashMap::new(),
        }
    }

    pub fn with_paths(mut self, paths: std::collections::HashMap<String, PathItem>) -> Self {
        self.paths = paths;
        self
    }
}

pub fn create_openapi_spec(version: &str) -> OpenApi {
    let mut paths = std::collections::HashMap::new();

    paths.insert(
        "/health".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Health check".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "OK".to_string(),
                    }),
                }),
            }),
            post: None,
            put: None,
            delete: None,
        },
    );

    paths.insert(
        "/tasks/{id}".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get task by ID".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Task found".to_string(),
                    }),
                }),
            }),
            post: None,
            put: None,
            delete: None,
        },
    );

    paths.insert(
        "/tasks/{id}/log".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get task log".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Task log".to_string(),
                    }),
                }),
            }),
            post: None,
            put: None,
            delete: None,
        },
    );

    paths.insert(
        "/jobs".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "List jobs".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "List of jobs".to_string(),
                    }),
                }),
            }),
            post: Some(Operation {
                summary: "Create job".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Job created".to_string(),
                    }),
                }),
            }),
            put: None,
            delete: None,
        },
    );

    paths.insert(
        "/jobs/{id}".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get job by ID".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Job found".to_string(),
                    }),
                }),
            }),
            post: None,
            put: None,
            delete: None,
        },
    );

    paths.insert(
        "/jobs/{id}/log".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get job log".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Job log".to_string(),
                    }),
                }),
            }),
            post: None,
            put: None,
            delete: None,
        },
    );

    paths.insert(
        "/jobs/{id}/cancel".to_string(),
        PathItem {
            get: None,
            post: Some(Operation {
                summary: "Cancel job".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Job cancelled".to_string(),
                    }),
                }),
            }),
            put: Some(Operation {
                summary: "Cancel job".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Job cancelled".to_string(),
                    }),
                }),
            }),
            delete: None,
        },
    );

    paths.insert(
        "/jobs/{id}/restart".to_string(),
        PathItem {
            get: None,
            post: None,
            put: Some(Operation {
                summary: "Restart job".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Job restarted".to_string(),
                    }),
                }),
            }),
            delete: None,
        },
    );

    paths.insert(
        "/scheduled-jobs".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "List scheduled jobs".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "List of scheduled jobs".to_string(),
                    }),
                }),
            }),
            post: Some(Operation {
                summary: "Create scheduled job".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Scheduled job created".to_string(),
                    }),
                }),
            }),
            put: None,
            delete: None,
        },
    );

    paths.insert(
        "/scheduled-jobs/{id}".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get scheduled job by ID".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Scheduled job found".to_string(),
                    }),
                }),
            }),
            post: None,
            put: None,
            delete: Some(Operation {
                summary: "Delete scheduled job".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Scheduled job deleted".to_string(),
                    }),
                }),
            }),
        },
    );

    paths.insert(
        "/scheduled-jobs/{id}/pause".to_string(),
        PathItem {
            get: None,
            post: None,
            put: Some(Operation {
                summary: "Pause scheduled job".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Scheduled job paused".to_string(),
                    }),
                }),
            }),
            delete: None,
        },
    );

    paths.insert(
        "/scheduled-jobs/{id}/resume".to_string(),
        PathItem {
            get: None,
            post: None,
            put: Some(Operation {
                summary: "Resume scheduled job".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Scheduled job resumed".to_string(),
                    }),
                }),
            }),
            delete: None,
        },
    );

    paths.insert(
        "/queues".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "List queues".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "List of queues".to_string(),
                    }),
                }),
            }),
            post: None,
            put: None,
            delete: None,
        },
    );

    paths.insert(
        "/queues/{name}".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get queue by name".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Queue found".to_string(),
                    }),
                }),
            }),
            post: None,
            put: None,
            delete: Some(Operation {
                summary: "Delete queue".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Queue deleted".to_string(),
                    }),
                }),
            }),
        },
    );

    paths.insert(
        "/nodes".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "List nodes".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "List of nodes".to_string(),
                    }),
                }),
            }),
            post: None,
            put: None,
            delete: None,
        },
    );

    paths.insert(
        "/metrics".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get metrics".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Metrics data".to_string(),
                    }),
                }),
            }),
            post: None,
            put: None,
            delete: None,
        },
    );

    paths.insert(
        "/users".to_string(),
        PathItem {
            get: None,
            post: Some(Operation {
                summary: "Create user".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "User created".to_string(),
                    }),
                }),
            }),
            put: None,
            delete: None,
        },
    );

    paths.insert(
        "/triggers".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "List triggers".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "List of triggers".to_string(),
                    }),
                }),
            }),
            post: Some(Operation {
                summary: "Create trigger".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Trigger created".to_string(),
                    }),
                }),
            }),
            put: None,
            delete: None,
        },
    );

    paths.insert(
        "/triggers/{id}".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get trigger by ID".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Trigger found".to_string(),
                    }),
                }),
            }),
            post: None,
            put: Some(Operation {
                summary: "Update trigger".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Trigger updated".to_string(),
                    }),
                }),
            }),
            delete: Some(Operation {
                summary: "Delete trigger".to_string(),
                responses: Some(Responses {
                    status_200: Some(Response {
                        description: "Trigger deleted".to_string(),
                    }),
                }),
            }),
        },
    );

    OpenApi::new("Twerk API", version).with_paths(paths)
}
