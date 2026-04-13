//! Exhaustive tests for ASL NewType foundations (twerk-fq8).
//!
//! TDD RED phase: tests compile but FAIL against stub implementations.

use std::collections::HashSet;

use twerk_core::asl::error_code::ErrorCode;
use twerk_core::asl::types::{
    BackoffRate, BackoffRateError, Expression, ExpressionError, ImageRef, ImageRefError, JsonPath,
    JsonPathError, ShellScript, ShellScriptError, StateName, StateNameError, VariableName,
    VariableNameError,
};

// ===========================================================================
// StateName
// ===========================================================================

mod state_name_tests {
    use super::*;

    // -- SN-1: Valid state name -------------------------------------------

    #[test]
    fn valid_construction() {
        let result = StateName::new("HelloWorld");
        assert!(result.is_ok(), "SN-1: valid 10-char name must succeed");
        let sn = result.unwrap();
        assert_eq!(
            sn.as_str(),
            "HelloWorld",
            "SN-1: as_str must return original"
        );
        assert_eq!(sn.to_string(), "HelloWorld", "SN-1: Display must match");
    }

    // -- SN-2: Upper boundary (256 chars) ---------------------------------

    #[test]
    fn boundary_256_chars_accepted() {
        let s = "a".repeat(256);
        let result = StateName::new(s.clone());
        assert!(result.is_ok(), "SN-2: exactly 256 chars must succeed");
        let sn = result.unwrap();
        assert_eq!(sn.as_str().len(), 256, "SN-2: length must be 256");
    }

    // -- SN-3: Lower boundary (1 char) ------------------------------------

    #[test]
    fn boundary_1_char_accepted() {
        let result = StateName::new("X");
        assert!(result.is_ok(), "SN-3: single char name must succeed");
        assert_eq!(result.unwrap().as_str(), "X", "SN-3: as_str must return X");
    }

    // -- SN-4: Empty string rejected --------------------------------------

    #[test]
    fn empty_rejected() {
        let result = StateName::new("");
        assert!(result.is_err(), "SN-4: empty string must fail");
        assert_eq!(
            result.unwrap_err(),
            StateNameError::Empty,
            "SN-4: must return Empty error"
        );
    }

    // -- SN-5: 257 characters rejected ------------------------------------

    #[test]
    fn too_long_rejected() {
        let s = "a".repeat(257);
        let result = StateName::new(s);
        assert!(result.is_err(), "SN-5: 257 chars must fail");
        assert_eq!(
            result.unwrap_err(),
            StateNameError::TooLong(257),
            "SN-5: must return TooLong(257)"
        );
    }

    // -- SN-6: FromStr roundtrip ------------------------------------------

    #[test]
    fn from_str_roundtrip() {
        let result = "MyState".parse::<StateName>();
        assert!(result.is_ok(), "SN-6: FromStr must succeed for valid input");
        assert_eq!(
            result.unwrap().to_string(),
            "MyState",
            "SN-6: roundtrip must preserve value"
        );
    }

    // -- SN-7: Serde roundtrip --------------------------------------------

    #[test]
    fn serde_roundtrip() {
        let sn = StateName::new("Foo");
        assert!(sn.is_ok(), "SN-7: construction must succeed");
        let sn = sn.unwrap();

        let json = serde_json::to_string(&sn);
        assert!(json.is_ok(), "SN-7: serialization must succeed");
        assert_eq!(json.unwrap(), "\"Foo\"", "SN-7: transparent serialization");

        let de: Result<StateName, _> = serde_json::from_str("\"Foo\"");
        assert!(de.is_ok(), "SN-7: deserialization must succeed");
        assert_eq!(de.unwrap().as_str(), "Foo", "SN-7: deser value must match");
    }

    // -- SN-8: Serde rejects invalid on deserialize -----------------------

    #[test]
    fn serde_rejects_empty() {
        let de: Result<StateName, _> = serde_json::from_str("\"\"");
        assert!(de.is_err(), "SN-8: deser of empty string must fail");
    }

    // -- SN-9: Deref allows str methods -----------------------------------

    #[test]
    fn deref_to_str() {
        let sn = StateName::new("Hello");
        assert!(sn.is_ok(), "SN-9: construction must succeed");
        let sn = sn.unwrap();
        assert!(sn.contains("ell"), "SN-9: Deref must allow str::contains");
    }

    // -- SN-10: Unicode characters ----------------------------------------

    #[test]
    fn unicode_allowed() {
        let result = StateName::new("状態名");
        assert!(
            result.is_ok(),
            "SN-10: Unicode state names must be accepted"
        );
    }
}

// ===========================================================================
// Expression
// ===========================================================================

mod expression_tests {
    use super::*;

    // -- EX-1: Valid template expression -----------------------------------

    #[test]
    fn valid_construction() {
        let result = Expression::new("$.input.name");
        assert!(result.is_ok(), "EX-1: valid expression must succeed");
        assert_eq!(
            result.unwrap().as_str(),
            "$.input.name",
            "EX-1: as_str must return original"
        );
    }

    // -- EX-2: Empty rejected ---------------------------------------------

    #[test]
    fn empty_rejected() {
        let result = Expression::new("");
        assert!(result.is_err(), "EX-2: empty expression must fail");
        assert_eq!(
            result.unwrap_err(),
            ExpressionError::Empty,
            "EX-2: must return Empty error"
        );
    }

    // -- EX-3: Intrinsic function expression ------------------------------

