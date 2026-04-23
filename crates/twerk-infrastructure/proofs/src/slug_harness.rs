//! Kani proof harnesses for `runtime::podman::slug::make`.
//!
//! Covers:
//! - `make`: output is lowercase
//! - `make`: output only contains [a-z0-9_-]

use twerk_infrastructure::runtime::podman::slug;

#[kani::proof]
fn slug_make_lowercase() {
    let input = "Hello WORLD Foo";
    let result = slug::make(input);
    assert!(
        result.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'),
        "slug::make output must be lowercase (or digits / dashes / underscores): got '{result}'"
    );
}

#[kani::proof]
fn slug_make_only_valid_chars() {
    let input = "foo@bar!baz#qux$test_value";
    let result = slug::make(input);
    assert!(
        result.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'),
        "slug::make output must only contain [a-z0-9_-]: got '{result}'"
    );
}

#[kani::proof]
fn slug_make_empty_input() {
    let result = slug::make("");
    assert!(result.is_empty(), "empty input should produce empty output");
}

#[kani::proof]
fn slug_make_spaces_become_dashes() {
    let result = slug::make("hello world");
    assert_eq!(result, "hello-world");
}

#[kani::proof]
fn slug_make_preserves_underscores() {
    let result = slug::make("hello_world");
    assert_eq!(result, "hello_world");
}
