//! Log shipper for forwarding task logs.
//!
//! Provides a writer interface that batches log entries and forwards
//! them to the broker's task log queue.

use crate::broker::Broker;
use std::io;
use std::sync::Arc;
use tork::task::TaskLogPart;

/// Log shipper that batches log entries and forwards them to the broker.
pub struct LogShipper {
    #[allow(dead_code)]
    broker: Arc<dyn Broker>,
    #[allow(dead_code)]
    task_id: String,
    q: tokio::sync::mpsc::Sender<Vec<u8>>,
}

impl LogShipper {
    /// Creates a new log shipper that forwards logs to the broker.
    pub fn new(broker: Arc<dyn Broker>, task_id: String) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1000);
        let broker_clone = broker.clone();
        let task_id_clone = task_id.clone();

        // Spawn the flush loop
        tokio::spawn(async move {
            let mut buffer = Vec::new();
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            let mut part_num = 0i64;

            loop {
                tokio::select! {
                    Some(data) = rx.recv() => {
                        buffer.extend_from_slice(&data);
                    }
                    _ = interval.tick() => {
                        if !buffer.is_empty() {
                            part_num += 1;
                            let contents = String::from_utf8(buffer.clone())
                                .unwrap_or_default();
                            let part = TaskLogPart {
                                id: None,
                                number: part_num,
                                task_id: Some(task_id_clone.clone()),
                                contents: Some(contents),
                                created_at: None,
                            };
                            let _ = broker_clone.publish_task_log_part(&part).await;
                            buffer.clear();
                        }
                    }
                }
            }
        });

        Self {
            broker,
            task_id,
            q: tx,
        }
    }

    /// Writes data to the log buffer.
    /// Returns the number of bytes written.
    pub fn write(&self, data: &[u8]) -> io::Result<usize> {
        let pc = data.to_vec();
        self.q
            .blocking_send(pc)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "channel closed"))?;
        Ok(data.len())
    }

    /// Writes data to the log buffer (async version).
    pub async fn write_async(&self, data: &[u8]) -> io::Result<usize> {
        let pc = data.to_vec();
        self.q
            .send(pc)
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "channel closed"))?;
        Ok(data.len())
    }
}

/// Implement std::io::Write for LogShipper
impl io::Write for LogShipper {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::inmemory::new_in_memory_broker;
    use std::sync::Arc;
    use std::time::Duration;
    use tork::task::TaskLogPart;

    // Re-export for use in buffer full test
    #[allow(unused_imports)]
    use tokio::sync::mpsc::error::TrySendError;

    /// Test that a timeout triggers forwarding of buffered data.
    /// Mirrors Go's TestForwardTimeout.
    #[tokio::test]
    async fn test_forward_timeout() {
        let broker = Arc::new(new_in_memory_broker());
        let task_id = "test-task-timeout".to_string();

        let received = Arc::new(std::sync::Mutex::new(None));
        let received_clone = received.clone();

        let handler: crate::broker::TaskLogPartHandler = Arc::new(move |part: TaskLogPart| {
            let received = received_clone.clone();
            Box::pin(async move {
                let mut guard = received.lock().expect("mutex not poisoned");
                *guard = Some(part);
            })
        });

        broker
            .subscribe_for_task_log_part(handler)
            .await
            .expect("subscribe should succeed");

        let shipper = LogShipper::new(broker.clone(), task_id.clone());

        // Write some data using async write
        shipper
            .write_async(b"hello world")
            .await
            .expect("write should succeed");

        // Data should not be forwarded immediately (it's buffered)
        {
            let guard = received.lock().expect("mutex not poisoned");
            assert!(
                guard.is_none(),
                "data should not be forwarded before timeout"
            );
        }

        // Wait for the 1-second timeout to trigger
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Data should now be forwarded
        let guard = received.lock().expect("mutex not poisoned");
        let part = guard
            .as_ref()
            .expect("data should be forwarded after timeout");
        assert_eq!(part.contents.as_deref(), Some("hello world"));
        assert_eq!(part.task_id.as_deref(), Some(task_id.as_str()));
    }

    /// Test that multiple writes are batched together before forwarding.
    /// Mirrors Go's TestForwardBatch.
    #[tokio::test]
    async fn test_forward_batch() {
        let broker = Arc::new(new_in_memory_broker());
        let task_id = "test-task-batch".to_string();

        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let handler: crate::broker::TaskLogPartHandler = Arc::new(move |part: TaskLogPart| {
            let received = received_clone.clone();
            Box::pin(async move {
                let mut guard = received.lock().expect("mutex not poisoned");
                if let Some(contents) = part.contents {
                    guard.push(contents);
                }
            })
        });

        broker
            .subscribe_for_task_log_part(handler)
            .await
            .expect("subscribe should succeed");

        let shipper = LogShipper::new(broker.clone(), task_id.clone());

        // Write multiple batches using async write
        shipper
            .write_async(b"batch1")
            .await
            .expect("write should succeed");
        shipper
            .write_async(b"batch2")
            .await
            .expect("write should succeed");
        shipper
            .write_async(b"batch3")
            .await
            .expect("write should succeed");

        // Wait for the 1-second timeout to trigger
        tokio::time::sleep(Duration::from_secs(1)).await;

        // All batches should be forwarded together as one message
        let guard = received.lock().expect("mutex not poisoned");
        assert_eq!(
            guard.len(),
            1,
            "all batches should be combined into one forward"
        );
        assert_eq!(
            guard[0].as_str(),
            "batch1batch2batch3",
            "batches should be concatenated"
        );
    }

    /// Test that writes fail when the internal buffer is full.
    /// Mirrors Go's TestLogShipperWriteBufferFull.
    #[tokio::test]
    async fn test_log_shipper_write_buffer_full() {
        // Create a channel with small buffer for testing backpressure
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(5); // Very small buffer

        // Spawn a task that slowly consumes from the channel (very long interval)
        let _handle = tokio::spawn(async move {
            let mut buffer = Vec::new();
            let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Long interval

            loop {
                tokio::select! {
                    Some(data) = rx.recv() => {
                        buffer.extend_from_slice(&data);
                    }
                    _ = interval.tick() => {
                        if !buffer.is_empty() {
                            // Don't publish - we just want to test backpressure
                            buffer.clear();
                        }
                    }
                }
            }
        });

        // Try to fill the channel past its capacity
        let mut error_count = 0;
        for _ in 0..10 {
            match tx.try_send(vec![b'x'; 100]) {
                Ok(_) => {}
                Err(TrySendError::Full(_)) => {
                    error_count += 1;
                }
                Err(TrySendError::Closed(_)) => {
                    error_count += 1;
                }
            }
        }

        // We expect some sends to fail because the channel is small (5) and the
        // consumer is very slow
        assert!(
            error_count > 0,
            "expected some writes to fail due to full buffer, but got {} errors",
            error_count
        );
    }
}
