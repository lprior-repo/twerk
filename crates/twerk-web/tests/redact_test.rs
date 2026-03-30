use std::collections::HashMap;
use twerk_core::job::{Job, JobContext, JobSummary};
use twerk_core::mount::Mount;
use twerk_core::task::{Registry, SubJobTask, Task};
use twerk_web::api::redact::{redact_job, redact_job_summary, redact_task};

fn redact_appended_when_value_contains_secret_value() {
    let mut job = Job {
        inputs: Some(
            [(
                "log_message".to_string(),
                "Using token abc123 and password secret123".to_string(),
            )]
            .into(),
        ),
        secrets: Some([("my_token".to_string(), "abc123".to_string())].into()),
        ..Default::default()
    };
    redact_job(&mut job);
    let inputs = job.inputs.unwrap();
    assert_eq!(
        inputs["log_message"],
        "Using token [REDACTED] and password secret123"
    );
}

fn redact_appended_when_multiple_secret_values_present() {
    let mut job = Job {
        inputs: Some(
            [(
                "config".to_string(),
                "api_key=sk-12345 secret=my-secret".to_string(),
            )]
            .into(),
        ),
        secrets: Some(
            [
                ("key1".to_string(), "sk-12345".to_string()),
                ("key2".to_string(), "my-secret".to_string()),
            ]
            .into(),
        ),
        ..Default::default()
    };
    redact_job(&mut job);
    let inputs = job.inputs.unwrap();
    assert_eq!(inputs["config"], "api_key=[REDACTED] secret=[REDACTED]");
}

fn redact_replaces_secret_value_when_key_is_password() {
    let mut job = Job {
        inputs: Some([("DB_PASSWORD".to_string(), "super_secret".to_string())].into()),
        ..Default::default()
    };
    redact_job(&mut job);
    let inputs = job.inputs.unwrap();
    assert_eq!(inputs["DB_PASSWORD"], "[REDACTED]");
}

fn redact_replaces_secret_value_when_key_is_access_key() {
    let mut job = Job {
        inputs: Some(
            [(
                "AWS_ACCESS_KEY_ID".to_string(),
                "AKIAIOSFODNN7EXAMPLE".to_string(),
            )]
            .into(),
        ),
        ..Default::default()
    };
    redact_job(&mut job);
    let inputs = job.inputs.unwrap();
    assert_eq!(inputs["AWS_ACCESS_KEY_ID"], "[REDACTED]");
}

fn redact_replaces_secret_value_when_key_contains_secret() {
    let mut job = Job {
        inputs: Some([("MY_SECRET_TOKEN".to_string(), "tok_abcdefghij".to_string())].into()),
        ..Default::default()
    };
    redact_job(&mut job);
    let inputs = job.inputs.unwrap();
    assert_eq!(inputs["MY_SECRET_TOKEN"], "[REDACTED]");
}

fn redact_job_secrets_map_replaced_with_redacted() {
    let mut job = Job {
        secrets: Some(
            [
                ("api_key".to_string(), "sk-live-12345".to_string()),
                ("db_pass".to_string(), "postgres_secret".to_string()),
            ]
            .into(),
        ),
        ..Default::default()
    };
    redact_job(&mut job);
    let secrets = job.secrets.unwrap();
    assert_eq!(secrets["api_key"], "[REDACTED]");
    assert_eq!(secrets["db_pass"], "[REDACTED]");
}

fn redact_job_webhook_headers_containing_secrets() {
    let mut job = Job {
        webhooks: Some(vec![twerk_core::webhook::Webhook {
            url: Some("https://example.com/hook".to_string()),
            headers: Some(
                [(
                    "Authorization".to_string(),
                    "Bearer super_secret_token".to_string(),
                )]
                .into(),
            ),
            ..Default::default()
        }]),
        secrets: Some([("auth_token".to_string(), "super_secret_token".to_string())].into()),
        ..Default::default()
    };
    redact_job(&mut job);
    let webhooks = job.webhooks.as_ref().unwrap();
    let headers = webhooks[0].headers.as_ref().unwrap();
    assert_eq!(headers["Authorization"], "Bearer [REDACTED]");
}

fn redact_job_context_inputs_when_containing_secrets() {
    let mut job = Job {
        context: Some(JobContext {
            inputs: Some([("context_token".to_string(), "ctx_secret_val".to_string())].into()),
            ..Default::default()
        }),
        secrets: Some([("ctx_key".to_string(), "ctx_secret_val".to_string())].into()),
        ..Default::default()
    };
    redact_job(&mut job);
    let context_inputs = job.context.as_ref().unwrap().inputs.as_ref().unwrap();
    assert_eq!(context_inputs["context_token"], "[REDACTED]");
}

