use twerk_compiler::{
    parse_expression, BinaryOp, CompilerError, Expr, ExprKind, FunctionName, Literal, UnaryOp,
};

#[test]
fn parses_precedence_and_slot_access() -> Result<(), CompilerError> {
    let parsed = parse_expression(r#"$input.total >= 10 and not false or $backup == "yes""#)?;

    assert_eq!(parsed.span.start, 0);
    assert_eq!(parsed.span.end, 52);
    assert_binary_op(&parsed, BinaryOp::Or)?;

    let ExprKind::Binary { left, right, .. } = &parsed.kind else {
        return Err(CompilerError::parse(1, 1, "expected or expression"));
    };

    assert_binary_op(left, BinaryOp::And)?;
    assert_binary_op(right, BinaryOp::Eq)?;

    Ok(())
}

#[test]
fn parses_function_calls_with_arguments() -> Result<(), CompilerError> {
    let parsed = parse_expression(r#"contains($items, "book")"#)?;

    let ExprKind::FunctionCall(call) = &parsed.kind else {
        return Err(CompilerError::parse(1, 1, "expected function call"));
    };

    assert_eq!(call.name, FunctionName::Contains);
    assert_eq!(call.args.len(), 2);
    assert_slot_path(&call.args[0], &["items"])?;
    assert_literal(&call.args[1], Literal::String("book".into()))?;
    assert_eq!(parsed.span.start, 0);
    assert_eq!(parsed.span.end, 24);

    Ok(())
}

#[test]
fn parses_custom_function_and_parenthesized_math() -> Result<(), CompilerError> {
    let parsed = parse_expression("score(($left + 3) - 1)")?;

    let ExprKind::FunctionCall(call) = &parsed.kind else {
        return Err(CompilerError::parse(1, 1, "expected function call"));
    };

    assert_eq!(call.name, FunctionName::Ident("score".into()));
    assert_eq!(call.args.len(), 1);
    assert_binary_op(&call.args[0], BinaryOp::Minus)?;

    Ok(())
}

#[test]
fn reports_parse_error_location() -> Result<(), CompilerError> {
    let error = match parse_expression("$input.total ==") {
        Ok(_expr) => return Err(CompilerError::parse(1, 1, "expected parse error")),
        Err(error) => error,
    };

    assert_eq!(
        error,
        CompilerError::Parse {
            line: 1,
            column: 14,
            message: "unexpected token after expression".into(),
        }
    );

    Ok(())
}

#[test]
fn parses_unary_not() -> Result<(), CompilerError> {
    let parsed = parse_expression("not exists($thing)")?;

    let ExprKind::Unary { op, expr } = &parsed.kind else {
        return Err(CompilerError::parse(1, 1, "expected unary expression"));
    };

    assert_eq!(*op, UnaryOp::Not);

    let ExprKind::FunctionCall(call) = &expr.kind else {
        return Err(CompilerError::parse(1, 1, "expected function call"));
    };

    assert_eq!(call.name, FunctionName::Exists);

    Ok(())
}

fn assert_binary_op(expr: &Expr, expected: BinaryOp) -> Result<(), CompilerError> {
    match &expr.kind {
        ExprKind::Binary { op, .. } => {
            assert_eq!(*op, expected);
            Ok(())
        }
        _other => Err(CompilerError::parse(1, 1, "expected binary expression")),
    }
}

fn assert_slot_path(expr: &Expr, expected: &[&str]) -> Result<(), CompilerError> {
    let ExprKind::SlotAccess(slot) = &expr.kind else {
        return Err(CompilerError::parse(1, 1, "expected slot access"));
    };

    assert_eq!(
        slot.path.iter().map(Box::as_ref).collect::<Vec<_>>(),
        expected
    );
    Ok(())
}

fn assert_literal(expr: &Expr, expected: Literal) -> Result<(), CompilerError> {
    let ExprKind::Literal(literal) = &expr.kind else {
        return Err(CompilerError::parse(1, 1, "expected literal"));
    };

    assert_eq!(*literal, expected);
    Ok(())
}
