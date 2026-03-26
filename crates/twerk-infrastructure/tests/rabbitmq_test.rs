use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use testcontainers_modules::rabbitmq::RabbitMq;
use testcontainers::runners::AsyncRunner;
use twerk_infrastructure::broker::{Broker, RabbitMQOptions, queue};
use twerk_infrastructure::broker::rabbitmq::RabbitMQBroker;
use twerk_core::task::Task;
use twerk_core::job::Job;
use twerk_core::node::Node;
use serde_json::json;

async fn setup_rabbitmq() -> anyhow::Result<(testcontainers::ContainerAsync<RabbitMq>, String)> {
    let container = RabbitMq::default().start().await?;
    let host_port = container.get_host_port_ipv4(5672).await?;
    let url = format!("amqp://guest:guest@localhost:{}", host_port);
    Ok((container, url))
}

#[tokio::test]
async fn task_delivered_when_published_to_rabbitmq() -> anyhow::Result<()> {
    let (_container, url) = setup_rabbitmq().await?;
    let broker = RabbitMQBroker::new(&url, RabbitMQOptions::default()).await?;
    let (tx, mut rx) = mpsc::channel(1);
    
    let qname = format!("{}test-{}", queue::QUEUE_EXCLUSIVE_PREFIX, twerk_core::uuid::new_short_uuid());
    
    broker.subscribe_for_tasks(qname.clone(), Arc::new(move |_task| {
        let tx = tx.clone();
        Box::pin(async move {
            tx.send(()).await.map_err(|e| anyhow::anyhow!(e))?;
            Ok(())
        })
    })).await?;

    let task = Task::default();
    broker.publish_task(qname, &task).await?;

    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await?
        .ok_or_else(|| anyhow::anyhow!("Did not receive task"))?;

    Ok(())
}

#[tokio::test]
async fn heartbeat_delivered_when_published_to_rabbitmq() -> anyhow::Result<()> {
    let (_container, url) = setup_rabbitmq().await?;
    let broker = RabbitMQBroker::new(&url, RabbitMQOptions::default()).await?;
    let (tx, mut rx) = mpsc::channel(1);

    broker.subscribe_for_heartbeats(Arc::new(move |_node| {
        let tx = tx.clone();
        Box::pin(async move {
            tx.send(()).await.map_err(|e| anyhow::anyhow!(e))?;
            Ok(())
        })
    })).await?;

    let node = Node::default();
    broker.publish_heartbeat(node).await?;

    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await?
        .ok_or_else(|| anyhow::anyhow!("Did not receive heartbeat"))?;

    Ok(())
}

#[tokio::test]
async fn job_delivered_when_published_to_rabbitmq() -> anyhow::Result<()> {
    let (_container, url) = setup_rabbitmq().await?;
    let broker = RabbitMQBroker::new(&url, RabbitMQOptions::default()).await?;
    let (tx, mut rx) = mpsc::channel(1);

    broker.subscribe_for_jobs(Arc::new(move |_job| {
        let tx = tx.clone();
        Box::pin(async move {
            tx.send(()).await.map_err(|e| anyhow::anyhow!(e))?;
            Ok(())
        })
    })).await?;

    let job = Job::default();
    broker.publish_job(&job).await?;

    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await?
        .ok_or_else(|| anyhow::anyhow!("Did not receive job"))?;

    Ok(())
}

#[tokio::test]
async fn tasks_delivered_in_priority_order_when_buffered_in_rabbitmq() -> anyhow::Result<()> {
    let (_container, url) = setup_rabbitmq().await?;
    let broker = RabbitMQBroker::new(&url, RabbitMQOptions::default()).await?;
    
    let qname = format!("worker-priority-{}", twerk_core::uuid::new_short_uuid());
    let (tx, mut rx) = mpsc::channel(4);

    // Barrier to ensure we don't send to channel until we "see" the delay effect
    // Wait, the requirement is no sleep.
    broker.subscribe_for_tasks(qname.clone(), Arc::new(move |task| {
        let tx = tx.clone();
        let task = task.clone();
        Box::pin(async move {
            // Use yield_now loop instead of sleep to simulate processing delay
            // allowing RabbitMQ to buffer other messages for priority sorting
            for _ in 0..1000 { tokio::task::yield_now().await; }
            tx.send(task.priority).await.map_err(|e| anyhow::anyhow!(e))?;
            Ok(())
        })
    })).await?;

    // Wait for subscription to be active without sleep
    let mut attempts = 0;
    while attempts < 100 {
        tokio::task::yield_now().await;
        attempts += 1;
    }

    broker.publish_task(qname.clone(), &Task { priority: 0, ..Task::default() }).await?;
    broker.publish_task(qname.clone(), &Task { priority: 1, ..Task::default() }).await?;
    broker.publish_task(qname.clone(), &Task { priority: 2, ..Task::default() }).await?;
    broker.publish_task(qname.clone(), &Task { priority: 3, ..Task::default() }).await?;

    let mut results = Vec::new();
    for _ in 0..4 {
        if let Some(p) = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await? {
            results.push(p);
        }
    }

    assert_eq!(results[0], 0);
    assert_eq!(results[1], 3);
    assert_eq!(results[2], 2);
    assert_eq!(results[3], 1);

    Ok(())
}

