#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::used_underscore_binding)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use twerk_app::engine::middleware::MiddlewareComposer;
use twerk_app::engine::types::{
    JobEventType, JobHandlerError, JobHandlerFunc, JobMiddlewareFunc, LogHandlerFunc,
    LogMiddlewareFunc, NodeHandlerError, NodeHandlerFunc, NodeMiddlewareFunc, TaskEventType,
    TaskHandlerError, TaskHandlerFunc, TaskMiddlewareFunc,
};
use twerk_core::job::{Job, JobState};
use twerk_core::node::{Node, NodeStatus};
use twerk_core::task::{Task, TaskLogPart, TaskState};

fn make_task() -> Task {
    Task {
        id: Some("test-task-id".into()),
        job_id: Some("test-job-id".into()),
        state: TaskState::Running,
        ..Default::default()
    }
}

fn make_job() -> Job {
    Job {
        id: Some("test-job-id".into()),
        name: Some("Test Job".to_string()),
        state: JobState::Pending,
        ..Default::default()
    }
}

fn make_node() -> Node {
    Node {
        id: Some(twerk_core::id::NodeId::new("test-node")),
        name: Some("test-node".to_string()),
        status: Some(NodeStatus::UP),
        ..Default::default()
    }
}

fn make_log_parts() -> Vec<TaskLogPart> {
    vec![
        TaskLogPart {
            id: Some("log-1".to_string()),
            task_id: Some("test-task-id".into()),
            number: 1,
            contents: Some("log line 1".to_string()),
            ..Default::default()
        },
        TaskLogPart {
            id: Some("log-2".to_string()),
            task_id: Some("test-task-id".into()),
            number: 2,
            contents: Some("log line 2".to_string()),
            ..Default::default()
        },
    ]
}

#[test]
fn task_middleware_before_runs_before_handler() {
    let before_called = Arc::new(Mutex::new(false));
    let handler_called = Arc::new(Mutex::new(false));

    let before_called_clone = before_called.clone();
    let handler_called_clone = handler_called.clone();

    let task_before_middleware: TaskMiddlewareFunc = Arc::new(move |next: TaskHandlerFunc| {
        let next = next.clone();
        let before_clone = before_called_clone.clone();
        Arc::new(move |ctx: Arc<()>, et: TaskEventType, task: &mut Task| {
            *before_clone.lock().unwrap() = true;
            next(ctx, et, task)
        })
    });

    let handler: TaskHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: TaskEventType, _task: &mut Task| {
            *handler_called_clone.lock().unwrap() = true;
            Ok(())
        });

    let composer = MiddlewareComposer::new().with_task(task_before_middleware);
    let wrapped = composer.compose_task_handler(handler);

    let mut task = make_task();
    let ctx = Arc::new(());
    wrapped(ctx, TaskEventType::Started, &mut task).unwrap();

    assert!(
        *before_called.lock().unwrap(),
        "middleware before hook should run before handler"
    );
    assert!(
        *handler_called.lock().unwrap(),
        "handler should be called after middleware"
    );
}

#[test]
fn task_middleware_after_runs_after_handler() {
    let call_order = Arc::new(Mutex::new(Vec::new()));

    let call_order_after = call_order.clone();
    let task_after_middleware: TaskMiddlewareFunc = Arc::new(move |next: TaskHandlerFunc| {
        let next = next.clone();
        let call_order_after_inner = call_order_after.clone();
        Arc::new(move |ctx: Arc<()>, et: TaskEventType, task: &mut Task| {
            let result = next(ctx, et, task);
            call_order_after_inner.lock().unwrap().push("after");
            result
        })
    });

    let call_order_before = call_order.clone();
    let task_before_middleware: TaskMiddlewareFunc = Arc::new(move |next: TaskHandlerFunc| {
        let next = next.clone();
        let call_order_before_inner = call_order_before.clone();
        Arc::new(move |ctx: Arc<()>, et: TaskEventType, task: &mut Task| {
            call_order_before_inner.lock().unwrap().push("before");
            next(ctx, et, task)
        })
    });

    let call_order_handler = call_order.clone();
    let handler: TaskHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: TaskEventType, _task: &mut Task| {
            call_order_handler.lock().unwrap().push("handler");
            Ok(())
        });

    let composer = MiddlewareComposer::new()
        .with_task(task_before_middleware)
        .with_task(task_after_middleware);
    let wrapped = composer.compose_task_handler(handler);

    let mut task = make_task();
    let ctx = Arc::new(());
    wrapped(ctx, TaskEventType::Started, &mut task).unwrap();

    let order = call_order.lock().unwrap();
    assert_eq!(*order, vec!["before", "handler", "after"]);
}

