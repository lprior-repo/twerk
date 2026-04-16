#[cfg(test)]
mod tests {
    use crate::domain::CronExpression;
    use crate::domain::CronExpressionError;

    // -------------------------------------------------------------------------
    // Behavior: CronExpression constructs successfully when given valid 5-field expression
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_new_returns_ok_when_given_valid_5_field_expression() {
        let result = CronExpression::new("0 0 * * *");
        assert!(result.is_ok());
        let expr = result.unwrap();
        assert_eq!(expr.as_str(), "0 0 * * *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_given_standard_cron_expression() {
        let result = CronExpression::new("*/15 * * * MON-FRI");
        assert!(result.is_ok());
        let expr = result.unwrap();
        assert_eq!(expr.as_str(), "*/15 * * * MON-FRI");
    }

    // -------------------------------------------------------------------------
    // Behavior: CronExpression constructs successfully when given valid 6-field expression
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_new_returns_ok_when_given_valid_6_field_expression() {
        let result = CronExpression::new("0 30 8 1 * *");
        assert!(result.is_ok());
        let expr = result.unwrap();
        assert_eq!(expr.as_str(), "0 30 8 1 * *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_given_six_field_with_seconds() {
        let result = CronExpression::new("0 0 0 1 JAN *");
        assert!(result.is_ok());
        let expr = result.unwrap();
        assert_eq!(expr.as_str(), "0 0 0 1 JAN *");
    }

    // -------------------------------------------------------------------------
    // Behavior: CronExpression returns error when input is empty string
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_new_returns_empty_error_when_input_is_empty() {
        let result = CronExpression::new("");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, CronExpressionError::Empty));
    }

    // -------------------------------------------------------------------------
    // Behavior: CronExpression returns error when input fails cron parsing
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_new_returns_parse_error_when_input_is_invalid_cron() {
        let result = CronExpression::new("not a cron expression");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, CronExpressionError::ParseError(_)));
        if let CronExpressionError::ParseError(s) = e {
            assert!(!s.is_empty());
        }
    }

    // -------------------------------------------------------------------------
    // Behavior: CronExpression returns error when field count is not 5 or 6
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_new_returns_invalid_field_count_error_when_too_few_fields() {
        let result = CronExpression::new("* * *");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, CronExpressionError::InvalidFieldCount(3)));
    }

    #[test]
    fn cron_expression_new_returns_invalid_field_count_error_when_too_many_fields() {
        let result = CronExpression::new("* * * * * * *");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, CronExpressionError::InvalidFieldCount(7)));
    }

    // -------------------------------------------------------------------------
    // Behavior: CronExpression returns original string when as_str is called
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_as_str_returns_original_input_exactly() {
        let input = "0 0 * * MON";
        let expr = CronExpression::new(input).unwrap();
        assert_eq!(expr.as_str(), input);
    }

    // -------------------------------------------------------------------------
    // Behavior: CronExpression invariant: as_str always returns non-empty string
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_as_str_never_returns_empty_string() {
        let expr = CronExpression::new("0 * * * *").unwrap();
        assert!(!expr.as_str().is_empty());
    }

    // -------------------------------------------------------------------------
    // Behavior: CronExpression invariant: contains exactly 5 or 6 space-separated fields
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_as_str_field_count_is_always_5_or_6() {
        let expr = CronExpression::new("0 0 * * *").unwrap();
        let field_count = expr.as_str().split_whitespace().count();
        assert!(field_count == 5 || field_count == 6);
    }

    // -------------------------------------------------------------------------
    // Additional boundary tests
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_new_returns_ok_when_given_daily_noon_expression() {
        let result = CronExpression::new("0 12 * * *");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "0 12 * * *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_given_monthly_expression() {
        let result = CronExpression::new("0 0 1 * *");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "0 0 1 * *");
    }

    // -------------------------------------------------------------------------
    // Proptest invariants
    // -------------------------------------------------------------------------

    mod proptest_inner {
        use super::*;
        use proptest::prelude::*;
        use proptest::proptest;

        proptest! {
            #[test]
            fn cron_expression_new_preserves_input_valid_expressions(expr in prop_oneof![
                "0 0 * * *",
                "*/15 * * * MON-FRI",
                "0 30 8 1 * *",
                "0 0 * * MON"
            ].prop_map(|s| s.to_string())) {
                let result = CronExpression::new(&expr);
                prop_assert!(result.is_ok());
                let cron_expr = result.unwrap();
                prop_assert_eq!(cron_expr.as_str(), expr);
            }

            #[test]
            fn cron_expression_field_count_is_always_5_or_6(expr in prop_oneof![
                "0 0 * * *",
                "*/15 * * * MON-FRI",
                "0 30 8 1 * *"
            ].prop_map(|s| s.to_string())) {
                let result = CronExpression::new(&expr);
                prop_assert!(result.is_ok());
                let cron_expr = result.unwrap();
                let field_count = cron_expr.as_str().split_whitespace().count();
                prop_assert!(field_count == 5 || field_count == 6);
            }

            #[test]
            fn cron_expression_display_matches_as_str(expr in prop_oneof![
                "0 0 * * *",
                "*/15 * * * MON-FRI"
            ].prop_map(|s| s.to_string())) {
                let cron_expr = CronExpression::new(expr.clone()).unwrap();
                prop_assert_eq!(format!("{}", cron_expr), cron_expr.as_str());
            }

            #[test]
            fn cron_expression_is_send_and_sync(expr in prop_oneof![
                "0 0 * * *",
                "*/15 * * * MON-FRI"
            ].prop_map(|s| s.to_string())) {
                let cron_expr = CronExpression::new(expr).unwrap();
                fn assert_send<T: Send>(_: &T) {}
                fn assert_sync<T: Sync>(_: &T) {}
                assert_send(&cron_expr);
                assert_sync(&cron_expr);
            }
        }
    }
}
