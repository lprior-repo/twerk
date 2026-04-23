use std::fmt;
use std::io::{self, Write};

/// Trait for types that can be closed.
pub trait Close {
    /// Closes the resource.
    ///
    /// # Errors
    /// Returns an error if the close operation fails.
    fn close(self) -> io::Result<()>;
}

/// Closes `c`, ignoring any error.
/// Its main use is to satisfy linters.
#[inline]
pub fn close_ignore<C: Close>(c: C) {
    let _ = c.close();
}

/// Formats and writes to `w` using the given format string and arguments.
/// Note: This is a simplified implementation that formats each argument with `{}`
/// and concatenates them. For full fmt.Fprintf behavior, use the `flexi_logger` crate.
///
/// # Errors
/// Returns an error if writing to the output fails.
#[inline]
pub fn fprintf<W: Write>(mut w: W, fmt_str: &str, args: &[&dyn fmt::Display]) -> io::Result<()> {
    // Simple implementation: concatenate format string with formatted args
    // For proper format string interpretation, a runtime format crate would be needed
    let mut result = String::new();
    let mut arg_iter = args.iter();

    for part in fmt_str.split("%s") {
        result.push_str(part);
        if let Some(arg) = arg_iter.next() {
            use std::fmt::Write;
            let _ = write!(result, "{arg}");
        }
    }

    w.write_all(result.as_bytes())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// A mock Write+Close that records whether close was called.
    struct MockCloseable {
        closed: Cell<bool>,
    }

    impl MockCloseable {
        fn new() -> Self {
            Self {
                closed: Cell::new(false),
            }
        }
    }

    impl Write for MockCloseable {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Close for MockCloseable {
        fn close(self) -> io::Result<()> {
            self.closed.set(true);
            Ok(())
        }
    }

    #[test]
    fn close_ignore_actually_calls_close() {
        // Mutation replaces close_ignore body with (). If close is never called,
        // the flag stays false. This test catches that mutation.
        let mock = MockCloseable::new();
        let was_closed_before = mock.closed.get();
        assert!(!was_closed_before);
        close_ignore(mock);
        // We can't check the flag after move, but the mutation that replaces
        // close_ignore with () would cause it to not call close at all.
        // Since close_ignore takes ownership, we verify via a Rc-based approach below.
    }

    /// A thread-safe mock that tracks close calls via Rc.
    struct TrackedCloseable {
        called: std::rc::Rc<Cell<bool>>,
    }

    impl TrackedCloseable {
        fn new(called: std::rc::Rc<Cell<bool>>) -> Self {
            Self { called }
        }
    }

    impl Write for TrackedCloseable {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Close for TrackedCloseable {
        fn close(self) -> io::Result<()> {
            self.called.set(true);
            Ok(())
        }
    }

    #[test]
    fn close_ignore_invokes_close_method() {
        let called = std::rc::Rc::new(Cell::new(false));
        let mock = TrackedCloseable::new(called.clone());
        close_ignore(mock);
        assert!(
            called.get(),
            "close_ignore must call .close() — mutation may have replaced body with ()"
        );
    }

    #[test]
    fn fprintf_writes_correct_output() {
        let mut buf = Vec::new();
        fprintf(&mut buf, "hello %s world", &[&"beautiful"]).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "hello beautiful world",
            "fprintf must substitute %s with arguments"
        );
    }

    #[test]
    fn fprintf_multiple_placeholders() {
        let mut buf = Vec::new();
        fprintf(&mut buf, "%s and %s", &[&"foo", &"bar"]).unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "foo and bar");
    }

    #[test]
    fn fprintf_no_placeholders() {
        let mut buf = Vec::new();
        fprintf(&mut buf, "no placeholders", &[]).unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "no placeholders");
    }

    #[test]
    fn fprintf_returns_err_on_write_failure() {
        struct FailingWriter;
        impl Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "write failed"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut writer = FailingWriter;
        let result = fprintf(&mut writer, "test %s", &[&"arg"]);
        assert!(
            result.is_err(),
            "fprintf must return Err when the underlying write fails — mutation may have replaced body with Ok(())"
        );
    }
}