    #[test]
    fn intrinsic_function_accepted() {
        let result = Expression::new("States.Format('Hello {}', $.name)");
        assert!(
            result.is_ok(),
            "EX-3: intrinsic function expression must succeed"
        );
    }

    // -- EX-4: FromStr roundtrip ------------------------------------------

    #[test]
    fn from_str_roundtrip() {
        let result = "$.foo".parse::<Expression>();
        assert!(result.is_ok(), "EX-4: FromStr must succeed for valid input");
        assert_eq!(
            result.unwrap().to_string(),
            "$.foo",
            "EX-4: roundtrip must preserve value"
        );
    }

    // -- EX-5: Serde roundtrip --------------------------------------------

    #[test]
    fn serde_roundtrip() {
        let expr = Expression::new("$.bar");
        assert!(expr.is_ok(), "EX-5: construction must succeed");
        let expr = expr.unwrap();

        let json = serde_json::to_string(&expr);
        assert!(json.is_ok(), "EX-5: serialization must succeed");
        assert_eq!(
            json.unwrap(),
            "\"$.bar\"",
            "EX-5: transparent serialization"
        );

        let de: Result<Expression, _> = serde_json::from_str("\"$.bar\"");
        assert!(de.is_ok(), "EX-5: deserialization must succeed");
        assert_eq!(
            de.unwrap().as_str(),
            "$.bar",
            "EX-5: deser value must match"
        );
    }

    // -- EX extra: Serde rejects empty on deserialize ---------------------

    #[test]
    fn serde_rejects_empty() {
        let de: Result<Expression, _> = serde_json::from_str("\"\"");
        assert!(de.is_err(), "Expression deser of empty string must fail");
    }

    // -- EX extra: Deref to str -------------------------------------------

    #[test]
    fn deref_to_str() {
        let expr = Expression::new("$.input");
        assert!(expr.is_ok(), "Expression construction must succeed");
        let expr = expr.unwrap();
        assert!(expr.starts_with("$."), "Deref must allow str::starts_with");
    }
}

// ===========================================================================
// JsonPath
// ===========================================================================

mod json_path_tests {
    use super::*;

    // -- JP-1: Valid JSONPath ---------------------------------------------

    #[test]
    fn valid_construction() {
        let result = JsonPath::new("$.store.book[0].title");
        assert!(result.is_ok(), "JP-1: valid JSONPath must succeed");
        assert_eq!(
            result.unwrap().as_str(),
            "$.store.book[0].title",
            "JP-1: as_str must return original"
        );
    }

    // -- JP-2: Root only --------------------------------------------------

    #[test]
    fn root_only_accepted() {
        let result = JsonPath::new("$");
        assert!(result.is_ok(), "JP-2: root-only '$' must succeed");
    }

    // -- JP-3: Empty rejected ---------------------------------------------

    #[test]
    fn empty_rejected() {
        let result = JsonPath::new("");
        assert!(result.is_err(), "JP-3: empty JsonPath must fail");
        assert_eq!(
            result.unwrap_err(),
            JsonPathError::Empty,
            "JP-3: must return Empty error"
        );
    }

    // -- JP-4: Missing dollar prefix rejected -----------------------------

    #[test]
    fn missing_dollar_prefix() {
        let result = JsonPath::new("store.book");
        assert!(result.is_err(), "JP-4: missing $ prefix must fail");
        assert_eq!(
            result.unwrap_err(),
            JsonPathError::MissingDollarPrefix("store.book".to_owned()),
            "JP-4: must return MissingDollarPrefix"
        );
    }

    // -- JP-5: Serde rejects invalid on deserialize -----------------------

    #[test]
    fn serde_rejects_no_dollar() {
        let de: Result<JsonPath, _> = serde_json::from_str("\"no.dollar\"");
        assert!(de.is_err(), "JP-5: deser without $ must fail");
    }

    // -- JP-6: FromStr roundtrip ------------------------------------------

    #[test]
    fn from_str_roundtrip() {
        let result = "$.x".parse::<JsonPath>();
        assert!(result.is_ok(), "JP-6: FromStr must succeed for valid input");
        assert_eq!(
            result.unwrap().to_string(),
            "$.x",
            "JP-6: roundtrip must preserve value"
        );
    }

    // -- JP extra: Serde roundtrip ----------------------------------------

    #[test]
    fn serde_roundtrip() {
        let jp = JsonPath::new("$.a.b");
        assert!(jp.is_ok(), "JsonPath construction must succeed");
        let jp = jp.unwrap();

        let json = serde_json::to_string(&jp);
        assert!(json.is_ok(), "JsonPath serialization must succeed");
        assert_eq!(json.unwrap(), "\"$.a.b\"", "transparent serialization");

        let de: Result<JsonPath, _> = serde_json::from_str("\"$.a.b\"");
        assert!(de.is_ok(), "JsonPath deserialization must succeed");
        assert_eq!(de.unwrap().as_str(), "$.a.b", "deser value must match");
    }

    // -- JP extra: Deref --------------------------------------------------

    #[test]
    fn deref_to_str() {
        let jp = JsonPath::new("$.items");
        assert!(jp.is_ok(), "JsonPath construction must succeed");
        let jp = jp.unwrap();
        assert!(jp.starts_with('$'), "Deref must allow str::starts_with");
    }

    // -- JP extra: Serde rejects empty ------------------------------------

    #[test]
    fn serde_rejects_empty() {
        let de: Result<JsonPath, _> = serde_json::from_str("\"\"");
        assert!(de.is_err(), "JsonPath deser of empty string must fail");
    }
}