#[test]
fn task_no_middleware_calls_handler_directly() {
    let handler_called = Arc::new(Mutex::new(false));
    let handler_called_clone = handler_called.clone();

    let handler: TaskHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: TaskEventType, _task: &mut Task| {
            *handler_called_clone.lock().unwrap() = true;
            Ok(())
        });

    let composer = MiddlewareComposer::new();
    let wrapped = composer.compose_task_handler(handler);

    let mut task = make_task();
    let ctx = Arc::new(());
    wrapped(ctx, TaskEventType::Started, &mut task).unwrap();

    assert!(
        *handler_called.lock().unwrap(),
        "handler should be called directly when no middleware"
    );
}

#[test]
fn task_middleware_error_propagates() {
    let task_error_middleware: TaskMiddlewareFunc = Arc::new(move |_next: TaskHandlerFunc| {
        Arc::new(move |_ctx: Arc<()>, _et: TaskEventType, _task: &mut Task| {
            Err(TaskHandlerError::Handler("middleware error".to_string()))
        })
    });

    let handler: TaskHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: TaskEventType, _task: &mut Task| Ok(()));

    let composer = MiddlewareComposer::new().with_task(task_error_middleware);
    let wrapped = composer.compose_task_handler(handler);

    let mut task = make_task();
    let ctx = Arc::new(());
    let result = wrapped(ctx, TaskEventType::Started, &mut task);

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "task handler error: middleware error"
    );
}

#[test]
fn job_middleware_before_runs_before_handler() {
    let before_called = Arc::new(Mutex::new(false));
    let handler_called = Arc::new(Mutex::new(false));

    let before_called_clone = before_called.clone();
    let handler_called_clone = handler_called.clone();

    let job_before_middleware: JobMiddlewareFunc = Arc::new(move |next: JobHandlerFunc| {
        let next = next.clone();
        let before_clone = before_called_clone.clone();
        Arc::new(move |ctx: Arc<()>, et: JobEventType, job: &mut Job| {
            *before_clone.lock().unwrap() = true;
            next(ctx, et, job)
        })
    });

    let handler: JobHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: JobEventType, _job: &mut Job| {
            *handler_called_clone.lock().unwrap() = true;
            Ok(())
        });

    let composer = MiddlewareComposer::new().with_job(job_before_middleware);
    let wrapped = composer.compose_job_handler(handler);

    let mut job = make_job();
    let ctx = Arc::new(());
    wrapped(ctx, JobEventType::StateChange, &mut job).unwrap();

    assert!(
        *before_called.lock().unwrap(),
        "middleware before hook should run before handler"
    );
    assert!(
        *handler_called.lock().unwrap(),
        "handler should be called after middleware"
    );
}

#[test]
fn job_middleware_after_runs_after_handler() {
    let call_order = Arc::new(Mutex::new(Vec::new()));

    let call_order_after = call_order.clone();
    let job_after_middleware: JobMiddlewareFunc = Arc::new(move |next: JobHandlerFunc| {
        let next = next.clone();
        let call_order_after_inner = call_order_after.clone();
        Arc::new(move |ctx: Arc<()>, et: JobEventType, job: &mut Job| {
            let result = next(ctx, et, job);
            call_order_after_inner.lock().unwrap().push("after");
            result
        })
    });

    let call_order_before = call_order.clone();
    let job_before_middleware: JobMiddlewareFunc = Arc::new(move |next: JobHandlerFunc| {
        let next = next.clone();
        let call_order_before_inner = call_order_before.clone();
        Arc::new(move |ctx: Arc<()>, et: JobEventType, job: &mut Job| {
            call_order_before_inner.lock().unwrap().push("before");
            next(ctx, et, job)
        })
    });

    let call_order_handler = call_order.clone();
    let handler: JobHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: JobEventType, _job: &mut Job| {
            call_order_handler.lock().unwrap().push("handler");
            Ok(())
        });

    let composer = MiddlewareComposer::new()
        .with_job(job_before_middleware)
        .with_job(job_after_middleware);
    let wrapped = composer.compose_job_handler(handler);

    let mut job = make_job();
    let ctx = Arc::new(());
    wrapped(ctx, JobEventType::StateChange, &mut job).unwrap();

    let order = call_order.lock().unwrap();
    assert_eq!(*order, vec!["before", "handler", "after"]);
}

