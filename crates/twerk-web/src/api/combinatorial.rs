//! Combinatorial test generator from `OpenAPI` specification.
//!
//! Generates exhaustive tests covering: endpoint x method x content-type x input-variation
//!
//! # Design
//!
//! This module reads an `OpenAPI` 3.0 specification and produces a comprehensive
//! test matrix that systematically covers all combinations of:
//! - Endpoints (paths)
//! - HTTP methods
//! - Content types (application/json, text/yaml, etc.)
//! - Input variations (valid, invalid, boundary values)

use itertools::Itertools;
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
    pub properties: HashMap<String, Schema>,
    #[serde(default)]
    pub items: Option<Box<Schema>>,
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
    fn get_type(&self) -> Option<&str> {
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
    pub auth_state: AuthState,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AuthState {
    NoAuth,
    BasicAuth,
    KeyAuth,
}

impl std::fmt::Display for AuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthState::NoAuth => write!(f, "no_auth"),
            AuthState::BasicAuth => write!(f, "basic_auth"),
            AuthState::KeyAuth => write!(f, "key_auth"),
        }
    }
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputVariation::ValidMinimal => write!(f, "valid_minimal"),
            InputVariation::ValidFull => write!(f, "valid_full"),
            InputVariation::InvalidEmpty => write!(f, "invalid_empty"),
            InputVariation::InvalidMalformed => write!(f, "invalid_malformed"),
            InputVariation::InvalidMissingRequired => write!(f, "invalid_missing_required"),
            InputVariation::InvalidBoundaryMin => write!(f, "invalid_boundary_min"),
            InputVariation::InvalidBoundaryMax => write!(f, "invalid_boundary_max"),
            InputVariation::InvalidEnum => write!(f, "invalid_enum"),
        }
    }
}

pub struct CombinatorialGenerator {
    spec: OpenApiSpec,
}