// ===========================================================================
// VariableName
// ===========================================================================

mod variable_name_tests {
    use super::*;

    // -- VN-1: Valid simple name ------------------------------------------

    #[test]
    fn valid_construction() {
        let result = VariableName::new("my_var");
        assert!(result.is_ok(), "VN-1: valid variable name must succeed");
        assert_eq!(
            result.unwrap().as_str(),
            "my_var",
            "VN-1: as_str must return original"
        );
    }

    // -- VN-2: Starts with underscore -------------------------------------

    #[test]
    fn starts_with_underscore() {
        let result = VariableName::new("_private");
        assert!(result.is_ok(), "VN-2: underscore start must succeed");
    }

    // -- VN-3: Single character -------------------------------------------

    #[test]
    fn single_char() {
        let result = VariableName::new("x");
        assert!(
            result.is_ok(),
            "VN-3: single char variable name must succeed"
        );
    }

    // -- VN-4: Empty rejected ---------------------------------------------

    #[test]
    fn empty_rejected() {
        let result = VariableName::new("");
        assert!(result.is_err(), "VN-4: empty variable name must fail");
        assert_eq!(
            result.unwrap_err(),
            VariableNameError::Empty,
            "VN-4: must return Empty error"
        );
    }

    // -- VN-5: Starts with digit rejected ---------------------------------

    #[test]
    fn starts_with_digit_rejected() {
        let result = VariableName::new("1abc");
        assert!(result.is_err(), "VN-5: digit start must fail");
        assert_eq!(
            result.unwrap_err(),
            VariableNameError::InvalidStart('1'),
            "VN-5: must return InvalidStart('1')"
        );
    }

    // -- VN-6: Contains hyphen rejected -----------------------------------

    #[test]
    fn contains_hyphen_rejected() {
        let result = VariableName::new("my-var");
        assert!(result.is_err(), "VN-6: hyphen must be rejected");
        assert_eq!(
            result.unwrap_err(),
            VariableNameError::InvalidCharacter('-'),
            "VN-6: must return InvalidCharacter('-')"
        );
    }

    // -- VN-7: 129 characters rejected ------------------------------------

    #[test]
    fn too_long_rejected() {
        let s = "a".repeat(129);
        let result = VariableName::new(s);
        assert!(result.is_err(), "VN-7: 129 chars must fail");
        assert_eq!(
            result.unwrap_err(),
            VariableNameError::TooLong(129),
            "VN-7: must return TooLong(129)"
        );
    }

    // -- VN-8: Exactly 128 characters accepted ----------------------------

    #[test]
    fn boundary_128_chars_accepted() {
        let s = "a".repeat(128);
        let result = VariableName::new(s);
        assert!(result.is_ok(), "VN-8: exactly 128 chars must succeed");
    }

    // -- VN-9: Contains space rejected ------------------------------------

    #[test]
    fn contains_space_rejected() {
        let result = VariableName::new("my var");
        assert!(result.is_err(), "VN-9: space in name must fail");
        assert_eq!(
            result.unwrap_err(),
            VariableNameError::InvalidCharacter(' '),
            "VN-9: must return InvalidCharacter(' ')"
        );
    }

    // -- VN-10: Serde roundtrip -------------------------------------------

    #[test]
    fn serde_roundtrip() {
        let vn = VariableName::new("count");
        assert!(vn.is_ok(), "VN-10: construction must succeed");
        let vn = vn.unwrap();

        let json = serde_json::to_string(&vn);
        assert!(json.is_ok(), "VN-10: serialization must succeed");
        assert_eq!(
            json.unwrap(),
            "\"count\"",
            "VN-10: transparent serialization"
        );

        let de: Result<VariableName, _> = serde_json::from_str("\"count\"");
        assert!(de.is_ok(), "VN-10: deserialization must succeed");
        assert_eq!(
            de.unwrap().as_str(),
            "count",
            "VN-10: deser value must match"
        );
    }

    // -- VN extra: FromStr roundtrip --------------------------------------

    #[test]
    fn from_str_roundtrip() {
        let result = "foo_bar".parse::<VariableName>();
        assert!(result.is_ok(), "VariableName FromStr must succeed");
        assert_eq!(
            result.unwrap().to_string(),
            "foo_bar",
            "roundtrip must match"
        );
    }

    // -- VN extra: Serde rejects invalid ----------------------------------

    #[test]
    fn serde_rejects_invalid_start() {
        let de: Result<VariableName, _> = serde_json::from_str("\"1bad\"");
        assert!(de.is_err(), "VariableName deser with digit start must fail");
    }

    // -- VN extra: Deref --------------------------------------------------

    #[test]
    fn deref_to_str() {
        let vn = VariableName::new("hello");
        assert!(vn.is_ok(), "VariableName construction must succeed");
        let vn = vn.unwrap();
        assert!(vn.contains("ell"), "Deref must allow str::contains");
    }
}

// ===========================================================================
// ImageRef
// ===========================================================================

mod image_ref_tests {
    use super::*;

    // -- IR-1: Valid Docker image -----------------------------------------

    #[test]
    fn valid_docker_image() {
        let result = ImageRef::new("docker.io/library/alpine:latest");
        assert!(result.is_ok(), "IR-1: valid Docker image must succeed");
        assert_eq!(
            result.unwrap().as_str(),
            "docker.io/library/alpine:latest",
            "IR-1: as_str must return original"
        );
    }