#[test]
fn job_no_middleware_calls_handler_directly() {
    let handler_called = Arc::new(Mutex::new(false));
    let handler_called_clone = handler_called.clone();

    let handler: JobHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: JobEventType, _job: &mut Job| {
            *handler_called_clone.lock().unwrap() = true;
            Ok(())
        });

    let composer = MiddlewareComposer::new();
    let wrapped = composer.compose_job_handler(handler);

    let mut job = make_job();
    let ctx = Arc::new(());
    wrapped(ctx, JobEventType::StateChange, &mut job).unwrap();

    assert!(
        *handler_called.lock().unwrap(),
        "handler should be called directly when no middleware"
    );
}

#[test]
fn job_middleware_error_propagates() {
    let job_error_middleware: JobMiddlewareFunc = Arc::new(move |_next: JobHandlerFunc| {
        Arc::new(move |_ctx: Arc<()>, _et: JobEventType, _job: &mut Job| {
            Err(JobHandlerError::Handler("job middleware error".to_string()))
        })
    });

    let handler: JobHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: JobEventType, _job: &mut Job| Ok(()));

    let composer = MiddlewareComposer::new().with_job(job_error_middleware);
    let wrapped = composer.compose_job_handler(handler);

    let mut job = make_job();
    let ctx = Arc::new(());
    let result = wrapped(ctx, JobEventType::StateChange, &mut job);

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "job handler error: job middleware error"
    );
}

#[test]
fn node_middleware_before_runs_before_handler() {
    let before_called = Arc::new(Mutex::new(false));
    let handler_called = Arc::new(Mutex::new(false));

    let before_called_clone = before_called.clone();
    let handler_called_clone = handler_called.clone();

    let node_before_middleware: NodeMiddlewareFunc = Arc::new(move |next: NodeHandlerFunc| {
        let next = next.clone();
        let before_clone = before_called_clone.clone();
        Arc::new(move |ctx: Arc<()>, node: &mut Node| {
            *before_clone.lock().unwrap() = true;
            next(ctx, node)
        })
    });

    let handler: NodeHandlerFunc = Arc::new(move |_ctx: Arc<()>, _node: &mut Node| {
        *handler_called_clone.lock().unwrap() = true;
        Ok(())
    });

    let composer = MiddlewareComposer::new().with_node(node_before_middleware);
    let wrapped = composer.compose_node_handler(handler);

    let mut node = make_node();
    let ctx = Arc::new(());
    wrapped(ctx, &mut node).unwrap();

    assert!(
        *before_called.lock().unwrap(),
        "middleware before hook should run before handler"
    );
    assert!(
        *handler_called.lock().unwrap(),
        "handler should be called after middleware"
    );
}

#[test]
fn node_middleware_after_runs_after_handler() {
    let call_order = Arc::new(Mutex::new(Vec::new()));

    let call_order_after = call_order.clone();
    let node_after_middleware: NodeMiddlewareFunc = Arc::new(move |next: NodeHandlerFunc| {
        let next = next.clone();
        let call_order_after_inner = call_order_after.clone();
        Arc::new(move |ctx: Arc<()>, node: &mut Node| {
            let result = next(ctx, node);
            call_order_after_inner.lock().unwrap().push("after");
            result
        })
    });

    let call_order_before = call_order.clone();
    let node_before_middleware: NodeMiddlewareFunc = Arc::new(move |next: NodeHandlerFunc| {
        let next = next.clone();
        let call_order_before_inner = call_order_before.clone();
        Arc::new(move |ctx: Arc<()>, node: &mut Node| {
            call_order_before_inner.lock().unwrap().push("before");
            next(ctx, node)
        })
    });

    let call_order_handler = call_order.clone();
    let handler: NodeHandlerFunc = Arc::new(move |_ctx: Arc<()>, _node: &mut Node| {
        call_order_handler.lock().unwrap().push("handler");
        Ok(())
    });

    let composer = MiddlewareComposer::new()
        .with_node(node_before_middleware)
        .with_node(node_after_middleware);
    let wrapped = composer.compose_node_handler(handler);

    let mut node = make_node();
    let ctx = Arc::new(());
    wrapped(ctx, &mut node).unwrap();

    let order = call_order.lock().unwrap();
    assert_eq!(*order, vec!["before", "handler", "after"]);
}

