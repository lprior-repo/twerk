use itertools::Itertools;

use super::{generate_test_module, CombinatorialGenerator};

#[test]
fn load_spec_reads_basic_info() {
    let json = r#"{
        "openapi": "3.0.3",
        "info": {"title": "Test", "version": "1.0"},
        "paths": {
            "/health": {
                "get": {
                    "operationId": "healthCheck",
                    "summary": "Health check",
                    "responses": {"200": {"description": "OK"}}
                }
            }
        }
    }"#;

    let generator = CombinatorialGenerator::from_json(json).unwrap();
    assert_eq!(generator.spec_info(), ("Test", "1.0"));
}

#[test]
fn generate_test_matrix_includes_expected_job_cases() {
    let json = r#"{
        "openapi": "3.0.3",
        "info": {"title": "Test", "version": "1.0"},
        "paths": {
            "/jobs": {
                "post": {
                    "operationId": "createJob",
                    "summary": "Create job",
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["name"],
                                    "properties": {"name": {"type": "string"}}
                                }
                            }
                        }
                    },
                    "responses": {"200": {"description": "OK"}}
                }
            }
        }
    }"#;

    let generator = CombinatorialGenerator::from_json(json).unwrap();
    let cases = generator.generate_test_matrix();
    assert_eq!(cases.len(), 7);
    assert!(cases.iter().all(|test_case| test_case.endpoint == "/jobs"));
    assert!(cases.iter().all(|test_case| test_case.method == "POST"));
}

#[test]
fn content_type_and_endpoint_helpers_work() {
    let json = r#"{
        "openapi": "3.0.3",
        "info": {"title": "Test", "version": "1.0"},
        "paths": {
            "/health": {
                "get": {
                    "operationId": "health",
                    "responses": {"200": {"description": "OK"}}
                }
            },
            "/jobs": {
                "get": {
                    "operationId": "listJobs",
                    "responses": {"200": {"description": "OK"}}
                },
                "post": {
                    "operationId": "createJob",
                    "requestBody": {
                        "content": {
                            "application/json": {"schema": {"type": "object"}},
                            "text/yaml": {"schema": {"type": "string"}}
                        }
                    },
                    "responses": {"200": {"description": "OK"}}
                }
            }
        }
    }"#;

    let generator = CombinatorialGenerator::from_json(json).unwrap();
    let content_types: Vec<_> = generator
        .generate_test_matrix()
        .iter()
        .filter_map(|test_case| test_case.content_type.clone())
        .unique()
        .collect();
    assert!(content_types.contains(&"application/json".to_string()));
    assert!(content_types.contains(&"text/yaml".to_string()));

    let endpoints = generator.get_endpoints();
    assert_eq!(endpoints.len(), 2);
    assert!(endpoints.contains(&"/health".to_string()));
    assert!(endpoints.contains(&"/jobs".to_string()));

    let job_methods = generator.get_methods_for_endpoint("/jobs");
    assert_eq!(job_methods.len(), 2);
    assert!(job_methods.contains(&"GET".to_string()));
    assert!(job_methods.contains(&"POST".to_string()));
}

#[test]
fn render_generates_module_text() {
    let json = r#"{
        "openapi": "3.0.3",
        "info": {"title": "Test", "version": "1.0"},
        "paths": {
            "/health": {
                "get": {
                    "operationId": "healthCheck",
                    "summary": "Health check",
                    "responses": {"200": {"description": "OK"}}
                }
            }
        }
    }"#;

    let generator = CombinatorialGenerator::from_json(json).unwrap();
    let module_text = generate_test_module(&generator);
    assert!(module_text.contains("Auto-generated combinatorial tests"));
    assert!(module_text.contains("healthcheck"));
}