fn redact_job_context_secrets_replaced_with_redacted() {
    let mut job = Job {
        context: Some(JobContext {
            secrets: Some([("ctx_secret".to_string(), "hidden_value".to_string())].into()),
            ..Default::default()
        }),
        secrets: Some([("ctx_secret".to_string(), "hidden_value".to_string())].into()),
        ..Default::default()
    };
    redact_job(&mut job);
    let ctx_secrets = job.context.as_ref().unwrap().secrets.as_ref().unwrap();
    assert_eq!(ctx_secrets["ctx_secret"], "[REDACTED]");
}

fn redact_job_context_tasks_when_containing_secrets() {
    let mut job = Job {
        context: Some(JobContext {
            tasks: Some([("task_token".to_string(), "task_secret_val".to_string())].into()),
            ..Default::default()
        }),
        secrets: Some([("task_key".to_string(), "task_secret_val".to_string())].into()),
        ..Default::default()
    };
    redact_job(&mut job);
    let ctx_tasks = job.context.as_ref().unwrap().tasks.as_ref().unwrap();
    assert_eq!(ctx_tasks["task_token"], "[REDACTED]");
}

fn redact_job_tasks_env_vars_containing_secrets() {
    let mut task = Task {
        env: Some([("DATABASE_PASSWORD".to_string(), "db_pass_123".to_string())].into()),
        ..Default::default()
    };
    let secrets: HashMap<String, String> =
        [("db_key".to_string(), "db_pass_123".to_string())].into();
    redact_task(&mut task, &secrets);
    assert_eq!(
        task.env.as_ref().unwrap()["DATABASE_PASSWORD"],
        "[REDACTED]"
    );
}

fn redact_job_tasks_mount_opts_containing_secrets() {
    let mut task = Task {
        mounts: Some(vec![Mount {
            target: Some("/tmp/secrets".to_string()),
            opts: Some(
                [(
                    "volume_secret".to_string(),
                    "secret_volume_data".to_string(),
                )]
                .into(),
            ),
            ..Default::default()
        }]),
        ..Default::default()
    };
    let secrets: HashMap<String, String> =
        [("vol_key".to_string(), "secret_volume_data".to_string())].into();
    redact_task(&mut task, &secrets);
    let opts = task.mounts.as_ref().unwrap()[0].opts.as_ref().unwrap();
    assert_eq!(opts["volume_secret"], "[REDACTED]");
}

fn redact_job_tasks_registry_password_replaced() {
    let mut task = Task {
        registry: Some(Registry {
            username: Some("admin".to_string()),
            password: Some("registry_secret".to_string()),
        }),
        ..Default::default()
    };
    let secrets: HashMap<String, String> = HashMap::new();
    redact_task(&mut task, &secrets);
    assert_eq!(
        task.registry.as_ref().unwrap().password.as_ref().unwrap(),
        "[REDACTED]"
    );
}

fn redact_job_tasks_pre_sidecars_recursively_redacted() {
    let mut task = Task {
        pre: Some(vec![Task {
            env: Some([("PRE_SECRET".to_string(), "pre_value".to_string())].into()),
            ..Default::default()
        }]),
        sidecars: Some(vec![Task {
            env: Some([("SIDECAR_API_KEY".to_string(), "sidecar_key".to_string())].into()),
            ..Default::default()
        }]),
        ..Default::default()
    };
    let secrets: HashMap<String, String> = HashMap::new();
    redact_task(&mut task, &secrets);
    assert_eq!(
        task.pre.as_ref().unwrap()[0].env.as_ref().unwrap()["PRE_SECRET"],
        "[REDACTED]"
    );
    assert_eq!(
        task.sidecars.as_ref().unwrap()[0].env.as_ref().unwrap()["SIDECAR_API_KEY"],
        "[REDACTED]"
    );
}

fn redact_job_tasks_parallel_tasks_recursively_redacted() {
    let mut task = Task {
        parallel: Some(twerk_core::task::ParallelTask {
            tasks: Some(vec![Task {
                env: Some(
                    [(
                        "PARALLEL_SECRET_TOKEN".to_string(),
                        "par_tok_12345".to_string(),
                    )]
                    .into(),
                ),
                ..Default::default()
            }]),
            completions: 1,
        }),
        ..Default::default()
    };
    let secrets: HashMap<String, String> = HashMap::new();
    redact_task(&mut task, &secrets);
    let parallel_secret = task.parallel.as_ref().unwrap().tasks.as_ref().unwrap()[0]
        .env
        .as_ref()
        .unwrap()["PARALLEL_SECRET_TOKEN"]
        .clone();
    assert_eq!(parallel_secret, "[REDACTED]");
}

