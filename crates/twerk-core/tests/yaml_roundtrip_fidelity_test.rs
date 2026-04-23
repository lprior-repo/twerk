//! YAML round-trip fidelity tests for Job serialization.
//!
//! This meta-test loads EVERY .yaml file in examples/, parses it into twerk Job
//! structs, serializes back to YAML and JSON, re-parses both, and verifies the
//! round-trip produces equivalent structures. This catches serde attribute issues
//! (rename_all, skip_serializing_if, etc).

use std::path::PathBuf;

fn get_examples_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("could not resolve workspace root")
        .join("examples")
}

fn get_all_yaml_files() -> Vec<PathBuf> {
    let examples_dir = get_examples_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&examples_dir)
        .expect("could not read examples directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .map(|ext| ext == "yaml" || ext == "yml")
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    files
}

fn parse_yaml_file<C: serde::de::DeserializeOwned>(path: &PathBuf) -> Result<C, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    serde_saphyr::from_str(&content)
        .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
}

fn serialize_to_json<T: serde::Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| format!("JSON serialization failed: {}", e))
}

fn serialize_to_yaml<T: serde::Serialize>(value: &T) -> Result<String, String> {
    serde_saphyr::to_string(value).map_err(|e| format!("YAML serialization failed: {}", e))
}

fn deserialize_json<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, String> {
    serde_json::from_str(json).map_err(|e| format!("JSON deserialization failed: {}", e))
}

fn deserialize_yaml<T: serde::de::DeserializeOwned>(yaml: &str) -> Result<T, String> {
    serde_saphyr::from_str(yaml).map_err(|e| format!("YAML deserialization failed: {}", e))
}

fn compare_job_yaml_roundtrip(original: &twerk_core::job::Job, reloaded: &twerk_core::job::Job) {
    assert_eq!(
        original.name, reloaded.name,
        "name mismatch after round-trip"
    );
    assert_eq!(
        original.description, reloaded.description,
        "description mismatch after round-trip"
    );
    assert_eq!(
        original.state, reloaded.state,
        "state mismatch after round-trip"
    );
    assert_eq!(
        original.task_count, reloaded.task_count,
        "task_count mismatch after round-trip"
    );
    assert!(
        (original.progress - reloaded.progress).abs() < 1e-10,
        "progress mismatch after round-trip"
    );
    assert_eq!(
        original.position, reloaded.position,
        "position mismatch after round-trip"
    );

    match (&original.tasks, &reloaded.tasks) {
        (Some(original_tasks), Some(reloaded_tasks)) => {
            assert_eq!(
                original_tasks.len(),
                reloaded_tasks.len(),
                "tasks length mismatch"
            );
            for (orig, rel) in original_tasks.iter().zip(reloaded_tasks.iter()) {
                assert_eq!(orig.name, rel.name, "task name mismatch");
                assert_eq!(orig.image, rel.image, "task image mismatch");
                assert_eq!(orig.var, rel.var, "task var mismatch");
                assert_eq!(orig.position, rel.position, "task position mismatch");
                assert_eq!(orig.priority, rel.priority, "task priority mismatch");
            }
        }
        (None, None) => {}
        (a, b) => panic!("tasks mismatch: {:?} vs {:?}", a.is_some(), b.is_some()),
    }

    match (&original.inputs, &reloaded.inputs) {
        (Some(orig), Some(rel)) => {
            assert_eq!(orig, rel, "inputs mismatch after round-trip");
        }
        (None, None) => {}
        (a, b) => panic!("inputs mismatch: {:?} vs {:?}", a.is_some(), b.is_some()),
    }

    assert_eq!(
        original.tags, reloaded.tags,
        "tags mismatch after round-trip"
    );
    assert_eq!(
        original.webhooks, reloaded.webhooks,
        "webhooks mismatch after round-trip"
    );
    assert_eq!(
        original.schedule, reloaded.schedule,
        "schedule mismatch after round-trip"
    );
    assert_eq!(
        original.defaults, reloaded.defaults,
        "defaults mismatch after round-trip"
    );
}

#[test]
fn yaml_files_are_valid_utf8() {
    let files = get_all_yaml_files();
    assert!(!files.is_empty(), "No YAML files found in examples/");
    for file in &files {
        let content =
            std::fs::read_to_string(file).expect(&format!("Failed to read {}", file.display()));
        assert!(
            content.is_ascii()
                || content
                    .chars()
                    .all(|c| c.is_ascii() || c.is_whitespace() || !c.is_control()),
            "File {} contains non-UTF8 characters",
            file.display()
        );
    }
}

