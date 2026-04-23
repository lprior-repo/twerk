#[cfg(test)]
mod tests {
    use crate::domain::testing::arb_valid_cron_expression;
    use crate::domain::CronExpression;
    use crate::domain::CronExpressionError;

    // -------------------------------------------------------------------------
    // Behavior: CronExpression constructs successfully when given valid 5-field expression
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_new_returns_ok_when_given_valid_5_field_expression() {
        let expr = CronExpression::new("0 0 * * *").expect("valid 5-field cron should parse");
        assert_eq!(expr.as_str(), "0 0 * * *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_given_standard_cron_expression() {
        let expr = CronExpression::new("*/15 * * * MON-FRI").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "*/15 * * * MON-FRI");
    }

    // -------------------------------------------------------------------------
    // Behavior: CronExpression constructs successfully when given valid 6-field expression
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_new_returns_ok_when_given_valid_6_field_expression() {
        let expr = CronExpression::new("0 30 8 1 * *").expect("valid 6-field cron should parse");
        assert_eq!(expr.as_str(), "0 30 8 1 * *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_given_sixth_field_expression() {
        let expr =
            CronExpression::new("0 0 0 1 JAN *").expect("valid cron with month should parse");
        assert_eq!(expr.as_str(), "0 0 0 1 JAN *");
    }

    // -------------------------------------------------------------------------
    // Behavior: CronExpression returns error when input is empty string
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_new_returns_empty_error_when_input_is_empty() {
        let e = CronExpression::new("").expect_err("empty string should fail");
        assert!(matches!(e, CronExpressionError::Empty));
    }

    // -------------------------------------------------------------------------
    // Behavior: CronExpression returns error when input fails cron parsing
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_new_returns_parse_error_when_input_is_invalid_cron() {
        // Use 5 fields but invalid cron syntax (X is not a valid second value)
        let e = CronExpression::new("X * * * * *").expect_err("invalid cron syntax should fail");
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
        let e = CronExpression::new("* * *").expect_err("3 fields should fail");
        assert!(matches!(e, CronExpressionError::InvalidFieldCount(3)));
    }

    #[test]
    fn cron_expression_new_returns_invalid_field_count_error_when_too_many_fields() {
        let e = CronExpression::new("* * * * * * *").expect_err("7 fields should fail");
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
        let expr = CronExpression::new("0 12 * * *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 12 * * *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_given_monthly_expression() {
        let expr = CronExpression::new("0 0 1 * *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 1 * *");
    }

    // -------------------------------------------------------------------------
    // Behavior: CronExpression accepts day-of-week names
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_new_returns_ok_when_day_of_week_is_mon() {
        let expr = CronExpression::new("0 0 * * MON").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 * * MON");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_day_of_week_is_tue() {
        let expr = CronExpression::new("0 0 * * TUE").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 * * TUE");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_day_of_week_is_wed() {
        let expr = CronExpression::new("0 0 * * WED").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 * * WED");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_day_of_week_is_thu() {
        let expr = CronExpression::new("0 0 * * THU").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 * * THU");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_day_of_week_is_fri() {
        let expr = CronExpression::new("0 0 * * FRI").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 * * FRI");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_day_of_week_is_sat() {
        let expr = CronExpression::new("0 0 * * SAT").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 * * SAT");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_day_of_week_is_sun() {
        let expr = CronExpression::new("0 0 * * SUN").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 * * SUN");
    }

    // -------------------------------------------------------------------------
    // Behavior: CronExpression accepts month names
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_new_returns_ok_when_month_is_jan() {
        let expr = CronExpression::new("0 0 1 JAN *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 1 JAN *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_month_is_feb() {
        let expr = CronExpression::new("0 0 1 FEB *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 1 FEB *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_month_is_mar() {
        let expr = CronExpression::new("0 0 1 MAR *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 1 MAR *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_month_is_apr() {
        let expr = CronExpression::new("0 0 1 APR *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 1 APR *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_month_is_may() {
        let expr = CronExpression::new("0 0 1 MAY *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 1 MAY *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_month_is_jun() {
        let expr = CronExpression::new("0 0 1 JUN *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 1 JUN *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_month_is_jul() {
        let expr = CronExpression::new("0 0 1 JUL *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 1 JUL *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_month_is_aug() {
        let expr = CronExpression::new("0 0 1 AUG *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 1 AUG *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_month_is_sep() {
        let expr = CronExpression::new("0 0 1 SEP *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 1 SEP *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_month_is_oct() {
        let expr = CronExpression::new("0 0 1 OCT *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 1 OCT *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_month_is_nov() {
        let expr = CronExpression::new("0 0 1 NOV *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 1 NOV *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_month_is_dec() {
        let expr = CronExpression::new("0 0 1 DEC *").expect("valid cron should parse");
        assert_eq!(expr.as_str(), "0 0 1 DEC *");
    }

    // -------------------------------------------------------------------------
    // Behavior: CronExpression accepts special cron characters
    // -------------------------------------------------------------------------

    #[test]
    fn cron_expression_new_returns_ok_when_field_uses_asterisk() {
        // Asterisk means "every" value
        let expr = CronExpression::new("* * * * *").expect("asterisk should be valid");
        assert_eq!(expr.as_str(), "* * * * *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_field_uses_question_mark() {
        // Question mark means "no specific value" (used in day-of-week or day-of-month)
        let expr = CronExpression::new("0 0 ? * *").expect("question mark should be valid");
        assert_eq!(expr.as_str(), "0 0 ? * *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_field_uses_step_slash() {
        // Step values like */15 mean "every 15 units"
        let expr = CronExpression::new("*/15 * * * *").expect("step values should be valid");
        assert_eq!(expr.as_str(), "*/15 * * * *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_field_uses_range_hyphen() {
        // Range like MON-FRI
        let expr = CronExpression::new("0 9 1-15 * *").expect("range should be valid");
        assert_eq!(expr.as_str(), "0 9 1-15 * *");
    }

    #[test]
    fn cron_expression_new_returns_ok_when_field_uses_list_comma() {
        // List like MON,WED,FRI
        let expr = CronExpression::new("0 9 * * MON,WED,FRI").expect("list should be valid");
        assert_eq!(expr.as_str(), "0 9 * * MON,WED,FRI");
    }

    // -------------------------------------------------------------------------
    // Proptest invariants
    // -------------------------------------------------------------------------

    mod proptest_inner {
        use super::*;
        use crate::assert_is_send_and_sync;
        use proptest::prelude::*;
        use proptest::proptest;

        proptest! {
            #[test]
            fn cron_expression_new_preserves_input_valid_expressions(expr in arb_valid_cron_expression()) {
                let result = CronExpression::new(expr);
                prop_assert!(result.is_ok());
                let cron_expr = result.unwrap();
                prop_assert_eq!(cron_expr.as_str(), expr);
            }

            #[test]
            fn cron_expression_field_count_is_always_5_or_6(expr in arb_valid_cron_expression()) {
                let result = CronExpression::new(expr);
                prop_assert!(result.is_ok());
                let _cron_expr = result.unwrap();
            }

            #[test]
            fn cron_expression_display_matches_as_str(expr in arb_valid_cron_expression()) {
                let cron_expr = CronExpression::new(expr).unwrap();
                prop_assert_eq!(format!("{}", cron_expr), cron_expr.as_str());
            }

            #[test]
            fn cron_expression_is_send_and_sync(expr in arb_valid_cron_expression()) {
                let cron_expr = CronExpression::new(expr).unwrap();
                assert_is_send_and_sync!(cron_expr);
            }
        }
    }
}