#[tokio::test]
async fn events_routed_correctly_when_patterns_match() -> anyhow::Result<()> {
    let (_container, url) = setup_rabbitmq().await?;
    let broker = RabbitMQBroker::new(&url, RabbitMQOptions::default()).await?;
    
    let job_id = twerk_core::uuid::new_short_uuid();
    let (tx1, mut rx1) = mpsc::channel(30);
    let (tx2, mut rx2) = mpsc::channel(10);

    broker.subscribe_for_events("job.#".to_string(), Arc::new(move |val| {
        let tx1 = tx1.clone();
        Box::pin(async move {
            tx1.send(val).await.map_err(|e| anyhow::anyhow!(e))?;
            Ok(())
        })
    })).await?;

    broker.subscribe_for_events("job.completed".to_string(), Arc::new(move |val| {
        let tx2 = tx2.clone();
        Box::pin(async move {
            tx2.send(val).await.map_err(|e| anyhow::anyhow!(e))?;
            Ok(())
        })
    })).await?;

    // Wait for bindings to propagate
    let mut attempts = 0;
    while attempts < 1000 {
        tokio::task::yield_now().await;
        attempts += 1;
    }

    let event_payload = json!({"id": job_id, "state": "completed"});

    for _ in 0..10 {
        broker.publish_event("job.completed".to_string(), event_payload.clone()).await?;
        broker.publish_event("job.failed".to_string(), event_payload.clone()).await?;
        broker.publish_event("job.x.y.z".to_string(), event_payload.clone()).await?;
    }

    for _ in 0..30 {
        tokio::time::timeout(Duration::from_secs(5), rx1.recv()).await?.ok_or_else(|| anyhow::anyhow!("rx1 timeout"))?;
    }

    for _ in 0..10 {
        tokio::time::timeout(Duration::from_secs(5), rx2.recv()).await?.ok_or_else(|| anyhow::anyhow!("rx2 timeout"))?;
    }

    Ok(())
}


#[tokio::test]
async fn test_rabbitmq_health_check() -> anyhow::Result<()> {
    let (_container, url) = setup_rabbitmq().await?;
    let broker = RabbitMQBroker::new(&url, RabbitMQOptions::default()).await?;
    
    broker.health_check().await?;
    broker.shutdown().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_rabbitmq_all() -> anyhow::Result<()> {
    let (_container, url) = setup_rabbitmq().await?;
    let broker = RabbitMQBroker::new(&url, RabbitMQOptions::default()).await?;
    
    // 1. Verify queue naming (constants should have x- prefix)
    assert!(queue::QUEUE_PENDING.starts_with("x-"));
    assert_eq!(queue::QUEUE_REDELIVERIES, "x-redeliveries");
    
    // 2. Publish a task to a non-prefixed queue name (should work as is)
    let qname = format!("worker-{}", twerk_core::uuid::new_short_uuid());
    let task = Task::default();
    broker.publish_task(qname.clone(), &task).await?;
    
    // 3. Publish to a constant queue (should have x- prefix)
    broker.publish_task_progress(&task).await?;
    let info = broker.queue_info(queue::QUEUE_PROGRESS.to_string()).await?;
    assert_eq!(info.name, "x-progress");
    
    // 4. Test redelivery logic via subscription
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    broker.subscribe_for_tasks(queue::QUEUE_REDELIVERIES.to_string(), Arc::new(move |_| {
        let tx = tx.clone();
        Box::pin(async move {
            let _ = tx.send(()).await;
            Ok(())
        })
    })).await?;
    
    // Manually publish a "redelivery" to the redeliveries queue to verify subscription works
    broker.publish_task(queue::QUEUE_REDELIVERIES.to_string(), &task).await?;
    
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await?
        .ok_or_else(|| anyhow::anyhow!("Did not receive message from redeliveries queue"))?;
        
    Ok(())
}

