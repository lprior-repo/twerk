use std::collections::HashMap;
use twerk_core::id::{JobId, ScheduledJobId};
use twerk_core::job::{
    job_event_from_state, new_job_summary, new_scheduled_job_summary, Job, JobContext, JobEvent,
    JobState, ScheduledJob, ScheduledJobState,
};
use twerk_core::mount::Mount;
use twerk_core::redact::{redact_job, redact_task, Redacter};
use twerk_core::task::{SubJobTask, Task};
use twerk_core::webhook::Webhook;

fn uuid_str() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn make_job(state: JobState) -> Job {
    Job {
        id: Some(JobId::new(uuid_str()).unwrap()),
        name: Some("test".into()),
        state,
        ..Default::default()
    }
}

fn secret_map() -> HashMap<String, String> {
    HashMap::from([("db_password".into(), "super-secret-123".into())])
}

#[test]
fn mutation_job_state_display_not_default() {
    assert_eq!(format!("{}", JobState::Pending), "PENDING");
    assert_eq!(format!("{}", JobState::Failed), "FAILED");
    assert_eq!(format!("{}", JobState::Cancelled), "CANCELLED");
    assert_eq!(format!("{}", JobState::Completed), "COMPLETED");
    assert_eq!(format!("{}", JobState::Running), "RUNNING");
    assert_eq!(format!("{}", JobState::Scheduled), "SCHEDULED");
    assert_eq!(format!("{}", JobState::Restart), "RESTART");
}

#[test]
fn mutation_parse_job_state_error_display() {
    let err = "bogus".parse::<JobState>().unwrap_err();
    let display = format!("{err}");
    assert!(display.contains("BOGUS") || display.contains("bogus"));
    assert!(!display.is_empty());
}

#[test]
fn mutation_can_transition_to_pending_scheduled_only() {
    assert!(JobState::Pending.can_transition_to(&JobState::Scheduled));
    assert!(!JobState::Pending.can_transition_to(&JobState::Running));
    assert!(!JobState::Pending.can_transition_to(&JobState::Completed));
    assert!(!JobState::Pending.can_transition_to(&JobState::Pending));
}

#[test]
fn mutation_can_transition_to_scheduled_running_only() {
    assert!(JobState::Scheduled.can_transition_to(&JobState::Running));
    assert!(!JobState::Scheduled.can_transition_to(&JobState::Pending));
}

#[test]
fn mutation_can_transition_running_to_terminal() {
    let running = JobState::Running;
    assert!(running.can_transition_to(&JobState::Completed));
    assert!(running.can_transition_to(&JobState::Failed));
    assert!(running.can_transition_to(&JobState::Cancelled));
    assert!(!running.can_transition_to(&JobState::Pending));
}

#[test]
fn mutation_can_transition_failed_cancelled_to_restart() {
    assert!(JobState::Failed.can_transition_to(&JobState::Restart));
    assert!(JobState::Cancelled.can_transition_to(&JobState::Restart));
    assert!(!JobState::Completed.can_transition_to(&JobState::Restart));
}

#[test]
fn mutation_can_transition_restart_to_pending() {
    assert!(JobState::Restart.can_transition_to(&JobState::Pending));
    assert!(!JobState::Restart.can_transition_to(&JobState::Running));
}

#[test]
fn mutation_can_cancel_only_cancellable_states() {
    assert!(JobState::Pending.can_cancel());
    assert!(JobState::Scheduled.can_cancel());
    assert!(JobState::Running.can_cancel());
    assert!(!JobState::Completed.can_cancel());
    assert!(!JobState::Failed.can_cancel());
    assert!(!JobState::Cancelled.can_cancel());
    assert!(!JobState::Restart.can_cancel());
}

#[test]
fn mutation_can_restart_only_failed_cancelled() {
    assert!(JobState::Failed.can_restart());
    assert!(JobState::Cancelled.can_restart());
    assert!(!JobState::Pending.can_restart());
    assert!(!JobState::Completed.can_restart());
    assert!(!JobState::Restart.can_restart());
}

#[test]
fn mutation_job_event_job_id_state_changed() {
    let id = JobId::new(uuid_str()).unwrap();
    let event = JobEvent::StateChanged {
        job_id: id.clone(),
        old_state: JobState::Pending,
        new_state: JobState::Scheduled,
    };
    assert!(event.job_id().is_some());
    assert_eq!(event.job_id().unwrap(), &id);
}

#[test]
fn mutation_job_event_job_id_completed() {
    let job = make_job(JobState::Completed);
    let event = JobEvent::Completed(job);
    assert!(event.job_id().is_some());
}

