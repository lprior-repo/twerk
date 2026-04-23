use itertools::Itertools;

use super::model::{InputVariation, OpenApiSpec, Operation, Schema, TestCase};

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
        serde_json::from_str(json).map(Self::from_spec)
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn load_spec(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&json)?)
    }

    #[must_use]
    pub fn generate_test_matrix(&self) -> Vec<TestCase> {
        self.spec
            .paths
            .iter()
            .flat_map(|(path, path_item)| {
                [
                    ("GET", &path_item.get),
                    ("POST", &path_item.post),
                    ("PUT", &path_item.put),
                    ("DELETE", &path_item.delete),
                    ("PATCH", &path_item.patch),
                ]
                .into_iter()
                .filter_map(move |(method, operation)| {
                    operation.as_ref().map(|op| (path, method, op))
                })
            })
            .flat_map(|(path, method, operation)| {
                let operation_id = operation.operation_id.as_ref().map_or_else(
                    || format!("{}_{}", path.replace('/', "_"), method.to_lowercase()),
                    std::clone::Clone::clone,
                );
                let description = operation
                    .summary
                    .as_ref()
                    .map_or_else(String::new, std::clone::Clone::clone);
                Self::extract_content_types(operation)
                    .into_iter()
                    .cartesian_product(Self::generate_input_variations(operation))
                    .map(move |(content_type, input_variation)| TestCase {
                        endpoint: path.clone(),
                        method: method.to_string(),
                        content_type: Some(content_type),
                        operation_id: operation_id.clone(),
                        description: description.clone(),
                        input_variation,
                    })
            })
            .collect()
    }

    fn extract_content_types(operation: &Operation) -> Vec<String> {
        let mut types = vec!["application/json".to_string()];
        if let Some(body) = &operation.request_body {
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
        if let Some(body) = &operation.request_body {
            let has_required = body.content.values().any(|media_type| {
                media_type
                    .schema
                    .as_ref()
                    .is_some_and(|schema| !schema.required.is_empty())
            });
            if has_required {
                variations.push(InputVariation::InvalidMissingRequired);
            }
            variations.extend([
                InputVariation::InvalidMalformed,
                InputVariation::InvalidEmpty,
            ]);

            let (mut has_string_field, mut has_explicit_bounds, mut has_enum) =
                (false, false, false);
            for media_type in body.content.values() {
                if let Some(schema) = &media_type.schema {
                    Self::analyze_schema(
                        schema,
                        &mut has_string_field,
                        &mut has_explicit_bounds,
                        &mut has_enum,
                    );
                }
            }

            if has_explicit_bounds || has_string_field {
                variations.extend([
                    InputVariation::InvalidBoundaryMin,
                    InputVariation::InvalidBoundaryMax,
                ]);
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
        if let Some(items) = &schema.items {
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
            .map_or_else(Vec::new, |path_item| {
                [
                    ("GET", path_item.get.is_some()),
                    ("POST", path_item.post.is_some()),
                    ("PUT", path_item.put.is_some()),
                    ("DELETE", path_item.delete.is_some()),
                    ("PATCH", path_item.patch.is_some()),
                ]
                .into_iter()
                .filter(|(_, present)| *present)
                .map(|(method, _)| method.to_string())
                .collect()
            })
    }

    #[must_use]
    pub fn spec_info(&self) -> (&str, &str) {
        (&self.spec.info.title, &self.spec.info.version)
    }
}