#[test]
fn all_example_yaml_files_parse_to_job() {
    let files = get_all_yaml_files();
    let mut failures = Vec::new();

    for file in &files {
        let result: Result<twerk_core::job::Job, String> = parse_yaml_file(file);
        if let Err(e) = result {
            failures.push(format!("{}: {}", file.display(), e));
        }
    }

    if !failures.is_empty() {
        panic!(
            "Failed to parse {} files:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

#[test]
fn job_yaml_roundtrip_preserves_all_example_files() {
    let files = get_all_yaml_files();
    let mut failures = Vec::new();

    for file in &files {
        let result: Result<twerk_core::job::Job, String> = parse_yaml_file(file);
        match result {
            Ok(original_job) => {
                let yaml_out = serialize_to_yaml(&original_job);
                match yaml_out {
                    Ok(yaml_string) => {
                        let reloaded: Result<twerk_core::job::Job, String> =
                            deserialize_yaml(&yaml_string);
                        match reloaded {
                            Ok(reloaded_job) => {
                                compare_job_yaml_roundtrip(&original_job, &reloaded_job);
                            }
                            Err(e) => {
                                failures.push(format!(
                                    "{}: YAML round-trip deserialize failed: {}",
                                    file.display(),
                                    e
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        failures.push(format!(
                            "{}: YAML serialization failed: {}",
                            file.display(),
                            e
                        ));
                    }
                }
            }
            Err(e) => {
                failures.push(format!("{}: Initial parse failed: {}", file.display(), e));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "YAML round-trip failures ({}):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

#[test]
fn job_json_roundtrip_preserves_all_example_files() {
    let files = get_all_yaml_files();
    let mut failures = Vec::new();

    for file in &files {
        let result: Result<twerk_core::job::Job, String> = parse_yaml_file(file);
        match result {
            Ok(original_job) => {
                let json_out = serialize_to_json(&original_job);
                match json_out {
                    Ok(json_string) => {
                        let reloaded: Result<twerk_core::job::Job, String> =
                            deserialize_json(&json_string);
                        match reloaded {
                            Ok(reloaded_job) => {
                                compare_job_yaml_roundtrip(&original_job, &reloaded_job);
                            }
                            Err(e) => {
                                failures.push(format!(
                                    "{}: JSON round-trip deserialize failed: {}",
                                    file.display(),
                                    e
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        failures.push(format!(
                            "{}: JSON serialization failed: {}",
                            file.display(),
                            e
                        ));
                    }
                }
            }
            Err(e) => {
                failures.push(format!(
                    "{}: Initial parse failed (skipping): {}",
                    file.display(),
                    e
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "JSON round-trip failures ({}):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

#[test]
fn yaml_to_json_to_yaml_preserves_structure() {
    let files = get_all_yaml_files();

    for file in &files {
        let result: Result<twerk_core::job::Job, String> = parse_yaml_file(file);
        if result.is_err() {
            continue;
        }
        let original_job = result.unwrap();

        let yaml_string = serialize_to_yaml(&original_job).expect("YAML serialization should work");
        let json_string = serialize_to_json(&original_job).expect("JSON serialization should work");

        let from_yaml: twerk_core::job::Job =
            deserialize_yaml(&yaml_string).expect("Should deserialize from YAML");
        let from_json: twerk_core::job::Job =
            deserialize_json(&json_string).expect("Should deserialize from JSON");

        compare_job_yaml_roundtrip(&original_job, &from_yaml);
        compare_job_yaml_roundtrip(&original_job, &from_json);
        compare_job_yaml_roundtrip(&from_yaml, &from_json);
    }
}

#[test]
fn job_serialization_does_not_lose_optional_fields() {
    let files = get_all_yaml_files();

    for file in &files {
        let result: Result<twerk_core::job::Job, String> = parse_yaml_file(file);
        if result.is_err() {
            continue;
        }
        let job = result.unwrap();

        let yaml_string = serialize_to_yaml(&job).expect("YAML serialization should work");
        let reloaded: twerk_core::job::Job =
            deserialize_yaml(&yaml_string).expect("Should deserialize from YAML");

        fn count_some_fields(job: &twerk_core::job::Job) -> usize {
            let mut count = 0;
            if job.id.is_some() {
                count += 1;
            }
            if job.parent_id.is_some() {
                count += 1;
            }
            if job.name.is_some() {
                count += 1;
            }
            if job.description.is_some() {
                count += 1;
            }
            if job.tags.is_some() {
                count += 1;
            }
            if job.tasks.is_some() {
                count += 1;
            }
            if job.execution.is_some() {
                count += 1;
            }
            if job.inputs.is_some() {
                count += 1;
            }
            if job.context.is_some() {
                count += 1;
            }
            if job.output.is_some() {
                count += 1;
            }
            if job.result.is_some() {
                count += 1;
            }
            if job.error.is_some() {
                count += 1;
            }
            if job.defaults.is_some() {
                count += 1;
            }
            if job.webhooks.is_some() {
                count += 1;
            }
            if job.permissions.is_some() {
                count += 1;
            }
            if job.auto_delete.is_some() {
                count += 1;
            }
            if job.delete_at.is_some() {
                count += 1;
            }
            if job.secrets.is_some() {
                count += 1;
            }
            if job.schedule.is_some() {
                count += 1;
            }
            if job.created_at.is_some() {
                count += 1;
            }
            if job.created_by.is_some() {
                count += 1;
            }
            if job.started_at.is_some() {
                count += 1;
            }
            if job.completed_at.is_some() {
                count += 1;
            }
            if job.failed_at.is_some() {
                count += 1;
            }
            count
        }

        let orig_count = count_some_fields(&job);
        let reload_count = count_some_fields(&reloaded);

        assert_eq!(
            orig_count,
            reload_count,
            "Optional field count mismatch for {}: original={}, reloaded={}",
            file.display(),
            orig_count,
            reload_count
        );
    }
}

#[test]
fn serde_skip_serializing_if_preserves_none_fields() {
    use twerk_core::job::Job;

    let job = Job {
        id: None,
        parent_id: None,
        name: Some("test".to_string()),
        description: None,
        tags: None,
        state: twerk_core::job::JobState::Pending,
        created_at: None,
        created_by: None,
        started_at: None,
        completed_at: None,
        failed_at: None,
        tasks: None,
        execution: None,
        position: 0,
        inputs: None,
        context: None,
        task_count: 0,
        output: None,
        result: None,
        error: None,
        defaults: None,
        webhooks: None,
        permissions: None,
        auto_delete: None,
        delete_at: None,
        secrets: None,
        progress: 0.0,
        schedule: None,
    };

    let yaml = serde_saphyr::to_string(&job).expect("Should serialize");
    let reloaded: Job = serde_saphyr::from_str(&yaml).expect("Should deserialize");

    assert_eq!(job.name, reloaded.name);
    assert_eq!(job.state, reloaded.state);
    assert!(reloaded.id.is_none());
    assert!(reloaded.description.is_none());
    assert!(reloaded.tasks.is_none());
}

#[test]
fn rename_all_camel_case_works_in_yaml() {
    use twerk_core::job::Job;

    let yaml = r#"
name: camelCase-test
taskCount: 5
progress: 0.75
autoDelete:
  after: after_success
"#;

    let job: Job = serde_saphyr::from_str(yaml).expect("Should parse camelCase YAML");
    assert_eq!(job.name, Some("camelCase-test".to_string()));
    assert_eq!(job.task_count, 5);
    assert!((job.progress - 0.75).abs() < 1e-10);
    assert!(job.auto_delete.is_some());
}

#[test]
fn rename_all_screaming_snake_case_works_in_yaml() {
    use twerk_core::job::{Job, JobState};

    let yaml = r#"
name: state-test
state: PENDING
"#;

    let job: Job = serde_saphyr::from_str(yaml).expect("Should parse SCREAMING_SNAKE_CASE state");
    assert_eq!(job.state, JobState::Pending);
}

#[test]
fn complex_nested_job_roundtrips_correctly() {
    let files = get_all_yaml_files();

    for file in &files {
        let result: Result<twerk_core::job::Job, String> = parse_yaml_file(file);
        if result.is_err() {
            continue;
        }
        let job = result.unwrap();

        let yaml1 = serialize_to_yaml(&job).expect("First YAML serialize");
        let json1 = serialize_to_json(&job).expect("First JSON serialize");

        let job2: twerk_core::job::Job = deserialize_yaml(&yaml1).expect("YAML->Job");
        let yaml2 = serialize_to_yaml(&job2).expect("Second YAML serialize");

        let job3: twerk_core::job::Job = deserialize_json(&json1).expect("JSON->Job");
        let json2 = serialize_to_json(&job3).expect("Second JSON serialize");

        let job4: twerk_core::job::Job = deserialize_yaml(&yaml2).expect("YAML2->Job");
        let job5: twerk_core::job::Job = deserialize_json(&json2).expect("JSON2->Job");

        compare_job_yaml_roundtrip(&job, &job4);
        compare_job_yaml_roundtrip(&job, &job5);
    }
}

#[test]
fn job_with_tasks_roundtrips_correctly() {
    use twerk_core::job::Job;

    let yaml = r#"
name: multi-task-job
taskCount: 3
tasks:
  - name: step1
    image: ubuntu:mantic
    var: VAR1
    run: echo step1
    position: 0
    priority: 0
  - name: step2
    image: alpine:latest
    var: VAR2
    run: echo step2
    position: 1
    priority: 1
  - name: step3
    image: debian:bookworm
    var: VAR3
    run: echo step3
    position: 2
    priority: 2
"#;

    let job: Job = serde_saphyr::from_str(yaml).expect("Should parse job with tasks");
    assert_eq!(job.tasks.as_ref().map(|t| t.len()), Some(3));

    let yaml_out = serialize_to_yaml(&job).expect("Should serialize to YAML");
    let reloaded: Job = deserialize_yaml(&yaml_out).expect("Should deserialize from YAML");

    compare_job_yaml_roundtrip(&job, &reloaded);
}

#[test]
fn job_with_context_roundtrips_correctly() {
    use twerk_core::job::Job;

    let yaml = r#"
name: context-job
context:
  job:
    execution_id: exec-123
  inputs:
    ENV: production
  secrets:
    API_KEY: secret123
  tasks:
    task1: completed
"#;

    let job: Job = serde_saphyr::from_str(yaml).expect("Should parse job with context");
    assert!(job.context.is_some());

    let yaml_out = serialize_to_yaml(&job).expect("Should serialize to YAML");
    let reloaded: Job = deserialize_yaml(&yaml_out).expect("Should deserialize from YAML");

    assert_eq!(job.context, reloaded.context);
}

#[test]
fn job_with_webhooks_roundtrips_correctly() {
    use twerk_core::job::Job;

    let yaml = r#"
name: webhook-job
webhooks:
  - url: https://example.com/success
    method: POST
  - url: https://example.com/failure
    method: PUT
"#;

    let job: Job = serde_saphyr::from_str(yaml).expect("Should parse job with webhooks");
    assert!(job.webhooks.is_some());

    let yaml_out = serialize_to_yaml(&job).expect("Should serialize to YAML");
    let reloaded: Job = deserialize_yaml(&yaml_out).expect("Should deserialize from YAML");

    assert_eq!(job.webhooks, reloaded.webhooks);
}

#[test]
fn job_with_schedule_roundtrips_correctly() {
    use twerk_core::job::Job;

    let yaml = r#"
name: scheduled-job
schedule:
  cron: "0 0 * * *"
"#;

    let job: Job = serde_saphyr::from_str(yaml).expect("Should parse job with schedule");
    assert!(job.schedule.is_some());

    let yaml_out = serialize_to_yaml(&job).expect("Should serialize to YAML");
    let reloaded: Job = deserialize_yaml(&yaml_out).expect("Should deserialize from YAML");

    assert_eq!(job.schedule, reloaded.schedule);
}

#[test]
fn job_with_secrets_roundtrips_correctly() {
    use twerk_core::job::Job;

    let yaml = r#"
name: secret-job
secrets:
  DATABASE_URL: postgres://localhost/db
  API_KEY: sk-1234567890abcdef
"#;

    let job: Job = serde_saphyr::from_str(yaml).expect("Should parse job with secrets");
    assert!(job.secrets.is_some());

    let yaml_out = serialize_to_yaml(&job).expect("Should serialize to YAML");
    let reloaded: Job = deserialize_yaml(&yaml_out).expect("Should deserialize from YAML");

    assert_eq!(job.secrets, reloaded.secrets);
}

#[test]
fn job_with_tags_roundtrips_correctly() {
    use twerk_core::job::Job;

    let yaml = r#"
name: tagged-job
tags:
  - frontend
  - api
  - v2
  - production
"#;

    let job: Job = serde_saphyr::from_str(yaml).expect("Should parse job with tags");
    assert_eq!(job.tags.as_ref().map(|t| t.len()), Some(4));

    let yaml_out = serialize_to_yaml(&job).expect("Should serialize to YAML");
    let reloaded: Job = deserialize_yaml(&yaml_out).expect("Should deserialize from YAML");

    assert_eq!(job.tags, reloaded.tags);
}

#[test]
fn job_with_inputs_roundtrips_correctly() {
    use twerk_core::job::Job;

    let yaml = r#"
name: input-job
inputs:
  ENV: production
  VERSION: "1.0.0"
  DEBUG: "false"
"#;

    let job: Job = serde_saphyr::from_str(yaml).expect("Should parse job with inputs");
    assert!(job.inputs.is_some());

    let yaml_out = serialize_to_yaml(&job).expect("Should serialize to YAML");
    let reloaded: Job = deserialize_yaml(&yaml_out).expect("Should deserialize from YAML");

    assert_eq!(job.inputs, reloaded.inputs);
}
