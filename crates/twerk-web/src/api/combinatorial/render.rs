use super::generator::CombinatorialGenerator;
use super::model::InputVariation;

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

    test_cases
        .into_iter()
        .fold(output, |mut rendered, test_case| {
            let test_name = format!(
                "test_{}_{}_{}",
                test_case
                    .operation_id
                    .replace(['-', ' '], "_")
                    .to_lowercase(),
                test_case.method.to_lowercase(),
                test_case.input_variation
            );
            rendered.push_str(&format!(
                r#"#[tokio::test]
async fn {}() {{
    let test_case = TestCase {{
        endpoint: "{}",
        method: "{}",
        content_type: Some("{}"),
        operation_id: "{}",
        description: "{}",
        input_variation: InputVariation::{},
    }};
    let _ = test_case;
}}
"#,
                test_name,
                test_case.endpoint,
                test_case.method,
                test_case
                    .content_type
                    .as_deref()
                    .map_or("none", std::convert::identity),
                test_case.operation_id,
                test_case.description,
                variant_name(&test_case.input_variation)
            ));
            rendered
        })
}

fn variant_name(variation: &InputVariation) -> &'static str {
    match variation {
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
