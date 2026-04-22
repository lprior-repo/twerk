#[cfg(test)]
#[allow(
    clippy::panic,
    clippy::approx_constant,
    clippy::unwrap_used,
    clippy::expect_used,
    dead_code
)]
mod yaml_suite {
    use crate::api::yaml::{from_slice, to_string, ApiError, MAX_YAML_BODY_SIZE};
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
        let bad_yaml = b"items: [1, 2\n";
        let result: Result<serde_json::Value, ApiError> = from_slice(bad_yaml);
        assert!(
            matches!(result, Err(ApiError::BadRequest(msg)) if msg.contains("YAML parse error"))
        );
    }

    #[test]
    fn from_slice_returns_bad_request_when_body_is_empty() {
        let result: Result<serde_json::Value, ApiError> = from_slice(b"");
        assert_eq!(
            result,
            Err(ApiError::BadRequest("YAML body is empty".to_string()))
        );
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
        assert!(matches!(
            result,
            Err(ApiError::BadRequest(msg)) if msg.contains("INVALID_STATE") && msg.contains("unknown")
        ));
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
        let Some(tasks) = job.tasks.as_ref() else {
            panic!("job should include tasks");
        };
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
        let yaml = b"name: webhook-job\nwebhooks:\n  - url: https://example.com/hook\n    event: job.StateChange\n    if: \"{{ job.state == 'COMPLETED' }}\"\n    headers:\n      X-Test: enabled\n";
        let job: Job = from_slice(yaml)?;
        let Some(webhooks) = job.webhooks.as_ref() else {
            panic!("job should include webhooks");
        };
        assert_eq!(webhooks.len(), 1);
        assert_eq!(webhooks[0].url.as_deref(), Some("https://example.com/hook"));
        assert_eq!(webhooks[0].event.as_deref(), Some("job.StateChange"));
        assert_eq!(
            webhooks[0].r#if.as_deref(),
            Some("{{ job.state == 'COMPLETED' }}")
        );
        assert_eq!(
            webhooks[0]
                .headers
                .as_ref()
                .and_then(|headers| headers.get("X-Test")),
            Some(&"enabled".to_string())
        );
        Ok(())
    }

    #[test]
    fn from_slice_job_with_schedule() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: scheduled-job\nschedule:\n  cron: \"0 0 * * *\"\n";
        let job: Job = from_slice(yaml)?;
        let Some(schedule) = job.schedule.as_ref() else {
            panic!("job should include schedule");
        };
        assert_eq!(schedule.cron.as_deref(), Some("0 0 * * *"));
        Ok(())
    }

    #[test]
    fn from_slice_job_with_inputs() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: input-job\ninputs:\n  env: production\n  version: \"1.0\"\n";
        let job: Job = from_slice(yaml)?;
        let Some(inputs) = job.inputs.as_ref() else {
            panic!("job should include inputs");
        };
        assert_eq!(inputs.get("env"), Some(&"production".to_string()));
        assert_eq!(inputs.get("version"), Some(&"1.0".to_string()));
        Ok(())
    }

    #[test]
    fn from_slice_job_with_tags() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: tagged-job\ntags:\n  - frontend\n  - api\n  - v2\n";
        let job: Job = from_slice(yaml)?;
        let Some(tags) = job.tags.as_ref() else {
            panic!("job should include tags");
        };
        assert_eq!(
            tags,
            &["frontend".to_string(), "api".to_string(), "v2".to_string(),]
        );
        Ok(())
    }

    #[test]
    fn from_slice_job_progress_preserves_supplied_value() -> Result<(), ApiError> {
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
        match result {
            Ok(job) => assert_eq!(
                job.tasks
                    .as_ref()
                    .and_then(|tasks| tasks.first())
                    .and_then(|task| task.image.as_deref()),
                Some("12345")
            ),
            Err(err) => panic!("numeric image should coerce to string, got {err:?}"),
        }
    }

    #[test]
    fn from_slice_rejects_job_with_invalid_task_priority_type() {
        let yaml = b"name: bad-priority\ntasks:\n  - name: step1\n    image: alpine\n    priority: not_a_number\n";
        let result: Result<twerk_core::job::Job, ApiError> = from_slice(yaml);
        assert!(
            matches!(result, Err(ApiError::BadRequest(msg)) if msg.contains("priority") || msg.contains("integer") || msg.contains("invalid type"))
        );
    }

    #[test]
    fn from_slice_job_with_context() -> Result<(), ApiError> {
        use twerk_core::job::Job;
        let yaml = b"name: context-job\ncontext:\n  job:\n    execution_id: exec-123\n  inputs:\n    region: us-west-2\n  tasks:\n    previous: complete\n";
        let job: Job = from_slice(yaml)?;
        let Some(context) = job.context.as_ref() else {
            panic!("job should include context");
        };
        assert_eq!(
            context
                .job
                .as_ref()
                .and_then(|values| values.get("execution_id")),
            Some(&"exec-123".to_string())
        );
        assert_eq!(
            context
                .inputs
                .as_ref()
                .and_then(|values| values.get("region")),
            Some(&"us-west-2".to_string())
        );
        assert_eq!(
            context
                .tasks
                .as_ref()
                .and_then(|values| values.get("previous")),
            Some(&"complete".to_string())
        );
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
        let Some(auto_delete) = job.auto_delete.as_ref() else {
            panic!("job should include auto_delete");
        };
        assert_eq!(auto_delete.after, Some("after_success".to_string()));
        Ok(())
    }

    fn assert_bad_request_contains<T: std::fmt::Debug>(result: Result<T, ApiError>, needle: &str) {
        match result {
            Err(ApiError::BadRequest(msg)) => {
                assert!(
                    msg.contains(needle),
                    "expected BadRequest containing {needle:?}, got {msg:?}"
                );
            }
            other => panic!("expected BadRequest containing {needle:?}, got {other:?}"),
        }
    }

    fn assert_internal_contains<T: std::fmt::Debug>(result: Result<T, ApiError>, needle: &str) {
        match result {
            Err(ApiError::Internal(msg)) => {
                assert!(
                    msg.contains(needle),
                    "expected Internal containing {needle:?}, got {msg:?}"
                );
            }
            other => panic!("expected Internal containing {needle:?}, got {other:?}"),
        }
    }

    fn workspace_root() -> std::path::PathBuf {
        match std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
        {
            Some(path) => path.to_path_buf(),
            None => panic!("could not resolve workspace root"),
        }
    }

    fn example_path(name: &str) -> std::path::PathBuf {
        workspace_root().join("examples").join(name)
    }

    fn example_bytes(name: &str) -> Vec<u8> {
        let path = example_path(name);
        std::fs::read(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
    }

    fn parse_job_example(name: &str) -> twerk_core::job::Job {
        from_slice::<twerk_core::job::Job>(&example_bytes(name))
            .unwrap_or_else(|err| panic!("failed to parse job example {name}: {err:?}"))
    }

    fn parse_state_machine_example(name: &str) -> twerk_core::asl::machine::StateMachine {
        from_slice::<twerk_core::asl::machine::StateMachine>(&example_bytes(name))
            .unwrap_or_else(|err| panic!("failed to parse ASL example {name}: {err:?}"))
    }

    fn job_value(name: &str) -> serde_json::Value {
        serde_json::to_value(parse_job_example(name))
            .unwrap_or_else(|err| panic!("failed to convert parsed job {name} to JSON: {err}"))
    }

    fn state_machine_value(name: &str) -> serde_json::Value {
        serde_json::to_value(parse_state_machine_example(name)).unwrap_or_else(|err| {
            panic!("failed to convert parsed ASL machine {name} to JSON: {err}")
        })
    }

    #[rstest]
    #[case("bash-ci-demo.yaml", "bash-ci-demo", 4)]
    #[case("bash-ci-pipeline.yaml", "bash-ci-pipeline", 5)]
    #[case("bash-each.yaml", "bash-each-demo", 1)]
    #[case("bash-pipeline.yaml", "bash-pipeline-demo", 5)]
    #[case("bash-quick.yaml", "bash-quick-test", 1)]
    #[case("bash-retry.yaml", "bash-retry-demo", 2)]
    #[case("bash-subjob.yaml", "bash-subjob-demo", 4)]
    #[case("each.yaml", "sample each job", 4)]
    #[case("hello-shell.yaml", "hello shell", 1)]
    #[case("hello.yaml", "hello world", 1)]
    #[case("parallel.yaml", "sample parallel job", 3)]
    #[case("pokemon-benchmark.yaml", "pokemon-api-benchmark", 5)]
    #[case("retry.yaml", "sample retry job", 1)]
    #[case("split_and_stitch.yaml", "split and stitch demo", 10)]
    #[case("subjob.yaml", "sample job with sub jobs", 4)]
    #[case("timeout.yaml", "sample timeout job", 1)]
    #[case("twerk-chaos-engineering.yaml", "twerk-chaos-engineering", 13)]
    #[case("twerk-massive-parallel.yaml", "twerk-massive-parallel", 2)]
    #[case("twerk-noop-100.yaml", "twerk-noop-stress", 100)]
    #[case("twerk-pokemon-shell-100.yaml", "twerk-pokemon-shell-stress", 100)]
    fn parse_native_example_job_with_expected_name_and_task_count(
        #[case] file: &str,
        #[case] expected_name: &str,
        #[case] expected_task_count: usize,
    ) {
        let job = parse_job_example(file);
        assert_eq!(job.name.as_deref(), Some(expected_name));
        assert_eq!(
            job.tasks.as_ref().map(std::vec::Vec::len),
            Some(expected_task_count)
        );
    }

    #[rstest]
    #[case("asl-hello.yaml", "Hello", 2)]
    #[case("asl-task-retry.yaml", "Build", 3)]
    fn parse_asl_example_state_machine_with_expected_start_and_state_count(
        #[case] file: &str,
        #[case] expected_start_at: &str,
        #[case] expected_state_count: usize,
    ) {
        let machine = parse_state_machine_example(file);
        let machine_value = state_machine_value(file);
        assert_eq!(machine.start_at().as_ref(), expected_start_at);
        assert_eq!(machine.states().len(), expected_state_count);
        assert_ne!(
            machine_value["states"][expected_start_at],
            serde_json::Value::Null
        );
    }

    #[test]
    fn hello_example_preserves_output_contract() {
        let value = job_value("hello.yaml");
        assert_eq!(value["output"], "{{ tasks.hello }}");
        assert_eq!(value["tasks"][0]["var"], "hello");
        assert_eq!(value["tasks"][0]["name"], "simple task");
    }

    #[test]
    fn each_example_preserves_default_iteration_shape() {
        let value = job_value("each.yaml");
        assert_eq!(value["tasks"][1]["each"]["list"], "{{ sequence(1,5) }}");
        assert_eq!(
            value["tasks"][1]["each"]["task"]["var"],
            "eachTask{{item_index}}"
        );
        assert_eq!(
            value["tasks"][1]["each"]["task"]["env"]["ITEM"],
            "{{item_value}}"
        );
        assert_eq!(value["tasks"][2]["each"]["var"], "myitem");
        assert_eq!(
            value["tasks"][2]["each"]["task"]["var"],
            "eachTask{{myitem_index}}"
        );
        assert_eq!(
            value["tasks"][2]["each"]["task"]["env"]["ITEM"],
            "{{myitem_value}}"
        );
    }

    #[test]
    fn bash_each_example_preserves_custom_iteration_env() {
        let value = job_value("bash-each.yaml");
        assert_eq!(value["tasks"][0]["each"]["var"], "num");
        assert_eq!(value["tasks"][0]["each"]["list"], "{{ sequence(1, 3) }}");
        assert_eq!(
            value["tasks"][0]["each"]["task"]["env"]["NUM"],
            "{{num_value}}"
        );
        assert_eq!(
            value["tasks"][0]["each"]["task"]["env"]["IDX"],
            "{{num_index}}"
        );
    }

    #[test]
    fn parallel_example_preserves_parallel_children() {
        let value = job_value("parallel.yaml");
        assert_eq!(value["tasks"][1]["name"], "a parallel task");
        assert_eq!(
            value["tasks"][1]["parallel"]["tasks"]
                .as_array()
                .map(std::vec::Vec::len),
            Some(6)
        );
        assert_eq!(
            value["tasks"][1]["parallel"]["tasks"][0]["name"],
            "sleep for .1 seconds"
        );
        assert_eq!(
            value["tasks"][1]["parallel"]["tasks"][5]["name"],
            "fast task 3"
        );
    }

    #[test]
    fn subjob_example_preserves_output_and_parallel_subjobs() {
        let value = job_value("subjob.yaml");
        assert_eq!(
            value["tasks"][1]["subjob"]["name"],
            "my sub job with output"
        );
        assert_eq!(
            value["tasks"][1]["subjob"]["output"],
            "{{ tasks.dataStuff }}"
        );
        assert_eq!(
            value["tasks"][1]["subjob"]["tasks"]
                .as_array()
                .map(std::vec::Vec::len),
            Some(1)
        );
        assert_eq!(
            value["tasks"][2]["parallel"]["tasks"]
                .as_array()
                .map(std::vec::Vec::len),
            Some(2)
        );
        assert_eq!(
            value["tasks"][2]["parallel"]["tasks"][0]["name"],
            "sample job 1"
        );
        assert_eq!(
            value["tasks"][2]["parallel"]["tasks"][1]["name"],
            "sample job 2"
        );
    }

    #[test]
    fn bash_subjob_example_preserves_nested_shape() {
        let value = job_value("bash-subjob.yaml");
        assert_eq!(value["output"], "{{ tasks.subjob_output }}");
        assert_eq!(value["tasks"][1]["subjob"]["name"], "build phase");
        assert_eq!(
            value["tasks"][1]["subjob"]["output"],
            "{{ tasks.build_result }}"
        );
        assert_eq!(
            value["tasks"][1]["subjob"]["tasks"]
                .as_array()
                .map(std::vec::Vec::len),
            Some(3)
        );
        assert_eq!(
            value["tasks"][2]["parallel"]["tasks"]
                .as_array()
                .map(std::vec::Vec::len),
            Some(2)
        );
        assert_eq!(
            value["tasks"][2]["parallel"]["tasks"][0]["name"],
            "build frontend"
        );
        assert_eq!(
            value["tasks"][2]["parallel"]["tasks"][1]["name"],
            "build backend"
        );
    }

    #[test]
    fn split_and_stitch_example_preserves_map_heavy_shape() {
        let value = job_value("split_and_stitch.yaml");
        assert_eq!(value["inputs"]["accessKeyID"], "minioadmin");
        assert_eq!(
            value["inputs"]["endpointURL"],
            "http://my-minio-server:9000"
        );
        assert_eq!(value["inputs"]["secretKeyID"], "minioadmin");
        assert_eq!(value["inputs"]["source"], "s3://master/master.mov");
        assert_eq!(
            value["tasks"][4]["files"]["script.py"]
                .as_str()
                .map(|script| script.contains("import re")),
            Some(true)
        );
        assert_eq!(value["tasks"][5]["env"]["DURATION"], "{{ tasks.duration }}");
        assert_eq!(
            value["tasks"][5]["env"]["FRAMERATE"],
            "{{ tasks.framerate }}"
        );
        assert_eq!(
            value["tasks"][8]["each"]["list"],
            "{{ fromJSON(tasks.chunks) }}"
        );
        assert_eq!(
            value["tasks"][8]["each"]["task"]["env"]["LENGTH"],
            "{{ item_value_length }}"
        );
        assert_eq!(
            value["tasks"][8]["each"]["task"]["env"]["SOURCE"],
            "{{ tasks.signedURL }}"
        );
        assert_eq!(
            value["tasks"][8]["each"]["task"]["env"]["START"],
            "{{ item_value_start }}"
        );
        assert_eq!(
            value["tasks"][8]["each"]["task"]["post"]
                .as_array()
                .map(std::vec::Vec::len),
            Some(1)
        );
        assert_eq!(
            value["tasks"][8]["each"]["task"]["mounts"]
                .as_array()
                .map(std::vec::Vec::len),
            Some(1)
        );
        assert_eq!(value["tasks"][8]["each"]["task"]["retry"]["limit"], 2);
        assert_eq!(
            value["tasks"][9]["pre"].as_array().map(std::vec::Vec::len),
            Some(1)
        );
        assert_eq!(
            value["tasks"][9]["post"].as_array().map(std::vec::Vec::len),
            Some(1)
        );
        assert_eq!(
            value["tasks"][9]["mounts"]
                .as_array()
                .map(std::vec::Vec::len),
            Some(1)
        );
        assert_eq!(value["tasks"][9]["retry"]["limit"], 2);
        assert_eq!(value["tasks"][9]["timeout"], "120s");
    }

    #[test]
    fn retry_and_timeout_examples_preserve_expected_limits() {
        let retry_value = job_value("retry.yaml");
        let bash_retry_value = job_value("bash-retry.yaml");
        let timeout_value = job_value("timeout.yaml");
        assert_eq!(retry_value["tasks"][0]["retry"]["limit"], 2);
        assert_eq!(bash_retry_value["tasks"][0]["retry"]["limit"], 5);
        assert_eq!(timeout_value["tasks"][0]["timeout"], "5s");
    }

    #[test]
    fn pokemon_benchmark_example_preserves_parallel_group_sizes() {
        let value = job_value("pokemon-benchmark.yaml");
        assert_eq!(
            value["tasks"][2]["parallel"]["tasks"]
                .as_array()
                .map(std::vec::Vec::len),
            Some(9)
        );
        assert_eq!(
            value["tasks"][2]["parallel"]["tasks"][0]["name"],
            "fetch-bulbasaur"
        );
        assert_eq!(
            value["tasks"][2]["parallel"]["tasks"][8]["name"],
            "fetch-dragonite"
        );
        assert_eq!(
            value["tasks"][3]["parallel"]["tasks"]
                .as_array()
                .map(std::vec::Vec::len),
            Some(6)
        );
        assert_eq!(
            value["tasks"][3]["parallel"]["tasks"][0]["name"],
            "type-fire"
        );
        assert_eq!(
            value["tasks"][3]["parallel"]["tasks"][5]["name"],
            "type-dragon"
        );
    }

    #[test]
    fn twerk_massive_parallel_example_preserves_stress_shape() {
        let value = job_value("twerk-massive-parallel.yaml");
        assert_eq!(value["tasks"].as_array().map(std::vec::Vec::len), Some(2));
        assert_eq!(value["tasks"][0]["name"], "parallel-fanout-200");
        assert_eq!(
            value["tasks"][0]["parallel"]["tasks"]
                .as_array()
                .map(std::vec::Vec::len),
            Some(151)
        );
        assert_eq!(
            value["tasks"][0]["parallel"]["tasks"][0]["name"],
            "fetch-001"
        );
        assert_eq!(
            value["tasks"][0]["parallel"]["tasks"][150]["name"],
            "fetch-151"
        );
    }

    #[test]
    fn stress_examples_preserve_first_and_last_tasks() {
        let noop_value = job_value("twerk-noop-100.yaml");
        let pokemon_value = job_value("twerk-pokemon-shell-100.yaml");
        assert_eq!(
            noop_value["tasks"].as_array().map(std::vec::Vec::len),
            Some(100)
        );
        assert_eq!(noop_value["tasks"][0]["name"], "noop-001");
        assert_eq!(noop_value["tasks"][99]["name"], "noop-100");
        assert_eq!(
            pokemon_value["tasks"].as_array().map(std::vec::Vec::len),
            Some(100)
        );
        assert_eq!(pokemon_value["tasks"][0]["name"], "fetch-001");
        assert_eq!(pokemon_value["tasks"][99]["name"], "fetch-100");
    }

    #[test]
    fn chaos_engineering_example_preserves_parallel_stage() {
        let value = job_value("twerk-chaos-engineering.yaml");
        assert_eq!(value["tasks"].as_array().map(std::vec::Vec::len), Some(13));
        assert_eq!(value["tasks"][2]["name"], "step-03-parallel-fanout");
        assert_eq!(
            value["tasks"][2]["parallel"]["tasks"]
                .as_array()
                .map(std::vec::Vec::len),
            Some(9)
        );
        assert_eq!(
            value["tasks"][2]["parallel"]["tasks"][0]["name"],
            "fetch-bulbasaur"
        );
        assert_eq!(
            value["tasks"][2]["parallel"]["tasks"][8]["name"],
            "fetch-lapras"
        );
    }

    #[test]
    fn asl_hello_example_preserves_two_pass_states() {
        let value = state_machine_value("asl-hello.yaml");
        assert_eq!(value["startAt"], "Hello");
        assert_eq!(value["states"]["Hello"]["type"], "pass");
        assert_eq!(value["states"]["Hello"]["result"], "Hello, World!");
        assert_eq!(value["states"]["Goodbye"]["type"], "pass");
        assert_eq!(value["states"]["Goodbye"]["result"], "Goodbye!");
        assert_eq!(value["states"]["Goodbye"]["end"], true);
    }

    #[test]
    fn asl_retry_example_preserves_retry_contract() {
        let value = state_machine_value("asl-task-retry.yaml");
        assert_eq!(value["startAt"], "Build");
        assert_eq!(value["states"]["Build"]["type"], "task");
        assert_eq!(value["states"]["Build"]["retry"][0]["errorEquals"][0], "taskfailed");
        assert_eq!(value["states"]["Build"]["retry"][0]["intervalSeconds"], 1);
        assert_eq!(value["states"]["Build"]["retry"][0]["maxAttempts"], 3);
        assert_eq!(value["states"]["Build"]["retry"][0]["backoffRate"], 2.0);
        assert_eq!(value["states"]["Deploy"]["end"], true);
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
        assert_eq!(
            result,
            Err(ApiError::BadRequest("YAML parse error".to_string()))
        );
    }

    #[test]
    fn from_slice_accepts_control_chars_0x01_to_0x1f_in_strings() {
        #[derive(Debug, serde::Deserialize, PartialEq, Eq)]
        struct WithControl {
            value: String,
        }
        let yaml = br#"value: "\u0001\u001f""#;
        let result: Result<WithControl, ApiError> = from_slice(yaml);
        assert_eq!(
            result,
            Ok(WithControl {
                value: "\u{1}\u{1f}".to_string()
            })
        );
    }

    #[test]
    fn from_slice_accepts_del_0x7f_in_quoted_strings() {
        #[derive(Debug, serde::Deserialize, PartialEq, Eq)]
        struct WithDel {
            value: String,
        }
        let yaml = br#"value: "hello\u007fworld""#;
        let result: Result<WithDel, ApiError> = from_slice(yaml);
        assert_eq!(
            result,
            Ok(WithDel {
                value: "hello\u{7f}world".to_string()
            })
        );
    }

    #[test]
    fn from_slice_returns_bad_request_for_binary_content_0x80_to_0xff() {
        let invalid_bytes: Vec<u8> = (0x80..=0xFF).collect();
        let result: Result<serde_json::Value, ApiError> = from_slice(&invalid_bytes);
        assert_bad_request_contains(result, "invalid UTF-8");
    }

    #[test]
    fn from_slice_returns_bad_request_for_mixed_valid_and_invalid_utf8() {
        let data: Vec<u8> = b"key: value \xff\xfe".to_vec();
        let result: Result<serde_json::Value, ApiError> = from_slice(&data);
        assert_bad_request_contains(result, "invalid UTF-8");
    }

    #[test]
    fn from_slice_returns_bad_request_for_truncated_utf8_sequence() {
        let data: Vec<u8> = b"key: \xc3".to_vec();
        let result: Result<serde_json::Value, ApiError> = from_slice(&data);
        assert_bad_request_contains(result, "invalid UTF-8");
    }

    #[test]
    fn from_slice_accepts_yaml_with_tabs_in_quoted_strings() {
        #[derive(Debug, serde::Deserialize, PartialEq, Eq)]
        struct WithTab {
            value: String,
        }
        let yaml = b"value: \"hello\\tworld\"";
        let result: Result<WithTab, ApiError> = from_slice(yaml);
        assert_eq!(
            result,
            Ok(WithTab {
                value: "hello\tworld".to_string()
            })
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
        assert_eq!(
            result,
            Ok(UnicodeVal {
                greeting: "🎉 Hello 世界".to_string()
            })
        );
    }

    #[test]
    fn from_slice_handles_printable_ascii_in_values() {
        #[derive(Debug, serde::Deserialize, PartialEq, Eq)]
        struct PrintableAscii {
            value: String,
        }
        let yaml = b"value: 'hello world 123'";
        let result: Result<PrintableAscii, ApiError> = from_slice(yaml);
        assert_eq!(
            result,
            Ok(PrintableAscii {
                value: "hello world 123".to_string()
            })
        );
    }

    #[test]
    fn from_slice_rejects_duplicate_keys_in_same_mapping() {
        let yaml = b"name: duplicate\ninputs:\n  token: first\n  token: second\n";
        let result: Result<twerk_core::job::Job, ApiError> = from_slice(yaml);
        assert_bad_request_contains(result, "duplicate");
    }

    #[test]
    fn from_slice_rejects_yaml_exceeding_depth_budget() {
        let yaml = (0..65).fold(String::new(), |mut acc, depth| {
            let indentation = "  ".repeat(depth);
            acc.push_str(&indentation);
            acc.push_str("level");
            acc.push_str(&depth.to_string());
            acc.push_str(":\n");
            acc
        }) + &("  ".repeat(65) + "leaf: done\n");
        let result: Result<serde_json::Value, ApiError> = from_slice(yaml.as_bytes());
        assert_bad_request_contains(result, "YAML parse error");
    }

    #[test]
    fn from_slice_rejects_yaml_exceeding_node_budget() {
        let yaml = (0..10_050).fold(String::from("items:\n"), |mut acc, index| {
            acc.push_str("  - item-");
            acc.push_str(&index.to_string());
            acc.push('\n');
            acc
        });
        let result: Result<serde_json::Value, ApiError> = from_slice(yaml.as_bytes());
        assert_bad_request_contains(result, "YAML parse error");
    }

    #[test]
    fn to_string_roundtrips_real_job_example() -> Result<(), ApiError> {
        let job = parse_job_example("each.yaml");
        let yaml = to_string(&job)?;
        let roundtrip: twerk_core::job::Job = from_slice(yaml.as_bytes())?;
        assert_eq!(roundtrip, job);
        Ok(())
    }

    #[test]
    fn to_string_returns_internal_when_serializer_fails() {
        struct FailingSerialize;

        impl serde::Serialize for FailingSerialize {
            fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                Err(serde::ser::Error::custom("boom"))
            }
        }

        let result = to_string(&FailingSerialize);
        assert_internal_contains(result, "YAML serialization error: boom");
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
            prop_assert_eq!(result1.map(|value| value.to_string()), result2.map(|value| value.to_string()));
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
        assert_eq!(
            result.map(|doc| doc.key.or(doc.name).or(doc.value)),
            Ok(Some(expected_val.to_string()))
        );
    }
}