#[test]
fn mutation_job_event_into_job_state_changed_returns_none() {
    let event = JobEvent::StateChanged {
        job_id: JobId::new(uuid_str()).unwrap(),
        old_state: JobState::Pending,
        new_state: JobState::Scheduled,
    };
    assert!(event.into_job().is_none());
}

#[test]
fn mutation_job_event_into_job_completed_returns_some() {
    let job = make_job(JobState::Completed);
    let event = JobEvent::Completed(job.clone());
    let result = event.into_job();
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, job.id);
}

#[test]
fn mutation_job_event_from_state_completed() {
    let job = make_job(JobState::Completed);
    assert!(matches!(
        job_event_from_state(&job),
        Some(JobEvent::Completed(_))
    ));
}

#[test]
fn mutation_job_event_from_state_failed() {
    let job = make_job(JobState::Failed);
    assert!(matches!(
        job_event_from_state(&job),
        Some(JobEvent::Failed(_))
    ));
}

#[test]
fn mutation_job_event_from_state_cancelled() {
    let job = make_job(JobState::Cancelled);
    assert!(matches!(
        job_event_from_state(&job),
        Some(JobEvent::Cancelled(_))
    ));
}

#[test]
fn mutation_job_event_from_state_pending_returns_none() {
    assert!(job_event_from_state(&make_job(JobState::Pending)).is_none());
}

#[test]
fn mutation_job_event_from_state_running_returns_none() {
    assert!(job_event_from_state(&make_job(JobState::Running)).is_none());
}

#[test]
fn mutation_scheduled_job_state_display() {
    assert_eq!(format!("{}", ScheduledJobState::Active), "ACTIVE");
    assert_eq!(format!("{}", ScheduledJobState::Paused), "PAUSED");
}

#[test]
fn mutation_parse_scheduled_job_state_error_display() {
    let err = "INVALID".parse::<ScheduledJobState>().unwrap_err();
    let display = format!("{err}");
    assert!(display.contains("INVALID"));
    assert!(!display.is_empty());
}

#[test]
fn mutation_scheduled_job_state_from_str_roundtrip() {
    for state in [ScheduledJobState::Active, ScheduledJobState::Paused] {
        let parsed: ScheduledJobState = format!("{state}").parse().unwrap();
        assert_eq!(state, parsed);
    }
}

#[test]
fn mutation_deep_clone_preserves_fields() {
    let job = make_job(JobState::Running);
    assert_eq!(job.deep_clone(), job);
}

#[test]
fn mutation_job_context_as_map_with_all_fields() {
    let ctx = JobContext {
        inputs: Some(HashMap::from([("key".into(), "val".into())])),
        secrets: Some(HashMap::from([("sec".into(), "ret".into())])),
        tasks: Some(HashMap::from([("task".into(), "data".into())])),
        job: Some(HashMap::from([("jk".into(), "jv".into())])),
    };
    let map = ctx.as_map();
    assert_eq!(map.len(), 4);
    assert!(map.contains_key("inputs"));
    assert!(map.contains_key("secrets"));
    assert!(map.contains_key("tasks"));
    assert!(map.contains_key("job"));
}

#[test]
fn mutation_job_context_as_map_empty() {
    assert!(JobContext::default().as_map().is_empty());
}

#[test]
fn mutation_new_job_summary_copies_fields() {
    let job = make_job(JobState::Running);
    let summary = new_job_summary(&job);
    assert_eq!(summary.id, job.id);
    assert_eq!(summary.state, job.state);
    assert_eq!(summary.name, job.name);
}

#[test]
fn mutation_new_scheduled_job_summary_copies_fields() {
    let sj = ScheduledJob {
        id: Some(ScheduledJobId::new("abc").unwrap()),
        name: Some("scheduled".into()),
        state: ScheduledJobState::Active,
        cron: Some("*/5 * * * *".into()),
        ..Default::default()
    };
    let summary = new_scheduled_job_summary(&sj);
    assert_eq!(summary.id, sj.id);
    assert_eq!(summary.name, sj.name);
    assert_eq!(summary.state, sj.state);
    assert_eq!(summary.cron, sj.cron);
}

#[test]
fn mutation_redact_keys_returns_actual_keys() {
    let r = Redacter::new(vec!["SECRET".into(), "TOKEN".into()]);
    assert_eq!(r.keys().len(), 2);
    assert!(r.keys().contains(&"SECRET".into()));
    assert!(r.keys().contains(&"TOKEN".into()));
}

