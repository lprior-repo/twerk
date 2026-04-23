use twerk_core::eval::transform::{sanitize_expr, transform_operators};

// ---------------------------------------------------------------------------
// sanitize_expr strips braces
// ---------------------------------------------------------------------------

#[kani::proof]
fn sanitize_expr_strips_braces() {
    assert_eq!(sanitize_expr("{{ x }}"), "x");
}

#[kani::proof]
fn sanitize_expr_strips_braces_no_inner_spaces() {
    assert_eq!(sanitize_expr("{{x}}"), "x");
}

#[kani::proof]
fn sanitize_expr_no_braces_returns_trimmed() {
    assert_eq!(sanitize_expr("  x  "), "x");
}

#[kani::proof]
fn sanitize_expr_empty_braces() {
    assert_eq!(sanitize_expr("{{}}"), "");
}

// ---------------------------------------------------------------------------
// transform_operators: and -> &&
// ---------------------------------------------------------------------------

#[kani::proof]
fn sanitize_expr_transforms_and() {
    assert_eq!(sanitize_expr("a and b"), "a && b");
}

#[kani::proof]
fn transform_operators_and() {
    assert_eq!(transform_operators("x and y"), "x && y");
}

// ---------------------------------------------------------------------------
// transform_operators: or -> ||
// ---------------------------------------------------------------------------

#[kani::proof]
fn sanitize_expr_transforms_or() {
    assert_eq!(sanitize_expr("a or b"), "a || b");
}

#[kani::proof]
fn transform_operators_or() {
    assert_eq!(transform_operators("x or y"), "x || y");
}

// ---------------------------------------------------------------------------
// transform_operators: combined and + or
// ---------------------------------------------------------------------------

#[kani::proof]
fn sanitize_expr_transforms_and_or_combined() {
    assert_eq!(sanitize_expr("a and b or c"), "a && b || c");
}

#[kani::proof]
fn transform_operators_combined() {
    assert_eq!(
        transform_operators("foo and bar or baz"),
        "foo && bar || baz"
    );
}

// ---------------------------------------------------------------------------
// sanitize_expr idempotent
// ---------------------------------------------------------------------------

#[kani::proof]
fn sanitize_expr_idempotent_simple() {
    let expr = "x && y || z";
    let first = sanitize_expr(expr);
    let second = sanitize_expr(&first);
    assert_eq!(first, second, "sanitize_expr should be idempotent");
}

#[kani::proof]
fn sanitize_expr_idempotent_with_braces() {
    let expr = "{{ x and y }}";
    let first = sanitize_expr(expr);
    let second = sanitize_expr(&first);
    assert_eq!(first, second, "sanitize_expr should be idempotent after brace removal");
}

#[kani::proof]
fn transform_operators_idempotent() {
    let expr = "a && b || c";
    let first = transform_operators(expr);
    let second = transform_operators(&first);
    assert_eq!(first, second, "transform_operators should be idempotent");
}

// ---------------------------------------------------------------------------
// transform_operators: no change when no operators present
// ---------------------------------------------------------------------------

#[kani::proof]
fn transform_operators_no_change() {
    assert_eq!(transform_operators("x + y"), "x + y");
    assert_eq!(transform_operators("hello"), "hello");
    assert_eq!(transform_operators(""), "");
}

// ---------------------------------------------------------------------------
// sanitize_expr: complex expression with braces and operators
// ---------------------------------------------------------------------------

#[kani::proof]
fn sanitize_expr_complex() {
    assert_eq!(sanitize_expr("{{ foo and bar or baz }}"), "foo && bar || baz");
}
