    async fn subscribe_queues(&self) -> Result<(), CoordinatorError> {
        for (qname, conc) in &self.queues {
            if !is_coordinator_queue(qname) {
                continue;
            }

            for _ in 0..*conc {
                self.subscribe_to_queue(qname).await?;
            }
        }
        Ok(())
    }

    /// Subscribe to a single queue based on its name.
    ///
    /// Go parity: the `switch qname { case ... }` block in `Start()`.
    /// Handlers are wrapped through the middleware chain before subscription.
    async fn subscribe_to_queue(&self, qname: &str) -> Result<(), CoordinatorError> {
        match qname {
            queue::QUEUE_PENDING => {
                let handler = self.pending_handler.clone();
                let base_handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        let mut t = (*task).clone();
                        if let Err(e) = handler.handle(Arc::new(()), &mut t).await {
                            error!(error = %e, "pending handler error");
                        }
                    })
                });
                let task_handler = self.middleware.apply_task(base_handler);
                self.broker
                    .subscribe_for_tasks(qname.to_string(), task_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_STARTED => {
                let handler = self.started_handler.clone();
                let broker = self.broker.clone();
                let ds = self.datastore.clone();
                let error_handler = self.error_handler.clone();
                let base_handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
                    let handler = handler.clone();
                    let broker = broker.clone();
                    let ds = ds.clone();
                    let error_handler = error_handler.clone();
                    Box::pin(async move {
                        match handler.handle(&task).await {
                            Ok(()) => {}
                            Err(e) => {
                                warn!(error = %e, queue = queue::QUEUE_STARTED, "started handler error");
                                Self::publish_task_error(&broker, &ds, &error_handler, &task, &e).await;
                            }
                        }
                    })
                });
                let task_handler = self.middleware.apply_task(base_handler);
                self.broker
                    .subscribe_for_tasks(qname.to_string(), task_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_COMPLETED => {
                let handler = self.completed_handler.clone();
                let broker = self.broker.clone();
                let ds = self.datastore.clone();
                let error_handler = self.error_handler.clone();
                let base_handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
                    let handler = handler.clone();
                    let broker = broker.clone();
                    let ds = ds.clone();
                    let error_handler = error_handler.clone();
                    Box::pin(async move {
                        match handler.handle(&task).await {
                            Ok(()) => {}
                            Err(e) => {
                                warn!(error = %e, queue = queue::QUEUE_COMPLETED, "completed handler error");
                                Self::publish_task_error(&broker, &ds, &error_handler, &task, &e).await;
                            }
                        }
                    })
                });
                let task_handler = self.middleware.apply_task(base_handler);
                self.broker
                    .subscribe_for_tasks(qname.to_string(), task_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_ERROR => {
                let handler = self.error_handler.clone();
                let base_handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        let mut t = (*task).clone();
                        if let Err(e) = handler.handle(Arc::new(()), &mut t) {
                            error!(error = %e, queue = queue::QUEUE_ERROR, "error handler error");
                        }
                    })
                });
                let task_handler = self.middleware.apply_task(base_handler);
                self.broker
                    .subscribe_for_tasks(qname.to_string(), task_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_HEARTBEAT => {
                let handler = self.heartbeat_handler.clone();
                let base_handler: HeartbeatHandler = Arc::new(move |node: Node| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        if let Err(e) = handler.handle(&node).await {
                            error!(error = %e, "heartbeat handler error");
                        }
                    })
                });
                let node_handler = self.middleware.apply_node(base_handler);
                self.broker
                    .subscribe_for_heartbeats(node_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_JOBS => {
                let handler = self.job_handler.clone();
                let broker = self.broker.clone();
                let ds = self.datastore.clone();
                let error_handler = self.error_handler.clone();
                let base_handler: JobHandler = Arc::new(move |job: Job| {
                    let handler = handler.clone();
                    let broker = broker.clone();
                    let ds = ds.clone();
                    let error_handler = error_handler.clone();
                    Box::pin(async move {
                        let mut j = job;
                        match handler.handle(crate::handlers::JobEventType::StateChange, &mut j).await {
                            Ok(()) => {}
                            Err(e) => {
                                warn!(error = %e, queue = queue::QUEUE_JOBS, "job handler error");
                                Self::publish_job_error(&broker, &ds, &error_handler, &j, &e).await;
                            }
                        }
                    })
                });
                let job_handler = self.middleware.apply_job(base_handler);
                self.broker
                    .subscribe_for_jobs(job_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_LOGS => {
                let handler = self.log_handler.clone();
                let base_handler: TaskLogPartHandler = Arc::new(move |part: tork::task::TaskLogPart| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        if let Err(e) = handler.handle(&part).await {
                            error!(error = %e, "log handler error");
                        }
                    })
                });
                let log_handler = self.middleware.apply_log(base_handler);
                self.broker
                    .subscribe_for_task_log_part(log_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_PROGRESS => {
                // Progress handler is not wrapped in task middleware (no middleware defined for progress)
                let handler = self.progress_handler.clone();
                let broker = self.broker.clone();
                let progress_handler: TaskProgressHandler = Arc::new(move |task: Task| {
                    let handler = handler.clone();
                    let broker = broker.clone();
                    Box::pin(async move {
                        match handler.handle(&task).await {
                            Ok(()) => {}
                            Err(e) => {
                                warn!(error = %e, queue = queue::QUEUE_PROGRESS, "progress handler error");
                                let failed = Task {
                                    state: TASK_STATE_FAILED.clone(),
                                    failed_at: Some(OffsetDateTime::now_utc()),
                                    error: Some(e.to_string()),
                                    ..task.clone()
                                };
                                if let Err(pe) = broker.publish_task(queue::QUEUE_ERROR.to_string(), &failed).await {
                                    error!(error = %pe, "error publishing failed task to error queue");
                                }
                            }
                        }
                    })
                });
                self.broker
                    .subscribe_for_task_progress(progress_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            queue::QUEUE_REDELIVERIES => {
                let handler = self.redelivered_handler.clone();
                let base_handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        if let Err(e) = handler.handle(&task).await {
                            error!(error = %e, "redelivered handler error");
                        }
                    })
                });
                let task_handler = self.middleware.apply_task(base_handler);
                self.broker
                    .subscribe_for_tasks(qname.to_string(), task_handler)
                    .await
                    .map_err(|e| CoordinatorError::Broker(format!("subscribe to {qname}: {e}")))?;
            }
            _ => {
                debug!(queue = qname, "skipping non-coordinator queue");
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Private: Scheduled job events
    // -----------------------------------------------------------------------

    /// Subscribe to scheduled job events.
    ///
    /// Go parity: `c.broker.SubscribeForEvents(broker.TOPIC_SCHEDULED_JOB, ...)`
    async fn subscribe_scheduled_jobs(&self) -> Result<(), CoordinatorError> {
        let handler = self.schedule_handler.clone();
        let event_handler: EventHandler = Arc::new(move |ev: serde_json::Value| {
            let handler = handler.clone();
            Box::pin(async move {
                match serde_json::from_value::<ScheduledJob>(ev) {
                    Ok(sj) => {
                        if let Err(e) = handler.handle(&sj).await {
                            error!(error = %e, "error handling scheduled job");
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "error casting scheduled job event");
                    }
                }
            })
        });

        self.broker
            .subscribe_for_events(TOPIC_SCHEDULED_JOB.to_string(), event_handler)
            .await
            .map_err(|e| CoordinatorError::Broker(format!("subscribe to {TOPIC_SCHEDULED_JOB}: {e}")))?;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Private: Heartbeat loop
    // -----------------------------------------------------------------------

    /// Spawn the heartbeat loop in a background task.
    ///
    /// Go parity: `go c.sendHeartbeats()`
    fn spawn_heartbeat(&self) {
        let id = self.id.clone();
        let name = self.name.clone();
        let start_time = self.start_time;
        let broker = self.broker.clone();
        let stop_rx = self.stop_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut rx = stop_rx;

            loop {
                // Get hostname (Go: os.Hostname())
                let hostname = hostname::get()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|_| String::new());

                // Get CPU percent (Go: host.GetCPUPercent())
                let cpu_percent = get_cpu_percent();

                // Build the node heartbeat
                // Go parity:
                //   tork.Node{
                //       ID: c.id, Name: c.Name,
                //       StartedAt: c.startTime, Status: tork.NodeStatusUP,
                //       CPUPercent: cpuPercent, LastHeartbeatAt: now,
                //       Hostname: hostname, Version: tork.Version,
                //   }
                let node = Node {
                    id: Some(id.clone()),
                    name: Some(name.clone()),
                    started_at: start_time,
                    status: NODE_STATUS_UP.to_string(),
                    cpu_percent,
                    last_heartbeat_at: OffsetDateTime::now_utc(),
                    hostname: Some(hostname),
                    version: VERSION.to_string(),
                    port: 0,
                    task_count: 0,
                    queue: None,
                };

                // Publish heartbeat (Go: c.broker.PublishHeartbeat(ctx, &tork.Node{...}))
                if let Err(e) = broker.publish_heartbeat(node).await {
                    error!(error = %e, coordinator_id = %id, "error publishing heartbeat");
                }

                // Wait for next tick or stop signal (Go: select { case <-c.stop; case <-time.After(...) })
                tokio::select! {
                    _ = rx.changed() => {
                        debug!(coordinator_id = %id, "heartbeat loop stopped");
                        return;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(HEARTBEAT_RATE_SECS as u64)) => {}
                }
            }
        });

        // Store the handle for clean shutdown
        let handle_ref = self.heartbeat_handle.clone();
        tokio::spawn(async move {
            let mut lock = handle_ref.lock().await;
            *lock = Some(handle);
        });
    }

    // -----------------------------------------------------------------------
    // Private: Error publishing helpers
    // -----------------------------------------------------------------------

    /// Publish a failed task to the error queue.
    ///
    /// Go parity: `taskHandler` wrapper — when a handler returns an error,
    /// marks the task as FAILED and publishes to QUEUE_ERROR.
    async fn publish_task_error(
        broker: &Arc<dyn Broker>,
        _ds: &Arc<dyn Datastore>,
        error_handler: &Arc<ErrorHandler>,
        task: &Task,
        handler_error: &HandlerError,
    ) {
        let now = OffsetDateTime::now_utc();
        let failed = Task {
            state: TASK_STATE_FAILED.clone(),
            failed_at: Some(now),
            error: Some(handler_error.to_string()),
            ..(*task).clone()
        };

        if let Err(e) = broker.publish_task(queue::QUEUE_ERROR.to_string(), &failed).await {
            error!(error = %e, "error publishing failed task to error queue");
        }

        // Go: also calls onError(ctx, task.StateChange, t) via the error handler
        let mut failed_mut = failed;
        if let Err(e) = error_handler.handle(Arc::new(()), &mut failed_mut) {
            error!(error = %e, "error handler callback failed");
        }
    }

    /// Publish a failed job to the jobs queue.
    ///
    /// Go parity: `jobHandler` wrapper — when a handler returns an error,
    /// marks the job as FAILED and publishes to QUEUE_JOBS.
    async fn publish_job_error(
        broker: &Arc<dyn Broker>,
        _ds: &Arc<dyn Datastore>,
        _error_handler: &Arc<ErrorHandler>,
        job: &Job,
        handler_error: &HandlerError,
    ) {
        let now = OffsetDateTime::now_utc();
        let failed = Job {
            state: JOB_STATE_FAILED.to_string(),
            failed_at: Some(now),
            error: Some(handler_error.to_string()),
            ..(*job).clone()
        };

        if let Err(e) = broker.publish_job(&failed).await {
            error!(error = %e, "error publishing failed job");
        }
    }
}

// ---------------------------------------------------------------------------
// Host system helpers (pure I/O at the boundary)
// ---------------------------------------------------------------------------

/// Gets the current CPU usage percentage.
///
/// Go parity with `host.GetCPUPercent()` — returns 0.0 on error.
#[must_use]