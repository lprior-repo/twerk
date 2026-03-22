//! Tests for the task module

#[cfg(test)]
mod tests {
    use crate::task::{
        clone_tasks, new_task_summary, EachTask, ParallelTask, Probe, Registry, Task, TaskLimits,
        TaskRetry, TASK_STATE_CANCELLED, TASK_STATE_COMPLETED, TASK_STATE_CREATED,
        TASK_STATE_PENDING, TASK_STATE_RUNNING,
    };
    use std::collections::HashMap;

    #[test]
    fn test_clone_task() {
        let mut env = HashMap::new();
        env.insert("VAR1".to_string(), "VAL1".to_string());

        let t1 = Task {
            env: Some(env),
            limits: Some(TaskLimits {
                cpus: Some("1".to_string()),
                memory: None,
            }),
            parallel: Some(ParallelTask {
                tasks: Some(vec![Task {
                    env: Some({
                        let mut m = HashMap::new();
                        m.insert("PVAR1".to_string(), "PVAL1".to_string());
                        m
                    }),
                    ..Task::default()
                }]),
                completions: 1,
            }),
            ..Task::default()
        };

        let mut t2 = t1.clone();

        // Verify env was deeply cloned
        assert_eq!(t1.env, t2.env);

        // Verify limits was deeply cloned
        assert_eq!(
            t1.limits.as_ref().unwrap().cpus,
            t2.limits.as_ref().unwrap().cpus
        );

        // Verify parallel tasks were deeply cloned
        assert_eq!(
            t1.parallel.as_ref().unwrap().tasks.as_ref().unwrap()[0]
                .env
                .as_ref()
                .unwrap()
                .get("PVAR1"),
            t2.parallel.as_ref().unwrap().tasks.as_ref().unwrap()[0]
                .env
                .as_ref()
                .unwrap()
                .get("PVAR1")
        );

        // Modify cloned values
        t2.env
            .as_mut()
            .unwrap()
            .insert("VAR2".to_string(), "VAL2".to_string());
        t2.limits.as_mut().unwrap().cpus = Some("2".to_string());
        t2.parallel.as_mut().unwrap().tasks.as_mut().unwrap()[0]
            .env
            .as_mut()
            .unwrap()
            .insert("PVAR2".to_string(), "PVAL2".to_string());

        // Verify original is unchanged
        assert!(t1.env.as_ref().unwrap().get("VAR2").is_none());
        assert_eq!(t1.limits.as_ref().unwrap().cpus.as_ref().unwrap(), "1");
        assert!(t1.parallel.as_ref().unwrap().tasks.as_ref().unwrap()[0]
            .env
            .as_ref()
            .unwrap()
            .get("PVAR2")
            .is_none());
    }

    #[test]
    fn test_is_active() {
        let t1 = Task {
            state: TASK_STATE_CANCELLED,
            ..Task::default()
        };
        assert!(!t1.is_active());

        let t2 = Task {
            state: TASK_STATE_CREATED,
            ..Task::default()
        };
        assert!(t2.is_active());

        let t3 = Task {
            state: TASK_STATE_PENDING,
            ..Task::default()
        };
        assert!(t3.is_active());

        let t4 = Task {
            state: TASK_STATE_RUNNING,
            ..Task::default()
        };
        assert!(t4.is_active());

        let t5 = Task {
            state: TASK_STATE_COMPLETED,
            ..Task::default()
        };
        assert!(!t5.is_active());
    }

    #[test]
    fn test_clone_tasks() {
        let tasks = vec![
            Task {
                name: Some("task1".to_string()),
                ..Task::default()
            },
            Task {
                name: Some("task2".to_string()),
                ..Task::default()
            },
        ];

        let cloned = clone_tasks(&tasks);

        assert_eq!(tasks.len(), cloned.len());
        assert_eq!(tasks[0].name, cloned[0].name);
        assert_eq!(tasks[1].name, cloned[1].name);

        // Verify deep clone (different memory allocations)
        assert_ne!(
            tasks[0].name.as_ref().unwrap().as_ptr(),
            cloned[0].name.as_ref().unwrap().as_ptr()
        );
    }

    #[test]
    fn test_task_retry_clone() {
        let retry = TaskRetry {
            limit: 3,
            attempts: 1,
        };
        let cloned = retry.clone();
        assert_eq!(retry.limit, cloned.limit);
        assert_eq!(retry.attempts, cloned.attempts);
    }

    #[test]
    fn test_task_limits_clone() {
        let limits = TaskLimits {
            cpus: Some("2".to_string()),
            memory: Some("4Gi".to_string()),
        };
        let cloned = limits.clone();
        assert_eq!(limits.cpus, cloned.cpus);
        assert_eq!(limits.memory, cloned.memory);
    }

    #[test]
    fn test_each_task_clone() {
        let each = EachTask {
            var: Some("i".to_string()),
            list: Some("1,2,3".to_string()),
            task: Some(Box::new(Task {
                name: Some("inner".to_string()),
                ..Task::default()
            })),
            size: 3,
            completions: 3,
            concurrency: 1,
            index: 0,
        };
        let cloned = each.clone();
        assert_eq!(each.var, cloned.var);
        assert_eq!(each.list, cloned.list);
        assert_eq!(each.size, cloned.size);
        assert_eq!(
            each.task.as_ref().unwrap().name,
            cloned.task.as_ref().unwrap().name
        );
    }

    #[test]
    fn test_registry_clone() {
        let reg = Registry {
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
        };
        let cloned = reg.clone();
        assert_eq!(reg.username, cloned.username);
        assert_eq!(reg.password, cloned.password);
    }

    #[test]
    fn test_probe_clone() {
        let probe = Probe {
            path: Some("/health".to_string()),
            port: 8080,
            timeout: Some("5s".to_string()),
        };
        let cloned = probe.clone();
        assert_eq!(probe.path, cloned.path);
        assert_eq!(probe.port, cloned.port);
        assert_eq!(probe.timeout, cloned.timeout);
    }

    #[test]
    fn test_new_task_summary() {
        let task = Task {
            id: Some("task-1".to_string()),
            job_id: Some("job-1".to_string()),
            position: 5,
            progress: 0.75,
            name: Some("My Task".to_string()),
            description: Some("A test task".to_string()),
            state: TASK_STATE_RUNNING,
            tags: Some(vec!["tag1".to_string(), "tag2".to_string()]),
            ..Task::default()
        };

        let summary = new_task_summary(&task);

        assert_eq!(summary.id, task.id);
        assert_eq!(summary.job_id, task.job_id);
        assert_eq!(summary.position, task.position);
        assert_eq!(summary.progress, task.progress);
        assert_eq!(summary.name, task.name);
        assert_eq!(summary.description, task.description);
        assert_eq!(summary.state, task.state);
        assert_eq!(summary.tags, task.tags);
    }
}