#[test]
fn mutation_redact_task_mounts_preserves_opts() {
    let task = Task {
        mounts: Some(vec![Mount {
            opts: Some(HashMap::from([(
                "PASSWORD".into(),
                "super-secret-123".into(),
            )])),
            ..Default::default()
        }]),
        ..Default::default()
    };
    let redacted = redact_task(task, &secret_map());
    let opts = redacted.mounts.as_ref().unwrap()[0].opts.as_ref().unwrap();
    assert_eq!(opts.get("PASSWORD").unwrap(), "[REDACTED]");
}

#[test]
fn mutation_redact_task_subjob_webhooks_headers() {
    let task = Task {
        subjob: Some(SubJobTask {
            webhooks: Some(vec![Webhook {
                headers: Some(HashMap::from([(
                    "X-Secret".into(),
                    "super-secret-123".into(),
                )])),
                ..Default::default()
            }]),
            secrets: Some(HashMap::from([("api_key".into(), "val".into())])),
            ..Default::default()
        }),
        ..Default::default()
    };
    let redacted = redact_task(task, &secret_map());
    let sj = redacted.subjob.as_ref().unwrap();
    assert_eq!(
        sj.webhooks.as_ref().unwrap()[0]
            .headers
            .as_ref()
            .unwrap()
            .get("X-Secret")
            .unwrap(),
        "[REDACTED]"
    );
    assert_eq!(
        sj.secrets.as_ref().unwrap().get("api_key").unwrap(),
        "[REDACTED]"
    );
}

#[test]
fn mutation_redact_nested_tasks_processes_children() {
    let mut env = HashMap::new();
    env.insert("PASSWORD".into(), "super-secret-123".into());
    let inner = Task {
        env: Some(env),
        ..Default::default()
    };
    let task = Task {
        parallel: Some(twerk_core::task::ParallelTask {
            tasks: Some(vec![inner]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let redacted = redact_task(task, &secret_map());
    let inner_tasks = redacted.parallel.as_ref().unwrap().tasks.as_ref().unwrap();
    assert_eq!(
        inner_tasks[0]
            .env
            .as_ref()
            .unwrap()
            .get("PASSWORD")
            .unwrap(),
        "[REDACTED]"
    );
}

#[test]
fn mutation_redact_task_pre_post_sidecars() {
    let make_inner = || {
        let mut env = HashMap::new();
        env.insert("PASSWORD".into(), "super-secret-123".into());
        Task {
            env: Some(env),
            ..Default::default()
        }
    };
    let task = Task {
        pre: Some(vec![make_inner()]),
        post: Some(vec![make_inner()]),
        sidecars: Some(vec![make_inner()]),
        ..Default::default()
    };
    let redacted = redact_task(task, &secret_map());
    for field in [&redacted.pre, &redacted.post, &redacted.sidecars] {
        assert_eq!(
            field.as_ref().unwrap()[0]
                .env
                .as_ref()
                .unwrap()
                .get("PASSWORD")
                .unwrap(),
            "[REDACTED]"
        );
    }
}

#[test]
fn mutation_redact_job_full_pipeline() {
    let job = Job {
        inputs: Some(HashMap::from([(
            "db_password".into(),
            "super-secret-123".into(),
        )])),
        webhooks: Some(vec![Webhook {
            headers: Some(HashMap::from([("Auth".into(), "super-secret-123".into())])),
            ..Default::default()
        }]),
        context: Some(JobContext {
            inputs: Some(HashMap::from([("k".into(), "super-secret-123".into())])),
            secrets: Some(HashMap::from([("s".into(), "super-secret-123".into())])),
            tasks: Some(HashMap::from([("t".into(), "super-secret-123".into())])),
            ..Default::default()
        }),
        secrets: Some(secret_map()),
        ..Default::default()
    };
    let r = redact_job(job);
    assert_eq!(
        r.inputs.as_ref().unwrap().get("db_password").unwrap(),
        "[REDACTED]"
    );
    assert_eq!(
        r.webhooks.as_ref().unwrap()[0]
            .headers
            .as_ref()
            .unwrap()
            .get("Auth")
            .unwrap(),
        "[REDACTED]"
    );
    let ctx = r.context.as_ref().unwrap();
    assert_eq!(ctx.inputs.as_ref().unwrap().get("k").unwrap(), "[REDACTED]");
    assert_eq!(
        ctx.secrets.as_ref().unwrap().get("s").unwrap(),
        "[REDACTED]"
    );
    assert_eq!(ctx.tasks.as_ref().unwrap().get("t").unwrap(), "[REDACTED]");
}

#[test]
fn mutation_mount_with_source_and_id() {
    let m = Mount::default().with_source("/data").with_id("vol-1");
    assert_eq!(m.source.as_ref().unwrap(), "/data");
    assert_eq!(m.id.as_ref().unwrap(), "vol-1");
}
