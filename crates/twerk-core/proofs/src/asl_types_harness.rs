use twerk_core::asl::types::{Expression, ImageRef, JsonPath, ShellScript, StateName, VariableName};

// ---------------------------------------------------------------------------
// StateName
// ---------------------------------------------------------------------------

#[kani::proof]
fn state_name_rejects_empty() {
    let result = StateName::new("");
    assert!(result.is_err(), "Empty StateName should be rejected");
}

#[kani::proof]
fn state_name_serde_roundtrip() {
    let name = StateName::new("MyState").unwrap();
    let serialized = serde_json::to_string(&name).unwrap();
    let deserialized: StateName = serde_json::from_str(&serialized).unwrap();
    assert_eq!(name.as_str(), deserialized.as_str());
}

// ---------------------------------------------------------------------------
// Expression
// ---------------------------------------------------------------------------

#[kani::proof]
fn expression_rejects_empty() {
    let result = Expression::new("");
    assert!(result.is_err(), "Empty Expression should be rejected");
}

#[kani::proof]
fn expression_serde_roundtrip() {
    let expr = Expression::new("$.value > 0").unwrap();
    let serialized = serde_json::to_string(&expr).unwrap();
    let deserialized: Expression = serde_json::from_str(&serialized).unwrap();
    assert_eq!(expr.as_str(), deserialized.as_str());
}

// ---------------------------------------------------------------------------
// JsonPath
// ---------------------------------------------------------------------------

#[kani::proof]
fn json_path_rejects_empty() {
    let result = JsonPath::new("");
    assert!(result.is_err(), "Empty JsonPath should be rejected");
}

#[kani::proof]
fn json_path_rejects_missing_dollar() {
    let result = JsonPath::new("foo.bar");
    assert!(
        result.is_err(),
        "JsonPath without leading '$' should be rejected"
    );
}

#[kani::proof]
fn json_path_serde_roundtrip() {
    let path = JsonPath::new("$.foo.bar").unwrap();
    let serialized = serde_json::to_string(&path).unwrap();
    let deserialized: JsonPath = serde_json::from_str(&serialized).unwrap();
    assert_eq!(path.as_str(), deserialized.as_str());
}

// ---------------------------------------------------------------------------
// VariableName
// ---------------------------------------------------------------------------

#[kani::proof]
fn variable_name_rejects_empty() {
    let result = VariableName::new("");
    assert!(result.is_err(), "Empty VariableName should be rejected");
}

#[kani::proof]
fn variable_name_rejects_starting_digit() {
    let result = VariableName::new("123abc");
    assert!(
        result.is_err(),
        "VariableName starting with a digit should be rejected"
    );
}

#[kani::proof]
fn variable_name_serde_roundtrip() {
    let name = VariableName::new("my_var").unwrap();
    let serialized = serde_json::to_string(&name).unwrap();
    let deserialized: VariableName = serde_json::from_str(&serialized).unwrap();
    assert_eq!(name.as_str(), deserialized.as_str());
}

// ---------------------------------------------------------------------------
// ImageRef
// ---------------------------------------------------------------------------

#[kani::proof]
fn image_ref_rejects_empty() {
    let result = ImageRef::new("");
    assert!(result.is_err(), "Empty ImageRef should be rejected");
}

#[kani::proof]
fn image_ref_rejects_whitespace() {
    let result = ImageRef::new("hello world");
    assert!(
        result.is_err(),
        "ImageRef containing whitespace should be rejected"
    );
}

#[kani::proof]
fn image_ref_serde_roundtrip() {
    let image = ImageRef::new("ubuntu:latest").unwrap();
    let serialized = serde_json::to_string(&image).unwrap();
    let deserialized: ImageRef = serde_json::from_str(&serialized).unwrap();
    assert_eq!(image.as_str(), deserialized.as_str());
}

// ---------------------------------------------------------------------------
// ShellScript
// ---------------------------------------------------------------------------

#[kani::proof]
fn shell_script_rejects_empty() {
    let result = ShellScript::new("");
    assert!(result.is_err(), "Empty ShellScript should be rejected");
}

#[kani::proof]
fn shell_script_serde_roundtrip() {
    let script = ShellScript::new("echo hello").unwrap();
    let serialized = serde_json::to_string(&script).unwrap();
    let deserialized: ShellScript = serde_json::from_str(&serialized).unwrap();
    assert_eq!(script.as_str(), deserialized.as_str());
}