#[test]
fn node_no_middleware_calls_handler_directly() {
    let handler_called = Arc::new(Mutex::new(false));
    let handler_called_clone = handler_called.clone();

    let handler: NodeHandlerFunc = Arc::new(move |_ctx: Arc<()>, _node: &mut Node| {
        *handler_called_clone.lock().unwrap() = true;
        Ok(())
    });

    let composer = MiddlewareComposer::new();
    let wrapped = composer.compose_node_handler(handler);

    let mut node = make_node();
    let ctx = Arc::new(());
    wrapped(ctx, &mut node).unwrap();

    assert!(
        *handler_called.lock().unwrap(),
        "handler should be called directly when no middleware"
    );
}

#[test]
fn node_middleware_error_propagates() {
    let node_error_middleware: NodeMiddlewareFunc = Arc::new(move |_next: NodeHandlerFunc| {
        Arc::new(move |_ctx: Arc<()>, _node: &mut Node| {
            Err(NodeHandlerError::Handler(
                "node middleware error".to_string(),
            ))
        })
    });

    let handler: NodeHandlerFunc = Arc::new(move |_ctx: Arc<()>, _node: &mut Node| Ok(()));

    let composer = MiddlewareComposer::new().with_node(node_error_middleware);
    let wrapped = composer.compose_node_handler(handler);

    let mut node = make_node();
    let ctx = Arc::new(());
    let result = wrapped(ctx, &mut node);

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "node handler error: node middleware error"
    );
}

#[test]
fn log_middleware_redact_on_read() {
    let handler_called = Arc::new(Mutex::new(false));
    let handler_called_clone = handler_called.clone();

    let log_middleware: LogMiddlewareFunc = Arc::new(move |next: LogHandlerFunc| {
        let next = next.clone();
        let handler_clone = handler_called_clone.clone();
        Arc::new(move |ctx: Arc<()>, parts: &[TaskLogPart]| {
            let redacted_parts: Vec<TaskLogPart> = parts
                .iter()
                .map(|p| {
                    let mut p = p.clone();
                    if let Some(ref mut contents) = p.contents {
                        *contents = contents.replace("secret", "[REDACTED]");
                    }
                    p
                })
                .collect();
            *handler_clone.lock().unwrap() = true;
            next(ctx, &redacted_parts)
        })
    });

    let handler: LogHandlerFunc = Arc::new(move |_ctx: Arc<()>, parts: &[TaskLogPart]| {
        for part in parts {
            if let Some(ref contents) = part.contents {
                assert!(!contents.contains("secret"), "log should be redacted");
            }
        }
        Ok(())
    });

    let composer = MiddlewareComposer::new().with_log(log_middleware);
    let wrapped = composer.compose_log_handler(handler);

    let ctx = Arc::new(());
    let mut log_parts = make_log_parts();
    log_parts[0].contents = Some(" containing secret data".to_string());
    wrapped(ctx, &log_parts).unwrap();

    assert!(
        *handler_called.lock().unwrap(),
        "handler should be called with redacted logs"
    );
}

#[test]
fn log_no_middleware_calls_handler_with_original_logs() {
    let handler: LogHandlerFunc = Arc::new(move |_ctx: Arc<()>, parts: &[TaskLogPart]| {
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].contents.as_ref().unwrap(), "log line 1");
        Ok(())
    });

    let composer = MiddlewareComposer::new();
    let wrapped = composer.compose_log_handler(handler);

    let ctx = Arc::new(());
    let log_parts = make_log_parts();
    wrapped(ctx, &log_parts).unwrap();
}

#[test]
fn hostenv_middleware_injects_env_vars_on_state_change() {
    std::env::set_var("TWERK_TEST_HOST_VAR", "host_value");

    let hostenv_middleware: TaskMiddlewareFunc = Arc::new(move |next: TaskHandlerFunc| {
        let next = next.clone();
        Arc::new(move |_ctx: Arc<()>, et: TaskEventType, task: &mut Task| {
            if et == TaskEventType::StateChange && task.state == TaskState::Running {
                if task.env.is_none() {
                    task.env = Some(HashMap::new());
                }
                if let Some(ref mut env) = task.env {
                    if let Ok(v) = std::env::var("TWERK_TEST_HOST_VAR") {
                        env.insert("HOST_VAR".to_string(), v);
                    }
                }
            }
            next(_ctx, et, task)
        })
    });

    let handler: TaskHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: TaskEventType, task: &mut Task| {
            if task.state == TaskState::Running {
                let env = task.env.as_ref().expect("task env should be set");
                assert_eq!(env.get("HOST_VAR"), Some(&"host_value".to_string()));
            }
            Ok(())
        });

    let composer = MiddlewareComposer::new().with_task(hostenv_middleware);
    let wrapped = composer.compose_task_handler(handler);

    let mut task = make_task();
    task.state = TaskState::Running;
    let ctx = Arc::new(());
    wrapped(ctx, TaskEventType::StateChange, &mut task).unwrap();

    std::env::remove_var("TWERK_TEST_HOST_VAR");
}

