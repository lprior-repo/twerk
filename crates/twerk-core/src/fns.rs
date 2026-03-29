use std::fmt;
use std::io::{self, Write};

/// Trait for types that can be closed.
pub trait Close {
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
            let _ = write!(result, "{}", arg);
        }
    }

    w.write_all(result.as_bytes())
}
