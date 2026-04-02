#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::error::ApiError;
use serde::de::DeserializeOwned;
use std::str;

const MAX_YAML_DEPTH: usize = 64;
const MAX_YAML_BODY_SIZE: usize = 512 * 1024;

/// Parses a YAML body from a byte slice.
///
/// # Errors
///
/// Returns `ApiError::BadRequest` if:
/// - The body exceeds `MAX_YAML_BODY_SIZE`.
/// - The body is not valid UTF-8.
/// - The YAML nesting depth exceeds `MAX_YAML_DEPTH`.
/// - The YAML is malformed and cannot be parsed into `T`.
pub fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiError> {
    if bytes.len() > MAX_YAML_BODY_SIZE {
        return Err(ApiError::bad_request(format!(
            "YAML body exceeds {MAX_YAML_BODY_SIZE} byte limit"
        )));
    }
    let s =
        str::from_utf8(bytes).map_err(|e| ApiError::bad_request(format!("invalid UTF-8: {e}")))?;
    validate_yaml_depth(s)?;
    serde_yaml2::from_str(s).map_err(|e| ApiError::bad_request(format!("YAML parse error: {e}")))
}

fn validate_yaml_depth(input: &str) -> Result<(), ApiError> {
    let depth = measure_max_nesting(input);
    if depth > MAX_YAML_DEPTH {
        return Err(ApiError::bad_request(format!(
            "YAML nesting depth {depth} exceeds maximum allowed depth {MAX_YAML_DEPTH}"
        )));
    }
    Ok(())
}

fn measure_max_nesting(input: &str) -> usize {
    input
        .lines()
        .map(|line| {
            let trimmed = line.trim_start_matches(' ');
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return 0;
            }
            (line.len() - trimmed.len()) / 2
        })
        .fold(0usize, std::cmp::Ord::max)
}