impl CombinatorialGenerator {
    #[must_use]
    pub fn from_spec(spec: OpenApiSpec) -> Self {
        Self { spec }
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let spec: OpenApiSpec = serde_json::from_str(json)?;
        Ok(Self { spec })
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn load_spec(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        let spec: OpenApiSpec = serde_json::from_str(&json)?;
        Ok(Self { spec })
    }

    #[must_use]
    pub fn generate_test_matrix(&self) -> Vec<TestCase> {
        let mut test_cases = Vec::new();

        let auth_states = vec![AuthState::NoAuth, AuthState::BasicAuth, AuthState::KeyAuth];

        for (path, path_item) in &self.spec.paths {
            let methods = [
                ("GET", &path_item.get),
                ("POST", &path_item.post),
                ("PUT", &path_item.put),
                ("DELETE", &path_item.delete),
                ("PATCH", &path_item.patch),
            ];

            for (method, operation) in methods {
                if let Some(op) = operation {
                    let content_types = Self::extract_content_types(op);
                    let variations = Self::generate_input_variations(op);

                    for (content_type, variation) in
                        content_types.into_iter().cartesian_product(variations)
                    {
                        let operation_id = op.operation_id.clone().unwrap_or_else(|| {
                            format!(
                                "{}_{method}",
                                path.replace('/', "_"),
                                method = method.to_lowercase()
                            )
                        });

                        for auth_state in &auth_states {
                            test_cases.push(TestCase {
                                endpoint: path.clone(),
                                method: method.to_string(),
                                content_type: Some(content_type.clone()),
                                operation_id: operation_id.clone(),
                                description: op.summary.clone().unwrap_or_default(),
                                input_variation: variation.clone(),
                                auth_state: auth_state.clone(),
                            });
                        }
                    }
                }
            }
        }

        test_cases
    }

    fn extract_content_types(operation: &Operation) -> Vec<String> {
        let mut types = vec!["application/json".to_string()];

        if let Some(ref body) = operation.request_body {
            for content_type in body.content.keys() {
                if !types.contains(content_type) {
                    types.push(content_type.clone());
                }
            }
        }

        types
    }

    fn generate_input_variations(operation: &Operation) -> Vec<InputVariation> {
        let mut variations = vec![InputVariation::ValidMinimal, InputVariation::ValidFull];

        if let Some(ref body) = operation.request_body {
            let has_required = body
                .content
                .values()
                .any(|mt| mt.schema.as_ref().is_some_and(|s| !s.required.is_empty()));

            if has_required {
                variations.push(InputVariation::InvalidMissingRequired);
            }

            variations.push(InputVariation::InvalidMalformed);
            variations.push(InputVariation::InvalidEmpty);
        }

        if let Some(ref body) = operation.request_body {
            let mut has_string_field = false;
            let mut has_explicit_bounds = false;
            let mut has_enum = false;

            for mt in body.content.values() {
                if let Some(ref schema) = mt.schema {
                    Self::analyze_schema(
                        schema,
                        &mut has_string_field,
                        &mut has_explicit_bounds,
                        &mut has_enum,
                    );
                }
            }

            if has_explicit_bounds || has_string_field {
                variations.push(InputVariation::InvalidBoundaryMin);
                variations.push(InputVariation::InvalidBoundaryMax);
            }
            if has_enum {
                variations.push(InputVariation::InvalidEnum);
            }
        }

        variations
    }

    fn analyze_schema(
        schema: &Schema,
        has_string_field: &mut bool,
        has_explicit_bounds: &mut bool,
        has_enum: &mut bool,
    ) {
        if schema.get_type() == Some("string") {
            *has_string_field = true;
        }
        if schema.minimum.is_some()
            || schema.maximum.is_some()
            || schema.min_length.is_some()
            || schema.max_length.is_some()
        {
            *has_explicit_bounds = true;
        }
        if schema.enum_values.is_some() {
            *has_enum = true;
        }
        for prop in schema.properties.values() {
            Self::analyze_schema(prop, has_string_field, has_explicit_bounds, has_enum);
        }
        if let Some(ref items) = schema.items {
            Self::analyze_schema(items, has_string_field, has_explicit_bounds, has_enum);
        }
    }

    #[must_use]
    pub fn test_case_count(&self) -> usize {
        self.generate_test_matrix().len()
    }

    #[must_use]
    pub fn get_endpoints(&self) -> Vec<String> {
        self.spec.paths.keys().cloned().collect()
    }

    #[must_use]
    pub fn get_methods_for_endpoint(&self, path: &str) -> Vec<String> {
        self.spec
            .paths
            .get(path)
            .map(|pi| {
                let mut methods = Vec::new();
                if pi.get.is_some() {
                    methods.push("GET".to_string());
                }
                if pi.post.is_some() {
                    methods.push("POST".to_string());
                }
                if pi.put.is_some() {
                    methods.push("PUT".to_string());
                }
                if pi.delete.is_some() {
                    methods.push("DELETE".to_string());
                }
                if pi.patch.is_some() {
                    methods.push("PATCH".to_string());
                }
                methods
            })
            .unwrap_or_default()
    }

    #[must_use]
    pub fn spec_info(&self) -> (&str, &str) {
        (&self.spec.info.title, &self.spec.info.version)
    }
}

#[allow(clippy::format_push_string)]
#[must_use]
pub fn generate_test_module(generator: &CombinatorialGenerator) -> String {
    let test_cases = generator.generate_test_matrix();
    let (title, version) = generator.spec_info();

    let mut output =
        format!("//! Auto-generated combinatorial tests from OpenAPI spec: {title} v{version}\n",);
    output.push_str("//!\n");
    output.push_str("//! This file is auto-generated. Do not edit manually.\n");
    output.push_str("//! Regenerate with: combinatorial_test_generator\n\n");
    output.push_str("#![allow(clippy::unwrap_used)]\n\n");
    output.push_str("use super::*;\n\n");

    for tc in &test_cases {
        let test_name = format!(
            "test_{}_{}_{}_{}",
            tc.operation_id.replace(['-', ' '], "_").to_lowercase(),
            tc.method.to_lowercase(),
            tc.input_variation,
            tc.auth_state
        );

        output.push_str(&format!(
            r#"#[tokio::test]
async fn {}() {{
    let test_case = TestCase {{
        endpoint: "{}",
        method: "{}",
        content_type: Some("{}"),
        operation_id: "{}",
        description: "{}",
        input_variation: InputVariation::{},
        auth_state: AuthState::{},
    }};
    // Test implementation would go here
    let _ = test_case;
}}
"#,
            test_name,
            tc.endpoint,
            tc.method,
            tc.content_type.as_deref().unwrap_or("none"),
            tc.operation_id,
            tc.description,
            variant_name(&tc.input_variation),
            auth_variant_name(&tc.auth_state)
        ));
    }

    output
}

fn variant_name(v: &InputVariation) -> &'static str {
    match v {
        InputVariation::ValidMinimal => "ValidMinimal",
        InputVariation::ValidFull => "ValidFull",
        InputVariation::InvalidEmpty => "InvalidEmpty",
        InputVariation::InvalidMalformed => "InvalidMalformed",
        InputVariation::InvalidMissingRequired => "InvalidMissingRequired",
        InputVariation::InvalidBoundaryMin => "InvalidBoundaryMin",
        InputVariation::InvalidBoundaryMax => "InvalidBoundaryMax",
        InputVariation::InvalidEnum => "InvalidEnum",
    }
}