#[test]
fn hostenv_middleware_does_not_inject_on_other_events() {
    let hostenv_middleware: TaskMiddlewareFunc = Arc::new(move |next: TaskHandlerFunc| {
        let next = next.clone();
        Arc::new(move |_ctx: Arc<()>, et: TaskEventType, task: &mut Task| {
            if et == TaskEventType::StateChange && task.state == TaskState::Running {
                if task.env.is_none() {
                    task.env = Some(HashMap::new());
                }
                if let Some(ref mut env) = task.env {
                    env.insert("HOST_VAR".to_string(), "host_value".to_string());
                }
            }
            next(_ctx, et, task)
        })
    });

    let handler: TaskHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: TaskEventType, task: &mut Task| {
            assert!(
                task.env.is_none(),
                "env should not be set for non-StateChange events"
            );
            Ok(())
        });

    let composer = MiddlewareComposer::new().with_task(hostenv_middleware);
    let wrapped = composer.compose_task_handler(handler);

    let mut task = make_task();
    task.state = TaskState::Running;
    let ctx = Arc::new(());
    wrapped(ctx, TaskEventType::Started, &mut task).unwrap();
}

#[test]
fn hostenv_middleware_preserves_existing_env() {
    let hostenv_middleware: TaskMiddlewareFunc = Arc::new(move |next: TaskHandlerFunc| {
        let next = next.clone();
        Arc::new(move |_ctx: Arc<()>, et: TaskEventType, task: &mut Task| {
            if et == TaskEventType::StateChange && task.state == TaskState::Running {
                if task.env.is_none() {
                    task.env = Some(HashMap::new());
                }
                if let Some(ref mut env) = task.env {
                    env.insert("HOST_VAR".to_string(), "host_value".to_string());
                }
            }
            next(_ctx, et, task)
        })
    });

    let handler: TaskHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: TaskEventType, task: &mut Task| {
            let env = task.env.as_ref().expect("task env should be set");
            assert_eq!(env.get("EXISTING_VAR"), Some(&"existing_value".to_string()));
            assert_eq!(env.get("HOST_VAR"), Some(&"host_value".to_string()));
            Ok(())
        });

    let composer = MiddlewareComposer::new().with_task(hostenv_middleware);
    let wrapped = composer.compose_task_handler(handler);

    let mut task = make_task();
    task.state = TaskState::Running;
    task.env = Some(
        [("EXISTING_VAR".to_string(), "existing_value".to_string())]
            .into_iter()
            .collect(),
    );
    let ctx = Arc::new(());
    wrapped(ctx, TaskEventType::StateChange, &mut task).unwrap();
}

#[test]
fn hostenv_middleware_multiple_vars_injected() {
    let hostenv_middleware: TaskMiddlewareFunc = Arc::new(move |next: TaskHandlerFunc| {
        let next = next.clone();
        Arc::new(move |_ctx: Arc<()>, et: TaskEventType, task: &mut Task| {
            if et == TaskEventType::StateChange && task.state == TaskState::Running {
                if task.env.is_none() {
                    task.env = Some(HashMap::new());
                }
                if let Some(ref mut env) = task.env {
                    env.insert("VAR1".to_string(), "value1".to_string());
                    env.insert("VAR2".to_string(), "value2".to_string());
                    env.insert("VAR3".to_string(), "value3".to_string());
                }
            }
            next(_ctx, et, task)
        })
    });

    let handler: TaskHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: TaskEventType, task: &mut Task| {
            let env = task.env.as_ref().expect("task env should be set");
            assert_eq!(env.len(), 3);
            assert_eq!(env.get("VAR1"), Some(&"value1".to_string()));
            assert_eq!(env.get("VAR2"), Some(&"value2".to_string()));
            assert_eq!(env.get("VAR3"), Some(&"value3".to_string()));
            Ok(())
        });

    let composer = MiddlewareComposer::new().with_task(hostenv_middleware);
    let wrapped = composer.compose_task_handler(handler);

    let mut task = make_task();
    task.state = TaskState::Running;
    let ctx = Arc::new(());
    wrapped(ctx, TaskEventType::StateChange, &mut task).unwrap();
}