fn redact_job_tasks_subjob_secrets_replaced() {
    let mut task = Task {
        subjob: Some(SubJobTask {
            secrets: Some([("subjob_api_key".to_string(), "sj_key_123".to_string())].into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let secrets: HashMap<String, String> = HashMap::new();
    redact_task(&mut task, &secrets);
    let sj_secrets = task.subjob.as_ref().unwrap().secrets.as_ref().unwrap();
    assert_eq!(sj_secrets["subjob_api_key"], "[REDACTED]");
}

fn redact_job_tasks_subjob_webhooks_redacted() {
    let mut task = Task {
        subjob: Some(SubJobTask {
            webhooks: Some(vec![twerk_core::webhook::Webhook {
                url: Some("https://example.com/webhook".to_string()),
                headers: Some([("X-API-Key".to_string(), "webhook_key_secret".to_string())].into()),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let secrets: HashMap<String, String> =
        [("wh_key".to_string(), "webhook_key_secret".to_string())].into();
    redact_task(&mut task, &secrets);
    let wh_headers = task.subjob.as_ref().unwrap().webhooks.as_ref().unwrap()[0]
        .headers
        .as_ref()
        .unwrap();
    assert_eq!(wh_headers["X-API-Key"], "[REDACTED]");
}

fn redact_job_execution_tasks_redacted() {
    let mut job = Job {
        execution: Some(vec![Task {
            env: Some([("EXECUTION_SECRET".to_string(), "exec_val".to_string())].into()),
            ..Default::default()
        }]),
        ..Default::default()
    };
    redact_job(&mut job);
    let exec_secret =
        job.execution.as_ref().unwrap()[0].env.as_ref().unwrap()["EXECUTION_SECRET"].clone();
    assert_eq!(exec_secret, "[REDACTED]");
}

fn redact_job_summary_inputs_by_key_only() {
    let mut summary = JobSummary {
        inputs: Some([("MY_PASSWORD".to_string(), "secret_value".to_string())].into()),
        ..Default::default()
    };
    redact_job_summary(&mut summary);
    let inputs = summary.inputs.unwrap();
    assert_eq!(inputs["MY_PASSWORD"], "[REDACTED]");
}

fn redact_job_summary_does_not_redact_non_secret_keys() {
    let mut summary = JobSummary {
        inputs: Some([("job_name".to_string(), "my-job".to_string())].into()),
        ..Default::default()
    };
    redact_job_summary(&mut summary);
    let inputs = summary.inputs.unwrap();
    assert_eq!(inputs["job_name"], "my-job");
}

fn redact_no_op_when_no_secrets_defined() {
    let mut job = Job {
        inputs: Some([("normal_key".to_string(), "normal_value".to_string())].into()),
        ..Default::default()
    };
    redact_job(&mut job);
    let inputs = job.inputs.unwrap();
    assert_eq!(inputs["normal_key"], "normal_value");
}

fn redact_no_op_when_secrets_map_is_empty() {
    let mut job = Job {
        inputs: Some([("api_key".to_string(), "should_stay".to_string())].into()),
        secrets: Some(HashMap::new()),
        ..Default::default()
    };
    redact_job(&mut job);
    let inputs = job.inputs.unwrap();
    assert_eq!(inputs["api_key"], "[REDACTED]");
}

fn redact_handles_empty_secret_values_without_panic() {
    let mut job = Job {
        inputs: Some([("key".to_string(), "value".to_string())].into()),
        secrets: Some([("empty_secret".to_string(), "".to_string())].into()),
        ..Default::default()
    };
    redact_job(&mut job);
    let inputs = job.inputs.unwrap();
    assert_eq!(inputs["key"], "value");
}

fn redact_patterns_api_key_when_key_contains_token() {
    let mut job = Job {
        inputs: Some(
            [(
                "GITHUB_TOKEN".to_string(),
                "ghp_xxxxxxxxxxxxxxxxxxxx".to_string(),
            )]
            .into(),
        ),
        ..Default::default()
    };
    redact_job(&mut job);
    let inputs = job.inputs.unwrap();
    assert_eq!(inputs["GITHUB_TOKEN"], "[REDACTED]");
}

fn redact_patterns_aws_secret_key_when_key_contains_secret() {
    let mut job = Job {
        inputs: Some(
            [(
                "AWS_SECRET_ACCESS_KEY".to_string(),
                "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            )]
            .into(),
        ),
        ..Default::default()
    };
    redact_job(&mut job);
    let inputs = job.inputs.unwrap();
    assert_eq!(inputs["AWS_SECRET_ACCESS_KEY"], "[REDACTED]");
}

fn redact_matches_secret_value_in_nested_json_string() {
    let mut job = Job {
        inputs: Some(
            [(
                "config_json".to_string(),
                r#"{"password": "nested_secret"}"#.to_string(),
            )]
            .into(),
        ),
        secrets: Some([("nested_key".to_string(), "nested_secret".to_string())].into()),
        ..Default::default()
    };
    redact_job(&mut job);
    let inputs = job.inputs.unwrap();
    assert_eq!(inputs["config_json"], r#"{"password": "[REDACTED]"}"#);
}

fn redact_matches_multiple_occurrences_of_same_secret() {
    let mut job = Job {
        inputs: Some(
            [(
                "log".to_string(),
                "token=abc123 used token=abc123 again".to_string(),
            )]
            .into(),
        ),
        secrets: Some([("token".to_string(), "abc123".to_string())].into()),
        ..Default::default()
    };
    redact_job(&mut job);
    let inputs = job.inputs.unwrap();
    assert_eq!(
        inputs["log"],
        "token=[REDACTED] used token=[REDACTED] again"
    );
}

#[cfg(test)]
mod tests {

    #[test]
    fn redact_appended_when_value_contains_secret_value() {
        super::redact_appended_when_value_contains_secret_value()
    }

    #[test]
    fn redact_appended_when_multiple_secret_values_present() {
        super::redact_appended_when_multiple_secret_values_present()
    }

    #[test]
    fn redact_replaces_secret_value_when_key_is_password() {
        super::redact_replaces_secret_value_when_key_is_password()
    }

    #[test]
    fn redact_replaces_secret_value_when_key_is_access_key() {
        super::redact_replaces_secret_value_when_key_is_access_key()
    }

    #[test]
    fn redact_replaces_secret_value_when_key_contains_secret() {
        super::redact_replaces_secret_value_when_key_contains_secret()
    }

    #[test]
    fn redact_job_secrets_map_replaced_with_redacted() {
        super::redact_job_secrets_map_replaced_with_redacted()
    }

    #[test]
    fn redact_job_webhook_headers_containing_secrets() {
        super::redact_job_webhook_headers_containing_secrets()
    }

    #[test]
    fn redact_job_context_inputs_when_containing_secrets() {
        super::redact_job_context_inputs_when_containing_secrets()
    }

    #[test]
    fn redact_job_context_secrets_replaced_with_redacted() {
        super::redact_job_context_secrets_replaced_with_redacted()
    }

    #[test]
    fn redact_job_context_tasks_when_containing_secrets() {
        super::redact_job_context_tasks_when_containing_secrets()
    }

    #[test]
    fn redact_job_tasks_env_vars_containing_secrets() {
        super::redact_job_tasks_env_vars_containing_secrets()
    }

    #[test]
    fn redact_job_tasks_mount_opts_containing_secrets() {
        super::redact_job_tasks_mount_opts_containing_secrets()
    }

    #[test]
    fn redact_job_tasks_registry_password_replaced() {
        super::redact_job_tasks_registry_password_replaced()
    }

    #[test]
    fn redact_job_tasks_pre_sidecars_recursively_redacted() {
        super::redact_job_tasks_pre_sidecars_recursively_redacted()
    }

    #[test]
    fn redact_job_tasks_parallel_tasks_recursively_redacted() {
        super::redact_job_tasks_parallel_tasks_recursively_redacted()
    }

    #[test]
    fn redact_job_tasks_subjob_secrets_replaced() {
        super::redact_job_tasks_subjob_secrets_replaced()
    }

    #[test]
    fn redact_job_tasks_subjob_webhooks_redacted() {
        super::redact_job_tasks_subjob_webhooks_redacted()
    }

    #[test]
    fn redact_job_execution_tasks_redacted() {
        super::redact_job_execution_tasks_redacted()
    }

    #[test]
    fn redact_job_summary_inputs_by_key_only() {
        super::redact_job_summary_inputs_by_key_only()
    }

    #[test]
    fn redact_job_summary_does_not_redact_non_secret_keys() {
        super::redact_job_summary_does_not_redact_non_secret_keys()
    }

    #[test]
    fn redact_no_op_when_no_secrets_defined() {
        super::redact_no_op_when_no_secrets_defined()
    }

    #[test]
    fn redact_no_op_when_secrets_map_is_empty() {
        super::redact_no_op_when_secrets_map_is_empty()
    }

    #[test]
    fn redact_handles_empty_secret_values_without_panic() {
        super::redact_handles_empty_secret_values_without_panic()
    }

    #[test]
    fn redact_patterns_api_key_when_key_contains_token() {
        super::redact_patterns_api_key_when_key_contains_token()
    }

    #[test]
    fn redact_patterns_aws_secret_key_when_key_contains_secret() {
        super::redact_patterns_aws_secret_key_when_key_contains_secret()
    }

    #[test]
    fn redact_matches_secret_value_in_nested_json_string() {
        super::redact_matches_secret_value_in_nested_json_string()
    }

    #[test]
    fn redact_matches_multiple_occurrences_of_same_secret() {
        super::redact_matches_multiple_occurrences_of_same_secret()
    }
}
