//! Writer that adapts `io::Write` to tracing
//!
//! - **Data**: `TracingWriter` struct holds the level
//! - **Calculations**: None (mostly I/O)
//! - **Actions**: `write` and `flush` implementations

use std::io;

/// A writer that logs each write at a specified level.
///
/// This is useful for capturing output from processes and logging
/// it with the task ID context.
#[derive(Debug)]
pub struct TracingWriter {
    task_id: String,
    level: Level,
}

#[derive(Debug, Clone, Copy)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<Level> for tracing::Level {
    fn from(level: Level) -> Self {
        match level {
            Level::Trace => tracing::Level::TRACE,
            Level::Debug => tracing::Level::DEBUG,
            Level::Info => tracing::Level::INFO,
            Level::Warn => tracing::Level::WARN,
            Level::Error => tracing::Level::ERROR,
        }
    }
}

impl TracingWriter {
    /// Create a new `TracingWriter`.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID to include in log entries
    /// * `level` - The log level to use
    #[must_use]
    pub fn new(task_id: String, level: Level) -> Self {
        Self { task_id, level }
    }

    /// Write a log entry.
    ///
    /// The entire contents are logged as a single log line.
    pub fn write(&self, contents: &str) {
        let line = contents.trim_end();
        if line.is_empty() {
            return;
        }

        let span = tracing::info_span!("task_log", task_id = %self.task_id);
        let _guard = span.enter();

        match self.level {
            Level::Trace => tracing::trace!(line),
            Level::Debug => tracing::debug!(line),
            Level::Info => tracing::info!(line),
            Level::Warn => tracing::warn!(line),
            Level::Error => tracing::error!(line),
        }
    }
}

impl io::Write for TracingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Convert bytes to string, ignoring invalid UTF-8
        let contents = String::from_utf8_lossy(buf);
        TracingWriter::write(self, &contents);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_writer_write() {
        let writer = TracingWriter::new("task-123".to_string(), Level::Info);

        // Should not panic when writing
        writer.write("test log line\n");
        writer.write("");
    }

    #[test]
    fn test_tracing_writer_as_trait() {
        use std::io::Write;

        let mut writer = TracingWriter::new("task-456".to_string(), Level::Debug);

        // Should implement Write trait
        let result = writer.write_all(b"test output\n");
        assert!(result.is_ok());

        let result = writer.flush();
        assert!(result.is_ok());
    }
}
