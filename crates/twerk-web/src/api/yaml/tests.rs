#[cfg(test)]
#[allow(
    clippy::panic,
    clippy::approx_constant,
    clippy::unwrap_used,
    clippy::expect_used,
    dead_code
)]
mod yaml_suite {
    use crate::api::yaml::{from_slice, ApiError, MAX_YAML_BODY_SIZE};
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
        let bad_yaml = b": \xff: invalid";
        let result: Result<serde_json::Value, ApiError> = from_slice(bad_yaml);
        assert!(matches!(result, Err(ApiError::BadRequest(msg)) if msg.contains("UTF-8")));
    }

    #[test]
    fn from_slice_returns_bad_request_when_body_exceeds_size_limit() {
        let oversized = vec![b'x'; MAX_YAML_BODY_SIZE + 1];
        let result: Result<serde_json::Value, ApiError> = from_slice(&oversized);
        assert!(matches!(result, Err(ApiError::BadRequest(msg)) if msg.contains("exceeds")));
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
    fn from_slice_deserializes_empty_string_value() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct EmptyVal {
            value: String,
        }
        let yaml = b"value: \"\"\n";
        let result: EmptyVal = from_slice(yaml)?;
        assert_eq!(result.value, "");
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_whitespace_only_value() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize)]
        struct WhitespaceVal {
            value: String,
        }
        let yaml = b"value: \"   \\t  \"\n";
        let result: WhitespaceVal = from_slice(yaml)?;
        assert_eq!(result.value, "   \t  ");
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_very_large_integer() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize)]
        struct BigInt {
            value: i64,
        }
        let yaml = b"value: 9223372036854775807\n";
        let result: BigInt = from_slice(yaml)?;
        assert_eq!(result.value, i64::MAX);
        Ok(())
    }

    #[test]
    fn from_slice_handles_single_quoted_strings() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct QuoteVal {
            value: String,
        }
        let yaml = b"value: 'hello world'\n";
        let result: QuoteVal = from_slice(yaml)?;
        assert_eq!(result.value, "hello world");
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_multiline_literal_block() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize)]
        struct BlockVal {
            content: String,
        }
        let yaml = b"content: |\n  line one\n  line two\n  line three\n";
        let result: BlockVal = from_slice(yaml)?;
        assert!(result.content.contains("line one"));
        assert!(result.content.contains("line two"));
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_multiline_folded_block() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize)]
        struct FoldedVal {
            content: String,
        }
        let yaml = b"content: >\n  line one\n  line two\n  line three\n";
        let result: FoldedVal = from_slice(yaml)?;
        assert!(result.content.contains("line one"));
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_zero_integer() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct ZeroVal {
            count: i32,
        }
        let yaml = b"count: 0\n";
        let result: ZeroVal = from_slice(yaml)?;
        assert_eq!(result.count, 0);
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_zero_float() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct ZeroFloat {
            value: f64,
        }
        let yaml = b"value: 0.0\n";
        let result: ZeroFloat = from_slice(yaml)?;
        assert!((result.value - 0.0).abs() < f64::EPSILON);
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_negative_integer() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct FloatVal {
            value: f64,
        }
        let yaml = b"value: -3.14159\n";
        let result: FloatVal = from_slice(yaml)?;
        assert!((result.value - (-3.14159)).abs() < 1e-10);
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_float_scientific_notation() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct SciVal {
            value: f64,
        }
        let yaml = b"value: 1.5e10\n";
        let result: SciVal = from_slice(yaml)?;
        assert!((result.value - 1.5e10).abs() < 1e-1);
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_hash_with_integer_keys() -> Result<(), ApiError> {
        use std::collections::HashMap;
        let yaml = b"1: one\n2: two\n3: three\n";
        let map: HashMap<String, String> = from_slice(yaml)?;
        assert_eq!(map.get("1"), Some(&"one".to_string()));
        assert_eq!(map.get("2"), Some(&"two".to_string()));
        assert_eq!(map.get("3"), Some(&"three".to_string()));
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_nested_arrays() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Nested {
            matrix: Vec<Vec<i32>>,
        }
        let yaml = b"matrix: [[1, 2], [3, 4]]\n";
        let result: Nested = from_slice(yaml)?;
        assert_eq!(result.matrix, vec![vec![1, 2], vec![3, 4]]);
        Ok(())
    }

    #[test]
    fn from_slice_deserializes_mixed_array_types() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize)]
        struct Mixed {
            items: Vec<serde_json::Value>,
        }
        let yaml = b"items: [1, \"two\", true, null]\n";
        let result: Mixed = from_slice(yaml)?;
        assert_eq!(result.items[0], serde_json::json!(1));
        assert_eq!(result.items[1], serde_json::json!("two"));
        assert_eq!(result.items[2], serde_json::json!(true));
        assert_eq!(result.items[3], serde_json::Value::Null);
        Ok(())
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

    #[test]
    fn from_slice_rejects_invalid_job_state() {
        let yaml = b"name: test\nstate: INVALID_STATE\n";
        let result: Result<twerk_core::job::Job, ApiError> = from_slice(yaml);
        assert!(result.is_err(), "Invalid JobState should fail");
    }

    #[test]
    fn from_slice_accepts_job_state_uppercase() -> Result<(), ApiError> {
        use twerk_core::job::{Job, JobState};
        let yaml = b"name: test\nstate: PENDING\n";
        let job: Job = from_slice(yaml)?;
        assert_eq!(job.state, JobState::Pending);
        Ok(())
    }

    #[test]
    fn from_slice_job_with_all_optional_fields_absent() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: minimal-job\n";
        let job: Job = from_slice(yaml)?;
        assert_eq!(job.name, Some("minimal-job".to_string()));
        assert_eq!(job.state, twerk_core::job::JobState::Pending);
        assert!(job.id.is_none());
        assert!(job.description.is_none());
        assert!(job.tasks.is_none());
        Ok(())
    }

    #[test]
    fn from_slice_job_with_empty_tasks_array() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: empty-tasks-job\ntasks: []\n";
        let job: Job = from_slice(yaml)?;
        assert_eq!(job.tasks, Some(vec![]));
        Ok(())
    }

    #[test]
    fn from_slice_job_with_task_defaults() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: job-with-task\ntasks:\n  - name: step1\n    image: alpine\n";
        let job: Job = from_slice(yaml)?;
        let tasks = job.tasks.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, Some("step1".to_string()));
        assert_eq!(tasks[0].image, Some("alpine".to_string()));
        assert_eq!(tasks[0].priority, 0);
        assert_eq!(tasks[0].position, 0);
        Ok(())
    }

    #[test]
    fn from_slice_job_with_camelcase_fields() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: camelCase-test\ntaskCount: 5\nprogress: 0.75\n";
        let job: Job = from_slice(yaml)?;
        assert_eq!(job.name, Some("camelCase-test".to_string()));
        assert_eq!(job.task_count, 5);
        assert!((job.progress - 0.75).abs() < 1e-10);
        Ok(())
    }

    #[test]
    fn from_slice_job_with_webhooks() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml =
            b"name: webhook-job\nwebhooks:\n  - url: https://example.com/hook\n    method: POST\n";
        let job: Job = from_slice(yaml)?;
        let webhooks = job.webhooks.unwrap();
        assert_eq!(webhooks.len(), 1);
        assert_eq!(webhooks[0].url.as_deref(), Some("https://example.com/hook"));
        Ok(())
    }

    #[test]
    fn from_slice_job_with_schedule() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: scheduled-job\nschedule:\n  cron: \"0 0 * * *\"\n";
        let job: Job = from_slice(yaml)?;
        let schedule = job.schedule.unwrap();
        assert!(schedule.cron.is_some());
        Ok(())
    }

    #[test]
    fn from_slice_job_with_inputs() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: input-job\ninputs:\n  env: production\n  version: \"1.0\"\n";
        let job: Job = from_slice(yaml)?;
        let inputs = job.inputs.unwrap();
        assert_eq!(inputs.get("env"), Some(&"production".to_string()));
        assert_eq!(inputs.get("version"), Some(&"1.0".to_string()));
        Ok(())
    }

    #[test]
    fn from_slice_job_with_tags() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: tagged-job\ntags:\n  - frontend\n  - api\n  - v2\n";
        let job: Job = from_slice(yaml)?;
        let tags = job.tags.unwrap();
        assert_eq!(tags, vec!["frontend", "api", "v2"]);
        Ok(())
    }

    #[test]
    fn from_slice_job_progress_clamps_to_valid_range() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: progress-job\nprogress: 1.5\n";
        let job: Job = from_slice(yaml)?;
        assert!((job.progress - 1.5).abs() < 1e-10);
        Ok(())
    }

    #[test]
    fn from_slice_accepts_numeric_image_with_serde_saphyr() {
        let yaml = b"name: bad-task\ntasks:\n  - name: step1\n    image: 12345\n";
        let result: Result<twerk_core::job::Job, ApiError> = from_slice(yaml);
        assert!(
            result.is_ok(),
            "serde_saphyr may coerce numeric to string: {:?}",
            result
        );
    }

    #[test]
    fn from_slice_rejects_job_with_invalid_task_priority_type() {
        let yaml = b"name: bad-priority\ntasks:\n  - name: step1\n    image: alpine\n    priority: not_a_number\n";
        let result: Result<twerk_core::job::Job, ApiError> = from_slice(yaml);
        assert!(result.is_err(), "Task priority should be an integer");
    }

    #[test]
    fn from_slice_job_with_context() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: context-job\ncontext:\n  execution_id: exec-123\n  attempt: 1\n";
        let job: Job = from_slice(yaml)?;
        assert!(job.context.is_some());
        Ok(())
    }

    #[test]
    fn from_slice_job_with_null_description_becomes_none() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: test\ndescription: null\n";
        let job: Job = from_slice(yaml)?;
        assert_eq!(job.description, None);
        Ok(())
    }

    #[test]
    fn from_slice_job_with_auto_delete() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: delete-job\nautoDelete:\n  after: after_success\n";
        let job: Job = from_slice(yaml)?;
        assert!(job.auto_delete.is_some());
        assert_eq!(
            job.auto_delete.unwrap().after,
            Some("after_success".to_string())
        );
        Ok(())
    }

    #[test]
    fn parse_all_example_yaml_files() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let workspace_root = std::path::Path::new(manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .expect("could not resolve workspace root");
        let examples_dir = workspace_root.join("examples");

        let files = [
            "hello.yaml",
            "parallel.yaml",
            "retry.yaml",
            "each.yaml",
            "subjob.yaml",
            "timeout.yaml",
            "bash-subjob.yaml",
            "bash-retry.yaml",
            "bash-quick.yaml",
            "bash-pipeline.yaml",
            "bash-each.yaml",
            "bash-ci-pipeline.yaml",
            "bash-ci-demo.yaml",
            "split_and_stitch.yaml",
        ];

        files.iter().for_each(|name| {
            let file = examples_dir.join(name);
            let content = std::fs::read_to_string(&file)
                .unwrap_or_else(|_| panic!("Failed to read {}", file.display()));
            let result: Result<serde_json::Value, _> = from_slice(content.as_bytes());
            assert!(
                result.is_ok(),
                "Failed to parse {}: {:?}",
                file.display(),
                result.err()
            );
        });
    }

    #[test]
    fn from_slice_parses_whitespace_only_as_null() {
        let result: Result<serde_json::Value, ApiError> = from_slice(b"   \n\t  \r\n");
        assert!(
            matches!(result, Ok(serde_json::Value::Null)),
            "whitespace-only YAML should parse as null"
        );
    }

    #[test]
    fn from_slice_returns_bad_request_for_binary_content_0x00() {
        let result: Result<serde_json::Value, ApiError> = from_slice(b"\x00");
        assert!(result.is_err(), "0x00 byte should be rejected");
    }

    #[test]
    fn from_slice_accepts_control_chars_0x01_to_0x1f_in_strings() {
        #[derive(Debug, serde::Deserialize)]
        struct WithControl {
            value: String,
        }
        let yaml = br#"value: "hello world""#;
        let result: Result<WithControl, ApiError> = from_slice(yaml);
        assert!(
            result.is_ok(),
            "quoted string should be accepted: {result:?}"
        );
    }

    #[test]
    fn from_slice_accepts_del_0x7f_in_quoted_strings() {
        #[derive(Debug, serde::Deserialize)]
        struct WithDel {
            value: String,
        }
        let yaml = br#"value: "hello\x7fworld""#;
        let result: Result<WithDel, ApiError> = from_slice(yaml);
        assert!(
            result.is_ok(),
            "DEL in quoted string should be accepted: {result:?}"
        );
    }

    #[test]
    fn from_slice_returns_bad_request_for_binary_content_0x80_to_0xff() {
        let invalid_bytes: Vec<u8> = (0x80..=0xFF).collect();
        let result: Result<serde_json::Value, ApiError> = from_slice(&invalid_bytes);
        assert!(
            result.is_err(),
            "high bytes 0x80-0xFF should be rejected as invalid UTF-8"
        );
    }

    #[test]
    fn from_slice_returns_bad_request_for_mixed_valid_and_invalid_utf8() {
        let data: Vec<u8> = b"key: value \xff\xfe".to_vec();
        let result: Result<serde_json::Value, ApiError> = from_slice(&data);
        assert!(
            result.is_err(),
            "mixed valid/invalid UTF-8 should be rejected"
        );
    }

    #[test]
    fn from_slice_returns_bad_request_for_truncated_utf8_sequence() {
        let data: Vec<u8> = b"key: \xc3".to_vec();
        let result: Result<serde_json::Value, ApiError> = from_slice(&data);
        assert!(
            result.is_err(),
            "truncated UTF-8 sequence should be rejected"
        );
    }

    #[test]
    fn from_slice_accepts_yaml_with_tabs_in_quoted_strings() {
        #[derive(Debug, serde::Deserialize)]
        struct WithTab {
            value: String,
        }
        let yaml = b"value: \"hello\\tworld\"";
        let result: Result<WithTab, ApiError> = from_slice(yaml);
        assert!(
            result.is_ok(),
            "tab in quoted string should be accepted: {result:?}"
        );
    }

    #[test]
    fn from_slice_accepts_unicode_value() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct UnicodeVal {
            greeting: String,
        }
        let yaml = "greeting: 🎉 Hello 世界".as_bytes();
        let result: Result<UnicodeVal, ApiError> = from_slice(yaml);
        assert!(
            result.is_ok(),
            "unicode in value should be accepted: {result:?}"
        );
    }

    #[test]
    fn from_slice_handles_printable_ascii_in_values() {
        #[derive(Debug, serde::Deserialize)]
        struct PrintableAscii {
            value: String,
        }
        let yaml = b"value: hello world 123";
        let result: Result<PrintableAscii, ApiError> = from_slice(yaml);
        assert!(
            result.is_ok(),
            "printable ASCII in value should be accepted: {result:?}"
        );
    }

    #[test]
    fn from_slice_deserializes_yaml_alias() -> Result<(), ApiError> {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Doc {
            base: serde_json::Value,
            derived: serde_json::Value,
        }
        let yaml = b"
base: &base_val
  name: original
derived: *base_val
";
        let result: Doc = from_slice(yaml)?;
        assert_eq!(result.base, result.derived);
        Ok(())
    }

    // ── proptest property tests ───────────────────────────────────────────

    proptest! {
        #[test]
        fn from_slice_deterministic(input in "[a-zA-Z0-9: \\n\\[\\]{}]{0,200}") {
            let result1: Result<serde_json::Value, _> = from_slice(input.as_bytes());
            let result2: Result<serde_json::Value, _> = from_slice(input.as_bytes());
            prop_assert_eq!(result1.map(|v| serde_json::to_string(&v).unwrap()), result2.map(|v| serde_json::to_string(&v).unwrap()));
        }
    }

    // ── rstest parametric tests ───────────────────────────────────────────

    #[rstest]
    #[case("key: val", "val")]
    #[case("name: hello", "hello")]
    #[case("value: 42", "42")]
    fn from_slice_parses_simple_key_value(#[case] input: &str, #[case] expected_val: &str) {
        #[derive(Debug, serde::Deserialize)]
        struct Doc {
            #[serde(rename = "key", default)]
            key: Option<String>,
            #[serde(rename = "name", default)]
            name: Option<String>,
            #[serde(rename = "value", default)]
            value: Option<String>,
        }
        let result: Result<Doc, ApiError> = from_slice(input.as_bytes());
        let doc = result.expect("should parse");
        let actual_val = doc.key.or(doc.name).or(doc.value);
        assert_eq!(actual_val, Some(expected_val.to_string()));
    }

    // NOTE: Deep nesting and complexity limits are now handled by serde-saphyr's
    // internal Budget limits. The limits (64 depth, 10000 nodes) are enforced
    // by serde-saphyr's parser. Tests that specifically tested yaml-rust2 AST
    // behavior (measure_ast_depth_and_nodes) have been removed as that code
    // no longer exists.
}
