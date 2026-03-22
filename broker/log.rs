//! Log shipper for forwarding task logs.
//!
//! Provides a writer interface that batches log entries and forwards
//! them to the broker's task log queue.

use crate::broker::Broker;
use tork::task::TaskLogPart;
use std::io;
use std::sync::Arc;

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
    #[allow(unused_imports)]
    use super::*;

    // Note: These tests would require a mock broker
    // They are placeholders for the test structure

    #[test]
    fn test_log_shipper_creation() {
        // This would need a mock broker
        // Skipping actual implementation
    }
}