    // -- IR-2: Simple image name ------------------------------------------

    #[test]
    fn simple_image_name() {
        let result = ImageRef::new("ubuntu");
        assert!(result.is_ok(), "IR-2: simple image name must succeed");
    }

    // -- IR-3: Image with digest ------------------------------------------

    #[test]
    fn image_with_digest() {
        let result = ImageRef::new("alpine@sha256:abcdef1234567890");
        assert!(result.is_ok(), "IR-3: image with digest must succeed");
    }

    // -- IR-4: Empty rejected ---------------------------------------------

    #[test]
    fn empty_rejected() {
        let result = ImageRef::new("");
        assert!(result.is_err(), "IR-4: empty ImageRef must fail");
        assert_eq!(
            result.unwrap_err(),
            ImageRefError::Empty,
            "IR-4: must return Empty error"
        );
    }

    // -- IR-5: Contains space rejected ------------------------------------

    #[test]
    fn contains_space_rejected() {
        let result = ImageRef::new("my image");
        assert!(result.is_err(), "IR-5: space in image ref must fail");
        assert_eq!(
            result.unwrap_err(),
            ImageRefError::ContainsWhitespace,
            "IR-5: must return ContainsWhitespace"
        );
    }

    // -- IR-6: Contains tab rejected --------------------------------------

    #[test]
    fn contains_tab_rejected() {
        let result = ImageRef::new("my\timage");
        assert!(result.is_err(), "IR-6: tab in image ref must fail");
        assert_eq!(
            result.unwrap_err(),
            ImageRefError::ContainsWhitespace,
            "IR-6: must return ContainsWhitespace"
        );
    }

    // -- IR-7: Serde roundtrip --------------------------------------------

    #[test]
    fn serde_roundtrip() {
        let ir = ImageRef::new("nginx:1.25");
        assert!(ir.is_ok(), "IR-7: construction must succeed");
        let ir = ir.unwrap();

        let json = serde_json::to_string(&ir);
        assert!(json.is_ok(), "IR-7: serialization must succeed");
        assert_eq!(
            json.unwrap(),
            "\"nginx:1.25\"",
            "IR-7: transparent serialization"
        );

        let de: Result<ImageRef, _> = serde_json::from_str("\"nginx:1.25\"");
        assert!(de.is_ok(), "IR-7: deserialization must succeed");
        assert_eq!(
            de.unwrap().as_str(),
            "nginx:1.25",
            "IR-7: deser value must match"
        );
    }

    // -- IR extra: Serde rejects whitespace on deserialize ----------------

    #[test]
    fn serde_rejects_whitespace() {
        let de: Result<ImageRef, _> = serde_json::from_str("\"has space\"");
        assert!(de.is_err(), "ImageRef deser with whitespace must fail");
    }

    // -- IR extra: FromStr roundtrip --------------------------------------

    #[test]
    fn from_str_roundtrip() {
        let result = "alpine:3.18".parse::<ImageRef>();
        assert!(result.is_ok(), "ImageRef FromStr must succeed");
        assert_eq!(
            result.unwrap().to_string(),
            "alpine:3.18",
            "roundtrip must match"
        );
    }

    // -- IR extra: Deref --------------------------------------------------

    #[test]
    fn deref_to_str() {
        let ir = ImageRef::new("nginx");
        assert!(ir.is_ok(), "ImageRef construction must succeed");
        let ir = ir.unwrap();
        assert!(ir.contains("gin"), "Deref must allow str::contains");
    }
}

// ===========================================================================
// ShellScript
// ===========================================================================

mod shell_script_tests {
    use super::*;

    // -- SS-1: Valid script -----------------------------------------------

    #[test]
    fn valid_construction() {
        let result = ShellScript::new("echo hello");
        assert!(result.is_ok(), "SS-1: valid script must succeed");
        assert_eq!(
            result.unwrap().as_str(),
            "echo hello",
            "SS-1: as_str must return original"
        );
    }

    // -- SS-2: Multi-line script ------------------------------------------

    #[test]
    fn multi_line_script() {
        let script = "#!/bin/bash\necho hello\nexit 0";
        let result = ShellScript::new(script);
        assert!(result.is_ok(), "SS-2: multi-line script must succeed");
    }

    // -- SS-3: Empty rejected ---------------------------------------------

    #[test]
    fn empty_rejected() {
        let result = ShellScript::new("");
        assert!(result.is_err(), "SS-3: empty ShellScript must fail");
        assert_eq!(
            result.unwrap_err(),
            ShellScriptError::Empty,
            "SS-3: must return Empty error"
        );
    }

    // -- SS-4: Serde roundtrip --------------------------------------------

    #[test]
    fn serde_roundtrip() {
        let ss = ShellScript::new("ls -la");
        assert!(ss.is_ok(), "SS-4: construction must succeed");
        let ss = ss.unwrap();

        let json = serde_json::to_string(&ss);
        assert!(json.is_ok(), "SS-4: serialization must succeed");
        assert_eq!(
            json.unwrap(),
            "\"ls -la\"",
            "SS-4: transparent serialization"
        );

        let de: Result<ShellScript, _> = serde_json::from_str("\"ls -la\"");
        assert!(de.is_ok(), "SS-4: deserialization must succeed");
        assert_eq!(
            de.unwrap().as_str(),
            "ls -la",
            "SS-4: deser value must match"
        );
    }

    // -- SS extra: Serde rejects empty on deserialize ---------------------

