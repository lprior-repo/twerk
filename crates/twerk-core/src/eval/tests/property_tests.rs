#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use crate::eval::evaluate_expr;
    use crate::eval::evaluate_template;
    use crate::eval::transform::{sanitize_expr, transform_operators};
    use crate::eval::valid_expr;
    use crate::eval::EvalError;
    use proptest::prelude::*;
    use proptest::proptest;
    use std::collections::HashMap;

    proptest! {
        #[test]
        fn test_evaluate_template_preserves_text(template in "[a-zA-Z0-9 ]*") {
            let context = HashMap::new();
            let result = evaluate_template(&template, &context).unwrap();
            prop_assert_eq!(result, template);
        }

        #[test]
        fn test_evaluate_template_multiple_variables(
            template in "[a-z ]*",
            val1 in "[a-z]*",
            val2 in "[a-z]*"
        ) {
            let mut context = HashMap::new();
            context.insert("var1".to_string(), serde_json::json!(val1));
            context.insert("var2".to_string(), serde_json::json!(val2));
            let result = evaluate_template(&template, &context).unwrap();
            prop_assert_eq!(result, template);
        }

        #[test]
        fn test_template_roundtrip_through_sanitize(template in "[a-zA-Z0-9{} ]*") {
            let sanitized = sanitize_expr(&template);
            let transformed = transform_operators(&sanitized);
            let re_sanitized = sanitize_expr(&transformed);
            prop_assert_eq!(sanitized, re_sanitized);
        }

        #[test]
        fn test_transform_operators_idempotent(expr in "[a-zA-Z ]*") {
            let first = transform_operators(&expr);
            let second = transform_operators(&first);
            prop_assert_eq!(first, second);
        }

        #[test]
        fn test_valid_expr_consistency(expr in "[a-zA-Z0-9+\\-*/<>=! ]*") {
            let result1 = valid_expr(&expr);
            let result2 = valid_expr(&expr);
            prop_assert_eq!(result1, result2);
        }
    }

    #[test]
    fn test_evaluate_template_handles_empty_string() {
        let context = HashMap::new();
        let result = evaluate_template("", &context).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_evaluate_template_with_whitespace_only() {
        let context = HashMap::new();
        let result = evaluate_template("   ", &context).unwrap();
        assert_eq!(result, "   ");
    }

    #[test]
    fn test_valid_expr_returns_false_for_empty_string() {
        assert!(!valid_expr(""));
    }

    #[test]
    fn test_valid_expr_returns_false_for_whitespace() {
        assert!(!valid_expr("   "));
        assert!(!valid_expr("\t"));
        assert!(!valid_expr("\n"));
    }

    #[test]
    fn test_valid_expr_returns_false_for_template_braces() {
        assert!(!valid_expr("{{"));
        assert!(!valid_expr("}}"));
        assert!(!valid_expr("{{}}"));
    }

    #[test]
    fn test_sanitize_expr_idempotent_simple() {
        let expr = "hello";
        let first = sanitize_expr(expr);
        let second = sanitize_expr(&first);
        assert_eq!(first, second);
    }

    #[test]
    fn test_transform_operators_preserves_non_operator_text() {
        let expr = "hello";
        let without_ops = expr.replace(" and ", " && ").replace(" or ", " || ");
        let transformed = transform_operators(expr);
        assert_eq!(transformed, without_ops);
    }

    #[test]
    fn test_eval_error_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<EvalError>();
    }

    #[test]
    fn test_eval_error_clone() {
        let error = EvalError::CompileError("test".to_string(), "message".to_string());
        let error2 = EvalError::CompileError("test".to_string(), "message".to_string());
        assert_eq!(error, error2);
    }

    #[test]
    fn test_eval_error_debug_format() {
        let error = EvalError::InvalidExpression("test error".to_string());
        let debug = format!("{:?}", error);
        let display = format!("{}", error);
        assert!(!debug.is_empty());
        assert!(!display.is_empty());
        assert!(display.contains("test error"));
    }

    #[test]
    fn test_roundtrip_sanitize_transform_simple() {
        let expr = "hello";
        let sanitized = sanitize_expr(expr);
        let transformed = transform_operators(&sanitized);
        let re_sanitized = sanitize_expr(&transformed);
        assert_eq!(sanitized, re_sanitized);
    }

    #[test]
    fn test_eval_error_variant_display() {
        let compile_err = EvalError::CompileError("expr".to_string(), "msg".to_string());
        let expr_err = EvalError::ExpressionError("expr".to_string(), "msg".to_string());
        let invalid_err = EvalError::InvalidExpression("msg".to_string());
        let unsupported_err = EvalError::UnsupportedFunction("fn".to_string());

        assert!(format!("{}", compile_err).contains("expr"));
        assert!(format!("{}", expr_err).contains("expr"));
        assert!(format!("{}", invalid_err).contains("msg"));
        assert!(format!("{}", unsupported_err).contains("fn"));
    }

    #[test]
    fn test_eval_error_variant_equality() {
        assert_eq!(
            EvalError::CompileError("a".to_string(), "b".to_string()),
            EvalError::CompileError("a".to_string(), "b".to_string())
        );
        assert_eq!(
            EvalError::ExpressionError("a".to_string(), "b".to_string()),
            EvalError::ExpressionError("a".to_string(), "b".to_string())
        );
        assert_eq!(
            EvalError::InvalidExpression("a".to_string()),
            EvalError::InvalidExpression("a".to_string())
        );
        assert_eq!(
            EvalError::UnsupportedFunction("a".to_string()),
            EvalError::UnsupportedFunction("a".to_string())
        );
    }

    #[test]
    fn test_malformed_template_braces_various_positions() {
        let context = HashMap::new();
        assert!(evaluate_template("{{", &context).is_ok());
        assert!(evaluate_template("}}", &context).is_ok());
        assert!(evaluate_template("{{}}", &context).is_ok());
        assert!(evaluate_template("}}{{", &context).is_ok());
        assert!(evaluate_template("{{ {{", &context).is_ok());
        assert!(evaluate_template("}} }}", &context).is_ok());
    }

    #[test]
    fn test_valid_expr_with_various_operators() {
        assert!(valid_expr("1 + 1"));
        assert!(valid_expr("1 - 1"));
        assert!(valid_expr("1 * 1"));
        assert!(valid_expr("1 / 1"));
        assert!(valid_expr("1 == 1"));
        assert!(valid_expr("1 != 1"));
        assert!(valid_expr("1 < 1"));
        assert!(valid_expr("1 > 1"));
        assert!(valid_expr("1 <= 1"));
        assert!(valid_expr("1 >= 1"));
        assert!(valid_expr("true and false"));
        assert!(valid_expr("true or false"));
        assert!(valid_expr("!true"));
    }

    #[test]
    fn test_template_preserves_text_outside_expressions() {
        let context = HashMap::new();
        let result = evaluate_template("hello world", &context).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_template_with_numeric_expression() {
        let context = HashMap::new();
        let result = evaluate_template("{{ 1 + 2 }}", &context).unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn test_template_with_variable_expression() {
        let mut context = HashMap::new();
        context.insert("x".to_string(), serde_json::json!(5));
        let result = evaluate_template("{{ x + 1 }}", &context).unwrap();
        assert_eq!(result, "6");
    }

    #[test]
    fn test_template_multiple_expressions() {
        let mut context = HashMap::new();
        context.insert("a".to_string(), serde_json::json!(10));
        context.insert("b".to_string(), serde_json::json!(5));
        let result = evaluate_template("{{ a }} and {{ b }}", &context).unwrap();
        assert_eq!(result, "10 and 5");
    }

    #[test]
    fn test_template_with_empty_expression() {
        let context = HashMap::new();
        let result = evaluate_template("{{ }}", &context).unwrap();
        assert_eq!(result, "null");
    }

    #[test]
    fn test_template_with_whitespace_in_expression() {
        let mut context = HashMap::new();
        context.insert("x".to_string(), serde_json::json!(42));
        let result = evaluate_template("{{  x  +  1  }}", &context).unwrap();
        assert_eq!(result, "43");
    }

    #[test]
    fn test_evaluate_expr_with_string_concatenation() {
        let mut context = HashMap::new();
        context.insert("name".to_string(), serde_json::json!("World"));
        let result = evaluate_expr("\"Hello \" + name", &context).unwrap();
        assert_eq!(result, serde_json::json!("Hello World"));
    }

    #[test]
    fn test_evaluate_expr_with_boolean_operators() {
        let context = HashMap::new();
        let result = evaluate_expr("true and false", &context).unwrap();
        assert_eq!(result, serde_json::json!(false));
    }

    #[test]
    fn test_evaluate_expr_returns_null_for_empty() {
        let context = HashMap::new();
        let result = evaluate_expr("", &context).unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }

    #[test]
    fn test_sanitize_expr_strips_template_braces() {
        assert_eq!(sanitize_expr("{{ foo }}"), "foo");
        assert_eq!(sanitize_expr("{{foo}}"), "foo");
        assert_eq!(sanitize_expr("{{  bar  }}"), "bar");
    }

    #[test]
    fn test_sanitize_expr_preserves_non_template() {
        assert_eq!(sanitize_expr("hello"), "hello");
        assert_eq!(sanitize_expr("foo and bar"), "foo && bar");
    }

    #[test]
    fn test_transform_operators_basic() {
        assert_eq!(transform_operators("a and b"), "a && b");
        assert_eq!(transform_operators("a or b"), "a || b");
        assert_eq!(transform_operators("a and b or c"), "a && b || c");
    }

    #[test]
    fn test_transform_operators_no_change_without_operators() {
        assert_eq!(transform_operators("hello"), "hello");
        assert_eq!(transform_operators("a + b"), "a + b");
        assert_eq!(transform_operators("x == y"), "x == y");
    }
}