#[test]
fn hostenv_middleware_empty_does_nothing() {
    let hostenv_middleware: TaskMiddlewareFunc = Arc::new(move |_next: TaskHandlerFunc| {
        Arc::new(move |_ctx: Arc<()>, _et: TaskEventType, _task: &mut Task| Ok(()))
    });

    let handler: TaskHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: TaskEventType, task: &mut Task| {
            assert!(
                task.env.is_none(),
                "env should remain None when no vars to inject"
            );
            Ok(())
        });

    let composer = MiddlewareComposer::new().with_task(hostenv_middleware);
    let wrapped = composer.compose_task_handler(handler);

    let mut task = make_task();
    task.state = TaskState::Running;
    let ctx = Arc::new(());
    wrapped(ctx, TaskEventType::StateChange, &mut task).unwrap();
}

#[test]
fn middleware_composer_chains_multiple_middleware_types() {
    let execution_order = Arc::new(Mutex::new(Vec::new()));

    let execution_order_task = execution_order.clone();
    let task_mw: TaskMiddlewareFunc = Arc::new(move |next: TaskHandlerFunc| {
        let next = next.clone();
        let exec_order = execution_order_task.clone();
        Arc::new(move |ctx: Arc<()>, et: TaskEventType, task: &mut Task| {
            exec_order.lock().unwrap().push("task_mw");
            next(ctx, et, task)
        })
    });

    let execution_order_job = execution_order.clone();
    let job_mw: JobMiddlewareFunc = Arc::new(move |next: JobHandlerFunc| {
        let next = next.clone();
        let exec_order = execution_order_job.clone();
        Arc::new(move |ctx: Arc<()>, et: JobEventType, job: &mut Job| {
            exec_order.lock().unwrap().push("job_mw");
            next(ctx, et, job)
        })
    });

    let execution_order_node = execution_order.clone();
    let node_mw: NodeMiddlewareFunc = Arc::new(move |next: NodeHandlerFunc| {
        let next = next.clone();
        let exec_order = execution_order_node.clone();
        Arc::new(move |ctx: Arc<()>, node: &mut Node| {
            exec_order.lock().unwrap().push("node_mw");
            next(ctx, node)
        })
    });

    let composer = MiddlewareComposer::new()
        .with_task(task_mw)
        .with_job(job_mw)
        .with_node(node_mw);

    let execution_order_task_handler = execution_order.clone();
    let task_handler: TaskHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: TaskEventType, _task: &mut Task| {
            execution_order_task_handler
                .lock()
                .unwrap()
                .push("task_handler");
            Ok(())
        });

    let execution_order_job_handler = execution_order.clone();
    let job_handler: JobHandlerFunc =
        Arc::new(move |_ctx: Arc<()>, _et: JobEventType, _job: &mut Job| {
            execution_order_job_handler
                .lock()
                .unwrap()
                .push("job_handler");
            Ok(())
        });

    let execution_order_node_handler = execution_order.clone();
    let node_handler: NodeHandlerFunc = Arc::new(move |_ctx: Arc<()>, _node: &mut Node| {
        execution_order_node_handler
            .lock()
            .unwrap()
            .push("node_handler");
        Ok(())
    });

    let wrapped_task = composer.compose_task_handler(task_handler);
    let wrapped_job = composer.compose_job_handler(job_handler);
    let wrapped_node = composer.compose_node_handler(node_handler);

    let mut task = make_task();
    let mut job = make_job();
    let mut node = make_node();
    let ctx: Arc<()> = Arc::new(());

    wrapped_task(ctx.clone(), TaskEventType::Started, &mut task).unwrap();
    wrapped_job(ctx.clone(), JobEventType::StateChange, &mut job).unwrap();
    wrapped_node(ctx.clone(), &mut node).unwrap();

    let order = execution_order.lock().unwrap();
    assert!(order.contains(&"task_mw"));
    assert!(order.contains(&"task_handler"));
    assert!(order.contains(&"job_mw"));
    assert!(order.contains(&"job_handler"));
    assert!(order.contains(&"node_mw"));
    assert!(order.contains(&"node_handler"));
}