    #[test]
    fn serde_rejects_empty() {
        let de: Result<ShellScript, _> = serde_json::from_str("\"\"");
        assert!(de.is_err(), "ShellScript deser of empty string must fail");
    }

    // -- SS extra: FromStr roundtrip --------------------------------------

    #[test]
    fn from_str_roundtrip() {
        let result = "pwd".parse::<ShellScript>();
        assert!(result.is_ok(), "ShellScript FromStr must succeed");
        assert_eq!(result.unwrap().to_string(), "pwd", "roundtrip must match");
    }

    // -- SS extra: Deref --------------------------------------------------

    #[test]
    fn deref_to_str() {
        let ss = ShellScript::new("echo ok");
        assert!(ss.is_ok(), "ShellScript construction must succeed");
        let ss = ss.unwrap();
        assert!(ss.contains("echo"), "Deref must allow str::contains");
    }
}

// ===========================================================================
// BackoffRate
// ===========================================================================

mod backoff_rate_tests {
    use super::*;

    // -- BR-1: Valid rate -------------------------------------------------

    #[test]
    fn valid_construction() {
        let result = BackoffRate::new(2.0);
        assert!(result.is_ok(), "BR-1: valid rate must succeed");
        let br = result.unwrap();
        assert!(
            (br.value() - 2.0).abs() < f64::EPSILON,
            "BR-1: value must be 2.0"
        );
    }

    // -- BR-2: Small positive (lower boundary) ----------------------------

    #[test]
    fn min_positive_accepted() {
        let result = BackoffRate::new(f64::MIN_POSITIVE);
        assert!(result.is_ok(), "BR-2: f64::MIN_POSITIVE must succeed");
    }

    // -- BR-3: Zero rejected ----------------------------------------------

    #[test]
    fn zero_rejected() {
        let result = BackoffRate::new(0.0);
        assert!(result.is_err(), "BR-3: zero must fail");
        match result.unwrap_err() {
            BackoffRateError::NotPositive(v) => {
                assert!((v - 0.0).abs() < f64::EPSILON, "BR-3: error must carry 0.0");
            }
            other => panic!("BR-3: expected NotPositive, got {other:?}"),
        }
    }

    // -- BR-4: Negative rejected ------------------------------------------

    #[test]
    fn negative_rejected() {
        let result = BackoffRate::new(-1.5);
        assert!(result.is_err(), "BR-4: negative must fail");
        match result.unwrap_err() {
            BackoffRateError::NotPositive(v) => {
                assert!(
                    (v - (-1.5)).abs() < f64::EPSILON,
                    "BR-4: error must carry -1.5"
                );
            }
            other => panic!("BR-4: expected NotPositive, got {other:?}"),
        }
    }

    // -- BR-5: NaN rejected -----------------------------------------------

    #[test]
    fn nan_rejected() {
        let result = BackoffRate::new(f64::NAN);
        assert!(result.is_err(), "BR-5: NaN must fail");
        match result.unwrap_err() {
            BackoffRateError::NotFinite(v) => {
                assert!(v.is_nan(), "BR-5: error must carry NaN");
            }
            other => panic!("BR-5: expected NotFinite, got {other:?}"),
        }
    }

    // -- BR-6: Positive infinity rejected ---------------------------------

    #[test]
    fn pos_infinity_rejected() {
        let result = BackoffRate::new(f64::INFINITY);
        assert!(result.is_err(), "BR-6: +infinity must fail");
        match result.unwrap_err() {
            BackoffRateError::NotFinite(v) => {
                assert!(
                    v.is_infinite() && v.is_sign_positive(),
                    "BR-6: must be +inf"
                );
            }
            other => panic!("BR-6: expected NotFinite, got {other:?}"),
        }
    }

    // -- BR-7: Negative infinity rejected ---------------------------------

    #[test]
    fn neg_infinity_rejected() {
        let result = BackoffRate::new(f64::NEG_INFINITY);
        assert!(result.is_err(), "BR-7: -infinity must fail");
        match result.unwrap_err() {
            BackoffRateError::NotFinite(v) => {
                assert!(
                    v.is_infinite() && v.is_sign_negative(),
                    "BR-7: must be -inf"
                );
            }
            other => panic!("BR-7: expected NotFinite, got {other:?}"),
        }
    }

    // -- BR-8: FromStr valid ----------------------------------------------

    #[test]
    fn from_str_valid() {
        let result = "1.5".parse::<BackoffRate>();
        assert!(result.is_ok(), "BR-8: FromStr '1.5' must succeed");
        let br = result.unwrap();
        assert!(
            (br.value() - 1.5).abs() < f64::EPSILON,
            "BR-8: value must be 1.5"
        );
    }

    // -- BR-9: FromStr non-numeric rejected -------------------------------

    #[test]
    fn from_str_non_numeric() {
        let result = "abc".parse::<BackoffRate>();
        assert!(result.is_err(), "BR-9: non-numeric must fail");
        match result.unwrap_err() {
            BackoffRateError::ParseError(_) => {}
            other => panic!("BR-9: expected ParseError, got {other:?}"),
        }
    }

    // -- BR-10: FromStr zero rejected -------------------------------------

    #[test]
    fn from_str_zero() {
        let result = "0.0".parse::<BackoffRate>();
        assert!(result.is_err(), "BR-10: FromStr '0.0' must fail");
        match result.unwrap_err() {
            BackoffRateError::NotPositive(v) => {
                assert!(
                    (v - 0.0).abs() < f64::EPSILON,
                    "BR-10: error must carry 0.0"
                );
            }
            other => panic!("BR-10: expected NotPositive, got {other:?}"),
        }
    }

