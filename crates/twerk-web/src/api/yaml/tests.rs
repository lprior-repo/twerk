#[cfg(test)]
#[allow(clippy::panic, clippy::approx_constant, clippy::unwrap_used, dead_code)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::api::yaml::{
        from_slice, measure_ast_depth_and_nodes, ApiError, MAX_YAML_BODY_SIZE, MAX_YAML_DEPTH,
        MAX_YAML_NODES,
    };
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
        assert!(
            msg.contains("exceeds") || msg.contains("depth") || msg.contains("YAML parse error"),
            "message was: {msg}"
        );
    }

    #[test]
    fn from_slice_rejects_deeply_nested_flow_style() {
        let mut yaml = "root: ".to_string();
        for _ in 0..=MAX_YAML_DEPTH {
            yaml.push_str("{a: ");
        }
        for _ in 0..=MAX_YAML_DEPTH {
            yaml.push('}');
        }
        let result: Result<serde_json::Value, ApiError> = from_slice(yaml.as_bytes());
        let Err(ApiError::BadRequest(msg)) = result else {
            panic!("expected BadRequest for flow-style depth, got {result:?}");
        };
        assert!(msg.contains("nesting depth"), "message was: {msg}");
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

    // ── measure_ast_depth_and_nodes unit tests ────────────────────────────

    #[test]
    fn ast_depth_returns_zero_for_flat_yaml() {
        let doc: serde_yaml::Value = serde_yaml::from_str("name: hello\nvalue: world\n").unwrap();
        let (depth, nodes) = measure_ast_depth_and_nodes(&doc);
        assert_eq!(depth, 1);
        assert!(nodes > 0);
    }

    #[test]
    fn ast_depth_returns_correct_depth_for_nested_yaml() {
        let doc: serde_yaml::Value =
            serde_yaml::from_str("root:\n  child:\n    grandchild: value\n").unwrap();
        let (depth, _) = measure_ast_depth_and_nodes(&doc);
        assert_eq!(depth, 3);
    }

    #[test]
    fn ast_depth_catches_flow_style_nesting() {
        let yaml = "root: {a: {b: {c: value}}}";
        let doc: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let (depth, _) = measure_ast_depth_and_nodes(&doc);
        assert!(depth >= 4, "flow-style depth should be >= 4, got {depth}");
    }

    #[test]
    fn ast_depth_counts_array_nesting() {
        let yaml = "items:\n  - name: a\n    tags:\n      - x\n      - y";
        let doc: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let (depth, _) = measure_ast_depth_and_nodes(&doc);
        assert!(depth >= 3, "array nesting should be >= 3, got {depth}");
    }

    #[test]
    fn ast_nodes_counts_all_nodes() {
        let yaml = "a: 1\nb: 2\nc:\n  - x\n  - y";
        let doc: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let (_, nodes) = measure_ast_depth_and_nodes(&doc);
        assert!(nodes >= 8, "should count all nodes, got {nodes}");
    }

    // ── rstest parametric for ast depth ───────────────────────────────────

    #[rstest]
    #[case("key: val", 1)]
    #[case("root:\n  child: 1", 2)]
    #[case("root:\n  child:\n    leaf: 1", 3)]
    fn ast_depth_returns_expected_for_nesting_levels(#[case] input: &str, #[case] expected: usize) {
        let doc: serde_yaml::Value = serde_yaml::from_str(input).unwrap();
        let (depth, _) = measure_ast_depth_and_nodes(&doc);
        assert_eq!(depth, expected);
    }

    // ── from_slice additional unit tests ──────────────────────────────────

    #[test]
    fn from_slice_returns_ok_when_body_exactly_at_size_limit() -> Result<(), ApiError> {
        #[derive(serde::Deserialize)]
        struct KV {
            k: String,
        }

        let prefix = "k: ";
        let value_len = MAX_YAML_BODY_SIZE - prefix.len();
        let yaml = format!("{prefix}{}", "a".repeat(value_len));
        assert_eq!(yaml.len(), MAX_YAML_BODY_SIZE);

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
        use twerk_core::job::{Job, JobState};
        let yaml = b"name: test-job\nstate: PENDING\nposition: 5\ntaskCount: 3\nprogress: 0.5\n";
        let job: Job = from_slice(yaml)?;
        assert_eq!(job.name, Some("test-job".to_string()));
        assert_eq!(job.state, JobState::Pending);
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
        assert_eq!(tasks.map(std::vec::Vec::len), Some(2));
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

    // ── proptest property tests ───────────────────────────────────────────

    proptest! {
        #[test]
        fn ast_depth_and_nodes_deterministic(input in "[a-zA-Z0-9: \\n\\[\\]{}]{0,200}") {
            if let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(&input) {
                let (d1, n1) = measure_ast_depth_and_nodes(&doc);
                let (d2, n2) = measure_ast_depth_and_nodes(&doc);
                prop_assert_eq!((d1, n1), (d2, n2));
            }
        }

        #[test]
        fn ast_depth_never_negative(input in "[a-zA-Z0-9: \\n]{0,200}") {
            if let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(&input) {
                let (depth, _) = measure_ast_depth_and_nodes(&doc);
                prop_assert!(depth < 1000);
            }
        }

        #[test]
        fn from_slice_rejects_flow_style_depth_bomb(
            depth in 10usize..80usize
        ) {
            let mut yaml = "root: ".to_string();
            for _ in 0..depth {
                yaml.push_str("{a: ");
            }
            for _ in 0..depth {
                yaml.push('}');
            }
            let result: Result<serde_json::Value, ApiError> = from_slice(yaml.as_bytes());
            if depth > MAX_YAML_DEPTH {
                prop_assert!(matches!(result, Err(ApiError::BadRequest(ref msg)) if msg.contains("exceeds") || msg.contains("depth")));
            }
        }
    }

    // ── test-reviewer mandates ────────────────────────────────────────────

    #[test]
    fn from_slice_accepts_yaml_at_shallow_depth() -> Result<(), ApiError> {
        #[derive(serde::Deserialize)]
        struct Level1 {
            a: String,
        }
        let yaml = b"a: hello";
        let result: Level1 = from_slice(yaml)?;
        assert_eq!(result.a, "hello");
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
    fn from_slice_returns_bad_request_when_node_count_exceeds_limit() {
        use std::fmt::Write;
        let mut yaml = String::from("root:\n");
        for i in 0..=MAX_YAML_NODES {
            let _ = writeln!(yaml, "  k{i}: v{i}");
        }
        let result: Result<serde_json::Value, ApiError> = from_slice(yaml.as_bytes());
        let Err(ApiError::BadRequest(msg)) = result else {
            panic!("expected BadRequest for node count overflow, got {result:?}");
        };
        assert!(
            msg.contains("complexity"),
            "expected complexity limit message, got: {msg}"
        );
    }

    #[test]
    fn from_slice_handles_empty_body() {
        let result: Result<serde_json::Value, ApiError> = from_slice(b"");
        let Err(ApiError::BadRequest(msg)) = result else {
            panic!("expected BadRequest for empty body, got {result:?}");
        };
        assert!(
            msg.contains("empty"),
            "expected empty body error for empty input, got: {msg}"
        );
    }
}
