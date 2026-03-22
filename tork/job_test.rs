//! Tests for the job module

#[cfg(test)]
mod tests {
    use crate::job::{
        new_job_summary, new_scheduled_job_summary, Job, JobContext, JobDefaults, JobSchedule,
        ScheduledJob,
    };
    use crate::task::Task;
    use std::collections::HashMap;

    #[test]
    fn test_clone_job() {
        let mut inputs = HashMap::new();
        inputs.insert("INPUT1".to_string(), "VAL1".to_string());

        let mut job_ctx = JobContext::default();
        let mut ctx_inputs = HashMap::new();
        ctx_inputs.insert("INPUT1".to_string(), "VAL1".to_string());
        let mut ctx_job = HashMap::new();
        ctx_job.insert("id".to_string(), "some-id".to_string());
        ctx_job.insert("name".to_string(), "my job".to_string());
        job_ctx.inputs = Some(ctx_inputs);
        job_ctx.job = Some(ctx_job);

        let j1 = Job {
            context: job_ctx,
            tasks: vec![Task {
                env: Some({
                    let mut m = HashMap::new();
                    m.insert("VAR1".to_string(), "VAL1".to_string());
                    m
                }),
                ..Task::default()
            }],
            execution: vec![Task {
                env: Some({
                    let mut m = HashMap::new();
                    m.insert("EVAR1".to_string(), "EVAL1".to_string());
                    m
                }),
                ..Task::default()
            }],
            ..Job::default()
        };

        let mut j2 = j1.clone();

        // Verify context inputs was deeply cloned
        assert_eq!(j1.context.inputs, j2.context.inputs);
        // Verify context job was deeply cloned
        assert_eq!(j1.context.job, j2.context.job);
        // Verify tasks was deeply cloned
        assert_eq!(
            j1.tasks[0].env.as_ref().unwrap().get("VAR1"),
            j2.tasks[0].env.as_ref().unwrap().get("VAR1")
        );
        // Verify execution was deeply cloned
        assert_eq!(
            j1.execution[0].env.as_ref().unwrap().get("EVAR1"),
            j2.execution[0].env.as_ref().unwrap().get("EVAR1")
        );

        // Modify cloned values
        j2.context
            .inputs
            .as_mut()
            .unwrap()
            .insert("INPUT2".to_string(), "VAL2".to_string());
        j2.tasks[0]
            .env
            .as_mut()
            .unwrap()
            .insert("VAR2".to_string(), "VAL2".to_string());
        j2.execution[0]
            .env
            .as_mut()
            .unwrap()
            .insert("EVAR2".to_string(), "VAL2".to_string());

        // Verify original is unchanged
        assert!(j1.context.inputs.as_ref().unwrap().get("INPUT2").is_none());
        assert!(j1.tasks[0].env.as_ref().unwrap().get("VAR2").is_none());
        assert!(j1.execution[0].env.as_ref().unwrap().get("EVAR2").is_none());
    }

    #[test]
    fn test_job_context_clone() {
        let mut ctx = JobContext::default();
        let mut inputs = HashMap::new();
        inputs.insert("key1".to_string(), "val1".to_string());
        ctx.inputs = Some(inputs);

        let mut cloned = ctx.clone();
        assert_eq!(ctx.inputs, cloned.inputs);

        // Modify clone
        cloned
            .inputs
            .as_mut()
            .unwrap()
            .insert("key2".to_string(), "val2".to_string());
        assert!(ctx.inputs.as_ref().unwrap().get("key2").is_none());
    }

    #[test]
    fn test_job_context_as_map() {
        let mut ctx = JobContext::default();
        let mut inputs = HashMap::new();
        inputs.insert("INPUT1".to_string(), "VAL1".to_string());
        ctx.inputs = Some(inputs);

        let map = ctx.as_map();
        assert!(map.contains_key("inputs"));
    }

    #[test]
    fn test_job_defaults_clone() {
        let defaults = JobDefaults {
            retry: Some(crate::task::TaskRetry {
                limit: 3,
                attempts: 1,
            }),
            limits: Some(crate::task::TaskLimits {
                cpus: Some("2".to_string()),
                memory: Some("4Gi".to_string()),
            }),
            timeout: Some("5m".to_string()),
            queue: Some("default".to_string()),
            priority: 10,
        };

        let cloned = defaults.clone();
        assert_eq!(
            defaults.retry.as_ref().unwrap().limit,
            cloned.retry.as_ref().unwrap().limit
        );
        assert_eq!(
            defaults.limits.as_ref().unwrap().cpus,
            cloned.limits.as_ref().unwrap().cpus
        );
        assert_eq!(defaults.timeout, cloned.timeout);
        assert_eq!(defaults.queue, cloned.queue);
        assert_eq!(defaults.priority, cloned.priority);
    }

    #[test]
    fn test_job_schedule_clone() {
        let schedule = JobSchedule {
            id: Some("sched-1".to_string()),
            cron: Some("0 * * * *".to_string()),
        };
        let cloned = schedule.clone();
        assert_eq!(schedule.id, cloned.id);
        assert_eq!(schedule.cron, cloned.cron);
    }

    #[test]
    fn test_scheduled_job_clone() {
        let sj1 = ScheduledJob {
            id: Some("sj-1".to_string()),
            name: Some("My Scheduled Job".to_string()),
            cron: Some("0 * * * *".to_string()),
            tasks: vec![Task {
                name: Some("task1".to_string()),
                ..Task::default()
            }],
            ..ScheduledJob::default()
        };

        let sj2 = sj1.clone();
        assert_eq!(sj1.id, sj2.id);
        assert_eq!(sj1.name, sj2.name);
        assert_eq!(sj1.cron, sj2.cron);
        assert_eq!(sj1.tasks.len(), sj2.tasks.len());
        assert_eq!(sj1.tasks[0].name, sj2.tasks[0].name);
    }

    #[test]
    fn test_new_job_summary() {
        let job = Job {
            id: Some("job-1".to_string()),
            name: Some("My Job".to_string()),
            description: Some("A test job".to_string()),
            state: crate::job::JOB_STATE_RUNNING.to_string(),
            position: 5,
            task_count: 10,
            progress: 0.5,
            tags: Some(vec!["tag1".to_string()]),
            ..Job::default()
        };

        let summary = new_job_summary(&job);
        assert_eq!(summary.id, job.id);
        assert_eq!(summary.name, job.name);
        assert_eq!(summary.description, job.description);
        assert_eq!(summary.state, job.state);
        assert_eq!(summary.position, job.position);
        assert_eq!(summary.task_count, job.task_count);
        assert_eq!(summary.progress, job.progress);
        assert_eq!(summary.tags, job.tags);
    }

    #[test]
    fn test_new_scheduled_job_summary() {
        let sj = ScheduledJob {
            id: Some("sj-1".to_string()),
            name: Some("My Scheduled Job".to_string()),
            description: Some("A test scheduled job".to_string()),
            cron: Some("0 * * * *".to_string()),
            state: crate::job::SCHEDULED_JOB_STATE_ACTIVE.to_string(),
            tags: Some(vec!["scheduled".to_string()]),
            ..ScheduledJob::default()
        };

        let summary = new_scheduled_job_summary(&sj);
        assert_eq!(summary.id, sj.id);
        assert_eq!(summary.name, sj.name);
        assert_eq!(summary.description, sj.description);
        assert_eq!(summary.cron, sj.cron);
        assert_eq!(summary.state, sj.state);
        assert_eq!(summary.tags, sj.tags);
    }
}