#[cfg(test)]
#[allow(clippy::panic, clippy::approx_constant, dead_code)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[test]
    fn from_slice_returns_valid_job_when_yaml_is_well_formed() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Simple {
            name: String,
        }
        let yaml = b"name: test-job";
        let result: Result<Simple, ApiError> = from_slice(yaml);
        assert_eq!(result.map(|s| s.name), Ok("test-job".to_string()));
    }

    #[test]
    fn from_slice_returns_bad_request_when_bytes_are_not_utf8() {
        let bad_bytes: &[u8] = &[0xff, 0xfe, 0xfd];
        let result: Result<serde_json::Value, ApiError> = from_slice(bad_bytes);
        assert!(matches!(result, Err(ApiError::BadRequest(msg)) if msg.contains("invalid UTF-8")));
    }

    #[test]
    fn from_slice_returns_bad_request_when_yaml_is_malformed() {
        let bad_yaml = b": : : invalid";
        let result: Result<serde_json::Value, ApiError> = from_slice(bad_yaml);
        assert!(
            matches!(result, Err(ApiError::BadRequest(msg)) if msg.contains("YAML parse error"))
        );
    }

    #[test]
    fn from_slice_returns_bad_request_when_body_exceeds_size_limit() {
        let oversized = vec![b'x'; MAX_YAML_BODY_SIZE + 1];
        let result: Result<serde_json::Value, ApiError> = from_slice(&oversized);
        assert!(matches!(result, Err(ApiError::BadRequest(msg)) if msg.contains("exceeds")));
    }

    #[test]
    fn from_slice_returns_bad_request_when_nesting_exceeds_depth_limit() {
        let target = MAX_YAML_DEPTH + 1;
        let deep_yaml = (0..=target)
            .map(|i| format!("{}level{i}: {i}", " ".repeat(i * 2)))
            .collect::<Vec<String>>()
            .join("\n");
        let result: Result<serde_json::Value, ApiError> = from_slice(deep_yaml.as_bytes());
        let Err(ApiError::BadRequest(msg)) = result else {
            panic!("expected BadRequest, got {result:?}");
        };
        assert!(msg.contains("nesting depth"), "message was: {msg}");
    }

    #[test]
    fn measure_max_nesting_returns_zero_for_flat_yaml() {
        let input = "name: hello\nvalue: world\n";
        assert_eq!(measure_max_nesting(input), 0);
    }

    #[test]
    fn measure_max_nesting_returns_correct_depth_for_nested_yaml() {
        let input = "root:\n  child:\n    grandchild: value\n";
        assert_eq!(measure_max_nesting(input), 2);
    }

    #[test]
    fn measure_max_nesting_ignores_comments_and_blank_lines() {
        let input = "# comment\n\n  # indented comment\nkey: val\n";
        assert_eq!(measure_max_nesting(input), 0);
    }

    #[test]
    fn from_slice_parses_complex_nested_structure() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Task {
            name: String,
            items: Vec<String>,
        }
        let yaml = b"name: deploy\nitems:\n  - build\n  - test\n  - release\n";
        let task: Task = from_slice(yaml)?;
        assert_eq!(task.name, "deploy");
        assert_eq!(task.items, vec!["build", "test", "release"]);
        Ok(())
    }

    // ── measure_max_nesting unit tests ──────────────────────────────────

    #[test]
    fn measure_max_nesting_returns_zero_for_empty_string() {
        assert_eq!(measure_max_nesting(""), 0);
    }

    #[test]
    fn measure_max_nesting_returns_zero_for_single_unindented_key() {
        assert_eq!(measure_max_nesting("key: value"), 0);
    }

    #[test]
    fn measure_max_nesting_returns_zero_for_single_space_indent() {
        // 1 space / 2 = 0 via integer division
        assert_eq!(measure_max_nesting(" key: value"), 0);
    }

    #[test]
    fn measure_max_nesting_returns_one_for_three_space_indent() {
        // 3 spaces / 2 = 1 via integer division
        assert_eq!(measure_max_nesting("   key: value"), 1);
    }

    #[test]
    fn measure_max_nesting_returns_zero_for_tab_indented_line() {
        // trim_start_matches(' ') does not strip tabs; tab alone = 0 spaces removed
        assert_eq!(measure_max_nesting("\tkey: value"), 0);
    }

    #[test]
    fn measure_max_nesting_counts_only_leading_spaces_before_tab() {
        // "  \tkey" → trimmed = "\tkey", spaces stripped = 2, depth = 2/2 = 1
        assert_eq!(measure_max_nesting("  \tkey: value"), 1);
    }

    #[test]
    fn measure_max_nesting_returns_zero_for_multi_tab_indented_line() {
        // "\t\tkey" → original: trim_start_matches(' ') strips 0 spaces, depth = 0
        //             mutant trim_start(): strips both tabs, diff = 2, depth = 1 → CAUGHT
        assert_eq!(measure_max_nesting("\t\tkey: value"), 0);
    }

    #[test]
    fn measure_max_nesting_returns_zero_for_whitespace_only_content() {
        assert_eq!(measure_max_nesting("   \n   \n"), 0);
    }

    #[test]
    fn measure_max_nesting_treats_indented_comment_as_zero() {
        // "  # comment" → trimmed = "# comment" → starts with '#'
        assert_eq!(measure_max_nesting("  # comment"), 0);
    }

    #[test]
    fn measure_max_nesting_measures_deepest_across_mixed_lines() {
        let input = "root: value\n  child: value\n    grandchild: value\n  sibling: value\n";
        assert_eq!(measure_max_nesting(input), 2);
    }

    #[test]
    fn measure_max_nesting_returns_zero_for_newlines_only_input() {
        assert_eq!(measure_max_nesting("\n\n\n"), 0);
    }

    #[test]
    fn measure_max_nesting_handles_single_line_without_trailing_newline() {
        assert_eq!(measure_max_nesting("    deep_key: value"), 2);
    }

    #[test]
    fn measure_max_nesting_returns_five_for_ten_space_indent() {
        assert_eq!(measure_max_nesting("          key: value"), 5);
    }

    // ── measure_max_nesting rstest parametric ───────────────────────────

    #[rstest]
    #[case("key: val", 0)]
    #[case("  key: val", 1)]
    #[case("    key: val", 2)]
    #[case("      key: val", 3)]
    #[case("        key: val", 4)]
    fn measure_max_nesting_returns_expected_depth_for_space_levels(
        #[case] input: &str,
        #[case] expected: usize,
    ) {
        assert_eq!(measure_max_nesting(input), expected);
    }

    #[rstest]
    #[case("", 0)]
    #[case("# comment", 0)]
    #[case("  # comment", 0)]
    #[case("   ", 0)]
    #[case("key: val", 0)]
    fn measure_max_nesting_returns_zero_for_non_content_lines(
        #[case] input: &str,
        #[case] expected: usize,
    ) {
        assert_eq!(measure_max_nesting(input), expected);
    }

    // ── validate_yaml_depth unit tests ──────────────────────────────────

    #[test]
    fn validate_yaml_depth_returns_ok_at_exactly_max_depth() {
        let input = (0..=MAX_YAML_DEPTH)
            .map(|i| format!("{}level{i}: value", " ".repeat(i * 2)))
            .collect::<Vec<String>>()
            .join("\n");
        assert_eq!(validate_yaml_depth(&input), Ok(()));
    }

    #[test]
    fn validate_yaml_depth_returns_error_one_over_max() {
        let input = (0..=MAX_YAML_DEPTH + 1)
            .map(|i| format!("{}level{i}: value", " ".repeat(i * 2)))
            .collect::<Vec<String>>()
            .join("\n");
        let Err(ApiError::BadRequest(msg)) = validate_yaml_depth(&input) else {
            panic!("expected BadRequest for depth exceeding limit");
        };
        assert!(msg.contains("nesting depth"), "message was: {msg}");
        assert!(
            msg.contains(&format!("{MAX_YAML_DEPTH}")),
            "message should mention {MAX_YAML_DEPTH}: {msg}"
        );
    }

    // ── from_slice additional unit tests ────────────────────────────────

    #[test]
    fn from_slice_returns_ok_when_body_exactly_at_size_limit() -> Result<(), ApiError> {
        let prefix = "k: ";
        let value_len = MAX_YAML_BODY_SIZE - prefix.len();
        let yaml = format!("{prefix}{}", "a".repeat(value_len));
        assert_eq!(yaml.len(), MAX_YAML_BODY_SIZE);

        #[derive(serde::Deserialize)]
        struct KV {
            k: String,
        }
        let kv: KV = from_slice(yaml.as_bytes())?;
        assert_eq!(kv.k.len(), value_len);
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_yaml_null_as_none() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Optional {
            value: Option<String>,
        }
        let yaml = b"value: null";
        let result: Result<Optional, ApiError> = from_slice(yaml);
        assert_eq!(result.map(|o| o.value), Ok(None));
    }

    #[test]
    fn from_slice_deserializes_yaml_boolean_true() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct BoolWrap {
            flag: bool,
        }
        let result: Result<BoolWrap, ApiError> = from_slice(b"flag: true");
        assert_eq!(result.map(|b| b.flag), Ok(true));
    }

    #[test]
    fn from_slice_deserializes_yaml_boolean_false() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct BoolWrap {
            flag: bool,
        }
        let result: Result<BoolWrap, ApiError> = from_slice(b"flag: false");
        assert_eq!(result.map(|b| b.flag), Ok(false));
    }

    #[test]
    fn from_slice_deserializes_yaml_integer() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct IntWrap {
            count: i64,
        }
        let result: Result<IntWrap, ApiError> = from_slice(b"count: 42");
        assert_eq!(result.map(|i| i.count), Ok(42));
    }

    #[test]
    fn from_slice_deserializes_yaml_negative_integer() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct IntWrap {
            count: i64,
        }
        let result: Result<IntWrap, ApiError> = from_slice(b"count: -7");
        assert_eq!(result.map(|i| i.count), Ok(-7));
    }

    #[test]
    fn from_slice_deserializes_yaml_float() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize)]
        struct FloatWrap {
            ratio: f64,
        }
        let result: FloatWrap = from_slice(b"ratio: 3.14")?;
        assert!(
            (result.ratio - 3.14).abs() < 1e-10,
            "expected ~3.14, got {}",
            result.ratio
        );
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_yaml_list_of_strings() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct ListWrap {
            items: Vec<String>,
        }
        let yaml = b"items:\n  - alpha\n  - beta\n  - gamma\n";
        let result: Result<ListWrap, ApiError> = from_slice(yaml);
        assert_eq!(
            result.map(|l| l.items),
            Ok(vec![
                "alpha".to_string(),
                "beta".to_string(),
                "gamma".to_string()
            ])
        );
    }

    #[test]
    fn from_slice_deserializes_nested_map() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Inner {
            x: i32,
            y: i32,
        }
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Outer {
            inner: Inner,
        }
        let yaml = b"inner:\n  x: 1\n  y: 2\n";
        let result: Result<Outer, ApiError> = from_slice(yaml);
        assert_eq!(result.map(|o| o.inner), Ok(Inner { x: 1, y: 2 }));
    }

    #[test]
    fn from_slice_returns_bad_request_for_type_mismatch() {
        #[derive(Debug, serde::Deserialize)]
        struct Strict {
            count: i64,
        }
        let result: Result<Strict, ApiError> = from_slice(b"count: not_a_number");
        let Err(ApiError::BadRequest(msg)) = result else {
            panic!("expected BadRequest for type mismatch");
        };
        assert!(
            msg.contains("YAML parse error"),
            "expected parse error message, got: {msg}"
        );
    }

    #[test]
    fn from_slice_handles_unicode_keys_and_values() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Unicode {
            #[serde(rename = "名前")]
            name: String,
        }
        let yaml = "名前: こんにちは".as_bytes();
        let result: Result<Unicode, ApiError> = from_slice(yaml);
        assert_eq!(result.map(|u| u.name), Ok("こんにちは".to_string()));
    }

    #[test]
    fn from_slice_deserializes_into_hashmap() -> Result<(), ApiError> {
        use std::collections::HashMap;
        let yaml = b"alpha: one\nbeta: two\ngamma: three\n";
        let map: HashMap<String, String> = from_slice(yaml)?;
        assert_eq!(map.get("alpha"), Some(&"one".to_string()));
        assert_eq!(map.get("beta"), Some(&"two".to_string()));
        assert_eq!(map.get("gamma"), Some(&"three".to_string()));
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_job_struct_with_minimal_fields() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: test-job\nstate: PENDING\nposition: 5\ntaskCount: 3\nprogress: 0.5\n";
        let job: Job = from_slice(yaml)?;
        assert_eq!(job.name, Some("test-job".to_string()));
        assert_eq!(job.state, "PENDING");
        assert_eq!(job.position, 5);
        assert_eq!(job.task_count, 3);
        assert!((job.progress - 0.5).abs() < 1e-10);
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_task_struct_with_image_and_queue() -> Result<(), ApiError> {
        use twerk_core::task::Task;
        let yaml =
            b"name: build\nimage: node:18\nqueue: default\nposition: 0\npriority: 1\nprogress: 0.0\nredelivered: 0\n";
        let task: Task = from_slice(yaml)?;
        assert_eq!(task.name, Some("build".to_string()));
        assert_eq!(task.image, Some("node:18".to_string()));
        assert_eq!(task.queue, Some("default".to_string()));
        assert_eq!(task.priority, 1);
        Ok(())
    }

    #[test]
    fn from_slice_returns_bad_request_when_missing_required_field() {
        #[derive(Debug, serde::Deserialize)]
        struct Required {
            name: String,
        }
        let result: Result<Required, ApiError> = from_slice(b"other: value");
        let Err(ApiError::BadRequest(msg)) = result else {
            panic!("expected BadRequest for missing required field");
        };
        assert!(
            msg.contains("YAML parse error"),
            "expected parse error message, got: {msg}"
        );
    }

    #[test]
    fn from_slice_handles_multiline_block_scalar() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Doc {
            content: String,
        }
        let yaml = b"content: |\n  line one\n  line two\n  line three\n";
        let parsed: Doc = from_slice(yaml)?;
        assert!(parsed.content.contains("line one"));
        assert!(parsed.content.contains("line two"));
        assert!(parsed.content.contains("line three"));
        Ok(())
    }

    #[test]
    fn from_slice_handles_double_quoted_strings() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Quote {
            value: String,
        }
        let yaml = br#"value: "hello: world""#;
        let result: Result<Quote, ApiError> = from_slice(yaml);
        assert_eq!(result.map(|q| q.value), Ok("hello: world".to_string()));
    }

    #[test]
    fn from_slice_handles_flow_sequence() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Seq {
            items: Vec<String>,
        }
        let yaml = b"items: [a, b, c]\n";
        let parsed: Seq = from_slice(yaml)?;
        assert_eq!(parsed.items, vec!["a", "b", "c"]);
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_empty_hashmap() -> Result<(), ApiError> {
        use std::collections::HashMap;
        let yaml = b"{}\n";
        let result: HashMap<String, String> = from_slice(yaml)?;
        assert!(result.is_empty());
        Ok(())
    }

    #[test]
    fn from_slice_handles_yaml_with_trailing_newlines() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct KV {
            key: String,
        }
        let yaml = b"key: value\n\n\n";
        let result: KV = from_slice(yaml)?;
        assert_eq!(result.key, "value");
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_job_with_tasks() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: multi-task\nstate: PENDING\ntasks:\n  - name: step1\n    image: alpine\n    position: 0\n    priority: 0\n    progress: 0.0\n    redelivered: 0\n  - name: step2\n    image: node:18\n    position: 0\n    priority: 0\n    progress: 0.0\n    redelivered: 0\nposition: 0\ntaskCount: 2\nprogress: 0.0\n";
        let job: Job = from_slice(yaml)?;
        assert_eq!(job.name, Some("multi-task".to_string()));
        let tasks = job.tasks.as_ref();
        assert_eq!(tasks.map(|t| t.len()), Some(2));
        assert_eq!(
            tasks.and_then(|t| t.first().and_then(|task| task.name.clone())),
            Some("step1".to_string())
        );
        assert_eq!(
            tasks.and_then(|t| t.get(1).and_then(|task| task.name.clone())),
            Some("step2".to_string())
        );
        Ok(())
    }

    #[test]
    fn from_slice_returns_bad_request_for_embedded_null_byte() {
        // \x00 is valid UTF-8 (U+0000) but serde_yaml2 rejects it at parse time
        let bad: &[u8] = b"key: \x00value";
        let result: Result<serde_json::Value, ApiError> = from_slice(bad);
        let Err(ApiError::BadRequest(msg)) = result else {
            panic!("expected BadRequest for null byte, got {result:?}");
        };
        assert!(
            msg.contains("YAML parse error"),
            "expected YAML parse error, got: {msg}"
        );
    }

    // ── proptest property tests for measure_max_nesting ─────────────────

    proptest! {
        #[test]
        fn measure_max_nesting_is_deterministic(input in ".*") {
            let first = measure_max_nesting(&input);
            let second = measure_max_nesting(&input);
            prop_assert_eq!(first, second);
        }

        #[test]
        fn measure_max_nesting_never_exceeds_line_length_over_two(input in ".*") {
            let max_possible = input
                .lines()
                .map(|line| line.len() / 2)
                .max()
                .unwrap_or(0);
            let result = measure_max_nesting(&input);
            prop_assert!(result <= max_possible);
        }

        #[test]
        fn measure_max_nesting_not_decreased_by_appending_blank_line(
            base in "[a-zA-Z: \\n]{0,100}"
        ) {
            let base_depth = measure_max_nesting(&base);
            let extended = format!("{base}\n");
            let extended_depth = measure_max_nesting(&extended);
            prop_assert!(extended_depth >= base_depth);
        }

        #[test]
        fn measure_max_nesting_not_decreased_by_appending_comment(
            base in "[a-zA-Z: \\n]{0,100}"
        ) {
            let base_depth = measure_max_nesting(&base);
            let extended = format!("{base}\n# a comment\n");
            let extended_depth = measure_max_nesting(&extended);
            prop_assert!(extended_depth >= base_depth);
        }

        #[test]
        fn measure_max_nesting_returns_zero_for_all_comment_lines(
            lines in prop::collection::vec("#[^\n]*", 0..20)
        ) {
            let input = lines.join("\n");
            prop_assert_eq!(measure_max_nesting(&input), 0);
        }

        #[test]
        fn measure_max_nesting_upper_bounded_by_max_indent_spaces(
            spaces in 0usize..200usize
        ) {
            let line = format!("{}key: val", " ".repeat(spaces));
            let depth = measure_max_nesting(&line);
            prop_assert_eq!(depth, spaces / 2);
        }

        #[test]
        fn measure_max_nesting_ignores_tab_only_indentation(
            lines in prop::collection::vec("\\t+[a-z]+: val", 0..20)
        ) {
            let input = lines.join("\n");
            prop_assert_eq!(measure_max_nesting(&input), 0);
        }
    }

    // ── test-reviewer mandates (B8, B9, B10, B18, empty) ────────────────

    #[test]
    fn from_slice_accepts_yaml_at_exactly_max_depth() -> Result<(), ApiError> {
        let input = (0..=MAX_YAML_DEPTH)
            .map(|i| format!("{}k{i}: v{i}", " ".repeat(i * 2)))
            .collect::<Vec<String>>()
            .join("\n");
        assert_eq!(measure_max_nesting(&input), MAX_YAML_DEPTH);
        assert!(validate_yaml_depth(&input).is_ok());
        Ok(())
    }

    #[test]
    fn from_slice_returns_none_for_absent_option_field() {
        #[derive(serde::Deserialize, PartialEq, Debug)]
        struct Optional {
            name: String,
            description: Option<String>,
        }
        let result: Result<Optional, ApiError> = from_slice(b"name: hello");
        assert_eq!(result.map(|o| o.description), Ok(None));
    }

    #[test]
    fn from_slice_uses_default_for_absent_field() {
        #[derive(serde::Deserialize, PartialEq, Debug)]
        struct WithDefault {
            name: String,
            #[serde(default)]
            count: i64,
        }
        let result: Result<WithDefault, ApiError> = from_slice(b"name: hello");
        assert_eq!(result.map(|w| w.count), Ok(0));
    }

    #[test]
    fn from_slice_ignores_unknown_yaml_fields() {
        #[derive(serde::Deserialize, PartialEq, Debug)]
        struct Strict {
            name: String,
        }
        let result: Result<Strict, ApiError> = from_slice(b"name: hello\nunknown: value");
        assert_eq!(result.map(|s| s.name), Ok("hello".to_string()));
    }

    #[test]
    fn from_slice_handles_empty_body() {
        let result: Result<serde_json::Value, ApiError> = from_slice(b"");
        let Err(ApiError::BadRequest(msg)) = result else {
            panic!("expected BadRequest for empty body, got {result:?}");
        };
        assert!(
            msg.contains("YAML parse error"),
            "expected parse error for empty input, got: {msg}"
        );
    }
}