    // -- BR-11: Serde roundtrip -------------------------------------------

    #[test]
    fn serde_roundtrip() {
        let br = BackoffRate::new(2.75);
        assert!(br.is_ok(), "BR-11: construction must succeed");
        let br = br.unwrap();

        let json = serde_json::to_string(&br);
        assert!(json.is_ok(), "BR-11: serialization must succeed");
        assert_eq!(
            json.unwrap(),
            "2.75",
            "BR-11: transparent numeric serialization"
        );

        let de: Result<BackoffRate, _> = serde_json::from_str("2.75");
        assert!(de.is_ok(), "BR-11: deserialization must succeed");
        let de = de.unwrap();
        assert!(
            (de.value() - 2.75).abs() < f64::EPSILON,
            "BR-11: deser value must match"
        );
    }

    // -- BR-12: Serde rejects zero on deserialize -------------------------

    #[test]
    fn serde_rejects_zero() {
        let de: Result<BackoffRate, _> = serde_json::from_str("0.0");
        assert!(de.is_err(), "BR-12: deser of 0.0 must fail");
    }

    // -- BR-13: Display ---------------------------------------------------

    #[test]
    fn display_format() {
        let br = BackoffRate::new(2.5);
        assert!(br.is_ok(), "BR-13: construction must succeed");
        assert_eq!(
            br.unwrap().to_string(),
            "2.5",
            "BR-13: Display must format correctly"
        );
    }

    // -- BR-14: Negative zero rejected ------------------------------------

    #[test]
    fn negative_zero_rejected() {
        let result = BackoffRate::new(-0.0_f64);
        assert!(result.is_err(), "BR-14: -0.0 must fail");
        match result.unwrap_err() {
            BackoffRateError::NotPositive(_) => {}
            other => panic!("BR-14: expected NotPositive, got {other:?}"),
        }
    }
}

// ===========================================================================
// ErrorCode
// ===========================================================================

mod error_code_tests {
    use super::*;

    // -- EC-1: Serialize known variants -----------------------------------

    #[test]
    fn serialize_all() {
        let json = serde_json::to_string(&ErrorCode::All);
        assert!(json.is_ok(), "EC-1: serialize All must succeed");
        assert_eq!(json.unwrap(), "\"all\"", "EC-1: All serializes as 'all'");
    }

    #[test]
    fn serialize_task_failed() {
        let json = serde_json::to_string(&ErrorCode::TaskFailed);
        assert!(json.is_ok(), "EC-1: serialize TaskFailed must succeed");
        assert_eq!(
            json.unwrap(),
            "\"taskfailed\"",
            "EC-1: TaskFailed serializes as 'taskfailed'"
        );
    }

    #[test]
    fn serialize_heartbeat_timeout() {
        let json = serde_json::to_string(&ErrorCode::HeartbeatTimeout);
        assert!(
            json.is_ok(),
            "EC-1: serialize HeartbeatTimeout must succeed"
        );
        assert_eq!(
            json.unwrap(),
            "\"heartbeattimeout\"",
            "EC-1: HeartbeatTimeout serializes as 'heartbeattimeout'"
        );
    }

    #[test]
    fn serialize_timeout() {
        let json = serde_json::to_string(&ErrorCode::Timeout);
        assert!(json.is_ok(), "serialize Timeout must succeed");
        assert_eq!(
            json.unwrap(),
            "\"timeout\"",
            "Timeout serializes as 'timeout'"
        );
    }

    #[test]
    fn serialize_permissions() {
        let json = serde_json::to_string(&ErrorCode::Permissions);
        assert!(json.is_ok(), "serialize Permissions must succeed");
        assert_eq!(
            json.unwrap(),
            "\"permissions\"",
            "Permissions serializes as 'permissions'"
        );
    }

    #[test]
    fn serialize_result_path_match_failure() {
        let json = serde_json::to_string(&ErrorCode::ResultPathMatchFailure);
        assert!(
            json.is_ok(),
            "serialize ResultPathMatchFailure must succeed"
        );
        assert_eq!(
            json.unwrap(),
            "\"resultpathmatchfailure\"",
            "ResultPathMatchFailure serializes correctly"
        );
    }

    #[test]
    fn serialize_parameter_path_failure() {
        let json = serde_json::to_string(&ErrorCode::ParameterPathFailure);
        assert!(json.is_ok(), "serialize ParameterPathFailure must succeed");
        assert_eq!(
            json.unwrap(),
            "\"parameterpathfailure\"",
            "ParameterPathFailure serializes correctly"
        );
    }

    #[test]
    fn serialize_branch_failed() {
        let json = serde_json::to_string(&ErrorCode::BranchFailed);
        assert!(json.is_ok(), "serialize BranchFailed must succeed");
        assert_eq!(
            json.unwrap(),
            "\"branchfailed\"",
            "BranchFailed serializes correctly"
        );
    }

    #[test]
    fn serialize_no_choice_matched() {
        let json = serde_json::to_string(&ErrorCode::NoChoiceMatched);
        assert!(json.is_ok(), "serialize NoChoiceMatched must succeed");
        assert_eq!(
            json.unwrap(),
            "\"nochoicematched\"",
            "NoChoiceMatched serializes correctly"
        );
    }

