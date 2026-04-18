use serde_json::{json, Value};

fn main() {
    let spec = build_openapi_spec();
    let json = serde_json::to_string_pretty(&spec).unwrap();
    println!("{json}");
}

fn build_openapi_spec() -> Value {
    json!({
        "openapi": "3.0.0",
        "info": {
            "title": "Twerk API",
            "version": "1.0.0",
            "description": "Task scheduling and job queue management API"
        },
        "paths": build_paths(),
        "components": build_components()
    })
}

fn build_paths() -> Value {
    json!({
        "/health": {
            "get": {
                "summary": "Health check",
                "responses": {
                    "200": {
                        "description": "Health check response"
                    }
                }
            }
        },
        "/nodes": {
            "get": {
                "summary": "List active nodes",
                "responses": {
                    "200": {
                        "description": "List of active nodes"
                    }
                }
            }
        },
        "/metrics": {
            "get": {
                "summary": "Get system metrics",
                "responses": {
                    "200": {
                        "description": "System metrics"
                    }
                }
            }
        },
        "/tasks/{id}": {
            "get": {
                "summary": "Get task by ID",
                "parameters": [build_path_param("id", "Task ID")],
                "responses": {
                    "200": {
                        "description": "Task details"
                    }
                }
            }
        },
        "/tasks/{id}/log": {
            "get": {
                "summary": "Get task log",
                "parameters": [build_path_param("id", "Task ID")],
                "responses": {
                    "200": {
                        "description": "Task log"
                    }
                }
            }
        },
        "/queues": {
            "get": {
                "summary": "List queues",
                "responses": {
                    "200": {
                        "description": "List of queues"
                    }
                }
            }
        },
        "/queues/{name}": {
            "get": {
                "summary": "Get queue info",
                "parameters": [build_path_param("name", "Queue name")],
                "responses": {
                    "200": {
                        "description": "Queue info"
                    }
                }
            },
            "delete": {
                "summary": "Delete queue",
                "parameters": [build_path_param("name", "Queue name")],
                "responses": {
                    "200": {
                        "description": "Queue deleted"
                    }
                }
            }
        },
        "/jobs": {
            "get": {
                "summary": "List jobs",
                "responses": {
                    "200": {
                        "description": "List of jobs"
                    }
                }
            },
            "post": {
                "summary": "Create job",
                "responses": {
                    "200": {
                        "description": "Job created"
                    }
                }
            }
        },
        "/jobs/{id}": {
            "get": {
                "summary": "Get job by ID",
                "parameters": [build_path_param("id", "Job ID")],
                "responses": {
                    "200": {
                        "description": "Job details"
                    }
                }
            }
        },
        "/jobs/{id}/cancel": {
            "put": {
                "summary": "Cancel job",
                "parameters": [build_path_param("id", "Job ID")],
                "responses": {
                    "200": {
                        "description": "Job cancelled"
                    }
                }
            }
        },
        "/jobs/{id}/restart": {
            "put": {
                "summary": "Restart job",
                "parameters": [build_path_param("id", "Job ID")],
                "responses": {
                    "200": {
                        "description": "Job restarted"
                    }
                }
            }
        },
        "/jobs/{id}/log": {
            "get": {
                "summary": "Get job log",
                "parameters": [build_path_param("id", "Job ID")],
                "responses": {
                    "200": {
                        "description": "Job log"
                    }
                }
            }
        },
        "/scheduled-jobs": {
            "get": {
                "summary": "List scheduled jobs",
                "responses": {
                    "200": {
                        "description": "List of scheduled jobs"
                    }
                }
            },
            "post": {
                "summary": "Create scheduled job",
                "responses": {
                    "200": {
                        "description": "Scheduled job created"
                    }
                }
            }
        },
        "/scheduled-jobs/{id}": {
            "get": {
                "summary": "Get scheduled job",
                "parameters": [build_path_param("id", "Scheduled job ID")],
                "responses": {
                    "200": {
                        "description": "Scheduled job details"
                    }
                }
            }
        },
        "/scheduled-jobs/{id}/pause": {
            "put": {
                "summary": "Pause scheduled job",
                "parameters": [build_path_param("id", "Scheduled job ID")],
                "responses": {
                    "200": {
                        "description": "Scheduled job paused"
                    }
                }
            }
        },
        "/scheduled-jobs/{id}/resume": {
            "put": {
                "summary": "Resume scheduled job",
                "parameters": [build_path_param("id", "Scheduled job ID")],
                "responses": {
                    "200": {
                        "description": "Scheduled job resumed"
                    }
                }
            }
        },
        "/scheduled-jobs/{id}": {
            "delete": {
                "summary": "Delete scheduled job",
                "parameters": [build_path_param("id", "Scheduled job ID")],
                "responses": {
                    "200": {
                        "description": "Scheduled job deleted"
                    }
                }
            }
        },
        "/api/v1/triggers": {
            "get": {
                "summary": "List triggers",
                "responses": {
                    "200": {
                        "description": "List of triggers"
                    }
                }
            },
            "post": {
                "summary": "Create trigger",
                "responses": {
                    "201": {
                        "description": "Trigger created"
                    }
                }
            }
        },
        "/api/v1/triggers/{id}": {
            "get": {
                "summary": "Get trigger",
                "parameters": [build_path_param("id", "Trigger ID")],
                "responses": {
                    "200": {
                        "description": "Trigger details"
                    }
                }
            },
            "put": {
                "summary": "Update trigger",
                "parameters": [build_path_param("id", "Trigger ID")],
                "responses": {
                    "200": {
                        "description": "Trigger updated"
                    }
                }
            },
            "delete": {
                "summary": "Delete trigger",
                "parameters": [build_path_param("id", "Trigger ID")],
                "responses": {
                    "204": {
                        "description": "Trigger deleted"
                    }
                }
            }
        },
        "/users": {
            "post": {
                "summary": "Create user",
                "responses": {
                    "200": {
                        "description": "User created"
                    }
                }
            }
        }
    })
}

fn build_path_param(name: &str, description: &str) -> Value {
    json!({
        "name": name,
        "in": "path",
        "required": true,
        "schema": {
            "type": "string"
        },
        "description": description
    })
}

fn build_components() -> Value {
    json!({
        "schemas": {}
    })
}