fn auth_variant_name(a: &AuthState) -> &'static str {
    match a {
        AuthState::NoAuth => "NoAuth",
        AuthState::BasicAuth => "BasicAuth",
        AuthState::KeyAuth => "KeyAuth",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_spec() {
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

        let gen = CombinatorialGenerator::from_json(json).unwrap();
        assert_eq!(gen.spec_info(), ("Test", "1.0"));
    }

    #[test]
    fn test_generate_test_matrix() {
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

        let gen = CombinatorialGenerator::from_json(json).unwrap();
        let cases = gen.generate_test_matrix();

        assert_eq!(cases.len(), 21);
        assert!(cases.iter().all(|tc| tc.endpoint == "/jobs"));
        assert!(cases.iter().all(|tc| tc.method == "POST"));

        let auth_states: Vec<_> = cases.iter().map(|tc| &tc.auth_state).unique().collect();
        assert_eq!(auth_states.len(), 3);
        assert!(auth_states.contains(&&AuthState::NoAuth));
        assert!(auth_states.contains(&&AuthState::BasicAuth));
        assert!(auth_states.contains(&&AuthState::KeyAuth));
    }

    #[test]
    fn test_content_type_extraction() {
        let json = r#"{
            "openapi": "3.0.3",
            "info": {"title": "Test", "version": "1.0"},
            "paths": {
                "/jobs": {
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

        let gen = CombinatorialGenerator::from_json(json).unwrap();
        let cases = gen.generate_test_matrix();

        let content_types: Vec<_> = cases
            .iter()
            .filter_map(|tc| tc.content_type.clone())
            .unique()
            .collect();

        assert!(content_types.contains(&"application/json".to_string()));
        assert!(content_types.contains(&"text/yaml".to_string()));
    }

    #[test]
    fn test_multiple_endpoints() {
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
                        "responses": {"200": {"description": "OK"}}
                    }
                }
            }
        }"#;

        let gen = CombinatorialGenerator::from_json(json).unwrap();
        let endpoints = gen.get_endpoints();

        assert_eq!(endpoints.len(), 2);
        assert!(endpoints.contains(&"/health".to_string()));
        assert!(endpoints.contains(&"/jobs".to_string()));

        let job_methods = gen.get_methods_for_endpoint("/jobs");
        assert_eq!(job_methods.len(), 2);
        assert!(job_methods.contains(&"GET".to_string()));
        assert!(job_methods.contains(&"POST".to_string()));
    }
}