    #[test]
    fn serialize_intrinsic_failure() {
        let json = serde_json::to_string(&ErrorCode::IntrinsicFailure);
        assert!(json.is_ok(), "serialize IntrinsicFailure must succeed");
        assert_eq!(
            json.unwrap(),
            "\"intrinsicfailure\"",
            "IntrinsicFailure serializes correctly"
        );
    }

    // -- EC-2: Serialize Custom -------------------------------------------

    #[test]
    fn serialize_custom() {
        let code = ErrorCode::Custom("MyApp.CustomError".to_owned());
        let json = serde_json::to_string(&code);
        assert!(json.is_ok(), "EC-2: serialize Custom must succeed");
        assert_eq!(
            json.unwrap(),
            "\"MyApp.CustomError\"",
            "EC-2: Custom serializes as raw string"
        );
    }

    // -- EC-3: Deserialize known variants (case-insensitive) --------------

    #[test]
    fn deserialize_all_lowercase() {
        let de: Result<ErrorCode, _> = serde_json::from_str("\"all\"");
        assert!(de.is_ok(), "EC-3: deser 'all' must succeed");
        assert_eq!(de.unwrap(), ErrorCode::All, "EC-3: 'all' -> All");
    }

    #[test]
    fn deserialize_all_uppercase() {
        let de: Result<ErrorCode, _> = serde_json::from_str("\"ALL\"");
        assert!(de.is_ok(), "EC-3: deser 'ALL' must succeed");
        assert_eq!(de.unwrap(), ErrorCode::All, "EC-3: 'ALL' -> All");
    }

    #[test]
    fn deserialize_task_failed_mixed_case() {
        let de: Result<ErrorCode, _> = serde_json::from_str("\"TaskFailed\"");
        assert!(de.is_ok(), "EC-3: deser 'TaskFailed' must succeed");
        assert_eq!(
            de.unwrap(),
            ErrorCode::TaskFailed,
            "EC-3: 'TaskFailed' -> TaskFailed"
        );
    }

    #[test]
    fn deserialize_timeout_case_insensitive() {
        for input in &["\"timeout\"", "\"Timeout\"", "\"TIMEOUT\""] {
            let de: Result<ErrorCode, _> = serde_json::from_str(input);
            assert!(de.is_ok(), "EC-3: deser {input} must succeed");
            assert_eq!(de.unwrap(), ErrorCode::Timeout, "EC-3: {input} -> Timeout");
        }
    }

    // -- EC-4: Deserialize unknown becomes Custom -------------------------

    #[test]
    fn deserialize_unknown_becomes_custom() {
        let de: Result<ErrorCode, _> = serde_json::from_str("\"MyCustomError\"");
        assert!(de.is_ok(), "EC-4: deser unknown must succeed");
        assert_eq!(
            de.unwrap(),
            ErrorCode::Custom("MyCustomError".to_owned()),
            "EC-4: unknown -> Custom"
        );
    }

    // -- EC-5: Display matches serialized form ----------------------------

    #[test]
    fn display_timeout() {
        assert_eq!(
            ErrorCode::Timeout.to_string(),
            "timeout",
            "EC-5: Timeout displays as 'timeout'"
        );
    }

    #[test]
    fn display_custom() {
        let code = ErrorCode::Custom("Foo".to_owned());
        assert_eq!(
            code.to_string(),
            "Foo",
            "EC-5: Custom displays as raw string"
        );
    }

    #[test]
    fn display_all() {
        assert_eq!(ErrorCode::All.to_string(), "all", "All displays as 'all'");
    }

    #[test]
    fn display_task_failed() {
        assert_eq!(
            ErrorCode::TaskFailed.to_string(),
            "taskfailed",
            "TaskFailed displays as 'taskfailed'"
        );
    }

    #[test]
    fn display_permissions() {
        assert_eq!(
            ErrorCode::Permissions.to_string(),
            "permissions",
            "Permissions displays as 'permissions'"
        );
    }

    #[test]
    fn display_result_path_match_failure() {
        assert_eq!(
            ErrorCode::ResultPathMatchFailure.to_string(),
            "resultpathmatchfailure",
            "ResultPathMatchFailure displays correctly"
        );
    }

    #[test]
    fn display_parameter_path_failure() {
        assert_eq!(
            ErrorCode::ParameterPathFailure.to_string(),
            "parameterpathfailure",
            "ParameterPathFailure displays correctly"
        );
    }

    #[test]
    fn display_branch_failed() {
        assert_eq!(
            ErrorCode::BranchFailed.to_string(),
            "branchfailed",
            "BranchFailed displays correctly"
        );
    }

    #[test]
    fn display_no_choice_matched() {
        assert_eq!(
            ErrorCode::NoChoiceMatched.to_string(),
            "nochoicematched",
            "NoChoiceMatched displays correctly"
        );
    }

    #[test]
    fn display_intrinsic_failure() {
        assert_eq!(
            ErrorCode::IntrinsicFailure.to_string(),
            "intrinsicfailure",
            "IntrinsicFailure displays correctly"
        );
    }

    #[test]
    fn display_heartbeat_timeout() {
        assert_eq!(
            ErrorCode::HeartbeatTimeout.to_string(),
            "heartbeattimeout",
            "HeartbeatTimeout displays correctly"
        );
    }

    // -- EC-6: FromStr known variant --------------------------------------

    #[test]
    fn from_str_known_variant() {
        let result = "timeout".parse::<ErrorCode>();
        assert!(result.is_ok(), "EC-6: FromStr 'timeout' must succeed");
        assert_eq!(
            result.unwrap(),
            ErrorCode::Timeout,
            "EC-6: 'timeout' -> Timeout"
        );
    }

    // -- EC-7: FromStr unknown variant ------------------------------------

    #[test]
    fn from_str_unknown_variant() {
        let result = "SomeRandomError".parse::<ErrorCode>();
        assert!(result.is_ok(), "EC-7: FromStr unknown must succeed");
        assert_eq!(
            result.unwrap(),
            ErrorCode::Custom("SomeRandomError".to_owned()),
            "EC-7: unknown -> Custom"
        );
    }

    // -- EC-8: FromStr is infallible --------------------------------------

    #[test]
    fn from_str_infallible() {
        // The type Err = Infallible guarantees this always succeeds
        let result = "anything_at_all".parse::<ErrorCode>();
        assert!(result.is_ok(), "EC-8: FromStr must always succeed");
    }

    // -- EC-9: Clone and equality -----------------------------------------

    #[test]
    fn clone_and_equality() {
        let code = ErrorCode::BranchFailed;
        let cloned = code.clone();
        assert_eq!(code, cloned, "EC-9: cloned value must equal original");
    }

    // -- EC-10: All known variants roundtrip through serde ----------------

    #[test]
    fn all_variants_serde_roundtrip() {
        let variants = vec![
            ErrorCode::All,
            ErrorCode::Timeout,
            ErrorCode::TaskFailed,
            ErrorCode::Permissions,
            ErrorCode::ResultPathMatchFailure,
            ErrorCode::ParameterPathFailure,
            ErrorCode::BranchFailed,
            ErrorCode::NoChoiceMatched,
            ErrorCode::IntrinsicFailure,
            ErrorCode::HeartbeatTimeout,
        ];

        for variant in &variants {
            let json = serde_json::to_string(variant);
            assert!(json.is_ok(), "EC-10: serialize {:?} must succeed", variant);
            let json = json.unwrap();

            let de: Result<ErrorCode, _> = serde_json::from_str(&json);
            assert!(
                de.is_ok(),
                "EC-10: deserialize {:?} from '{}' must succeed",
                variant,
                json
            );
            assert_eq!(
                &de.unwrap(),
                variant,
                "EC-10: {:?} must roundtrip through serde",
                variant
            );
        }
    }

    // -- EC-11: Hash consistency ------------------------------------------

    #[test]
    fn hash_consistency() {
        let mut set = HashSet::new();
        set.insert(ErrorCode::All);
        set.insert(ErrorCode::All);
        assert_eq!(set.len(), 1, "EC-11: duplicate All must merge in HashSet");
    }

    // -- EC-12: Custom equality -------------------------------------------

    #[test]
    fn custom_equality_different() {
        let a = ErrorCode::Custom("A".to_owned());
        let b = ErrorCode::Custom("B".to_owned());
        assert_ne!(a, b, "EC-12: Custom('A') != Custom('B')");
    }

    #[test]
    fn custom_equality_same() {
        let a = ErrorCode::Custom("A".to_owned());
        let b = ErrorCode::Custom("A".to_owned());
        assert_eq!(a, b, "EC-12: Custom('A') == Custom('A')");
    }

    // -- EC extra: FromStr case-insensitive for all known variants --------

    #[test]
    fn from_str_case_insensitive_all_variants() {
        let cases = vec![
            ("all", ErrorCode::All),
            ("ALL", ErrorCode::All),
            ("All", ErrorCode::All),
            ("timeout", ErrorCode::Timeout),
            ("TIMEOUT", ErrorCode::Timeout),
            ("taskfailed", ErrorCode::TaskFailed),
            ("TASKFAILED", ErrorCode::TaskFailed),
            ("permissions", ErrorCode::Permissions),
            ("PERMISSIONS", ErrorCode::Permissions),
            ("resultpathmatchfailure", ErrorCode::ResultPathMatchFailure),
            ("parameterpathfailure", ErrorCode::ParameterPathFailure),
            ("branchfailed", ErrorCode::BranchFailed),
            ("nochoicematched", ErrorCode::NoChoiceMatched),
            ("intrinsicfailure", ErrorCode::IntrinsicFailure),
            ("heartbeattimeout", ErrorCode::HeartbeatTimeout),
            ("HEARTBEATTIMEOUT", ErrorCode::HeartbeatTimeout),
        ];

        for (input, expected) in &cases {
            let result = input.parse::<ErrorCode>();
            assert!(result.is_ok(), "FromStr for '{input}' must succeed");
            assert_eq!(
                &result.unwrap(),
                expected,
                "FromStr '{input}' must map to {expected:?}"
            );
        }
    }

    // -- EC extra: Display roundtrip for all known variants ---------------

    #[test]
    fn display_roundtrip_all_variants() {
        let variants = vec![
            ErrorCode::All,
            ErrorCode::Timeout,
            ErrorCode::TaskFailed,
            ErrorCode::Permissions,
            ErrorCode::ResultPathMatchFailure,
            ErrorCode::ParameterPathFailure,
            ErrorCode::BranchFailed,
            ErrorCode::NoChoiceMatched,
            ErrorCode::IntrinsicFailure,
            ErrorCode::HeartbeatTimeout,
        ];

        for variant in &variants {
            let displayed = variant.to_string();
            let parsed: ErrorCode = displayed.parse().unwrap();
            assert_eq!(
                &parsed, variant,
                "Display -> FromStr roundtrip must preserve {:?}",
                variant
            );
        }
    }
}
