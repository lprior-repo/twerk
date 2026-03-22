//! # FNS Module
//!
//! Utility functions for I/O operations with error ignoring.

use std::io::{self, Write};

/// Closes the given closer by calling its close method, ignoring any error.
/// Its main use is to satisfy linters.
pub fn close_ignore<C: ?Sized>(c: &C)
where
    C: Close,
{
    let _ = c.close();
}

/// Trait for types that can be closed.
///
/// This mirrors the Go io.Closer interface.
pub trait Close {
    /// Close the resource, returning any error.
    fn close(&self) -> io::Result<()>;
}

impl Close for std::fs::File {
    fn close(&self) -> io::Result<()> {
        std::fs::File::sync_all(self)?;
        Ok(())
    }
}

impl Close for std::net::TcpStream {
    fn close(&self) -> io::Result<()> {
        Ok(())
    }
}

/// Writes formatted output to a Writer, ignoring any errors.
///
/// Mirrors Go's fmt.Fprintf with format string support (%s, %d, %v, etc.)
///
/// # Arguments
///
/// * `w` - The writer to write to
/// * `format` - The format string with %s, %d, %v, etc. placeholders
/// * `args` - Arguments to insert into the format string
pub fn fprintf<W: Write>(w: &mut W, format: &str, args: &[&dyn std::fmt::Display]) {
    let result = go_format(format, args);
    let _ = w.write_all(result.as_bytes());
}

/// Formats a string using Go-style format specifiers.
/// Handles: %s, %d, %v, %f, %g, %e, %x, %p, %t, %c, %b, %o, %x, %U
/// Simple implementation that parses format strings and applies arguments.
fn go_format(format: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut result = String::new();
    let mut chars = format.chars().peekable();
    let mut arg_index = 0;

    while let Some(c) = chars.next() {
        if c == '%' {
            if let Some(next) = chars.peek() {
                match next {
                    '%' => {
                        result.push('%');
                        chars.next(); // consume second %
                    }
                    's' => {
                        chars.next(); // consume 's'
                        if arg_index < args.len() {
                            result.push_str(&args[arg_index].to_string());
                            arg_index += 1;
                        }
                    }
                    'd' => {
                        chars.next();
                        if arg_index < args.len() {
                            result.push_str(&args[arg_index].to_string());
                            arg_index += 1;
                        }
                    }
                    'v' => {
                        chars.next();
                        if arg_index < args.len() {
                            result.push_str(&args[arg_index].to_string());
                            arg_index += 1;
                        }
                    }
                    'f' | 'g' | 'e' => {
                        chars.next();
                        if arg_index < args.len() {
                            result.push_str(&args[arg_index].to_string());
                            arg_index += 1;
                        }
                    }
                    'x' | 'X' => {
                        chars.next();
                        if arg_index < args.len() {
                            result.push_str(&args[arg_index].to_string());
                            arg_index += 1;
                        }
                    }
                    'p' => {
                        chars.next();
                        if arg_index < args.len() {
                            result.push_str(&args[arg_index].to_string());
                            arg_index += 1;
                        }
                    }
                    't' => {
                        chars.next();
                        if arg_index < args.len() {
                            result.push_str(&args[arg_index].to_string());
                            arg_index += 1;
                        }
                    }
                    'c' => {
                        chars.next();
                        if arg_index < args.len() {
                            result.push_str(&args[arg_index].to_string());
                            arg_index += 1;
                        }
                    }
                    'b' => {
                        chars.next();
                        if arg_index < args.len() {
                            result.push_str(&args[arg_index].to_string());
                            arg_index += 1;
                        }
                    }
                    'o' => {
                        chars.next();
                        if arg_index < args.len() {
                            result.push_str(&args[arg_index].to_string());
                            arg_index += 1;
                        }
                    }
                    'U' => {
                        chars.next();
                        if arg_index < args.len() {
                            result.push_str(&args[arg_index].to_string());
                            arg_index += 1;
                        }
                    }
                    // Handle width and precision (simplified)
                    '1'..='9' | '.' => {
                        // Skip width/precision digits
                        while let Some(&c) = chars.peek() {
                            if c.is_ascii_digit() || c == '.' {
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        // Now check what specifier follows
                        if let Some(&spec) = chars.peek() {
                            match spec {
                                'f' | 'g' | 'e' | 'd' | 's' | 'v' | 'x' | 'X' | 'p' | 'c' | 'b'
                                | 'o' | 'U' => {
                                    chars.next();
                                    if arg_index < args.len() {
                                        result.push_str(&args[arg_index].to_string());
                                        arg_index += 1;
                                    }
                                }
                                _ => {
                                    result.push('%');
                                }
                            }
                        }
                    }
                    _ => {
                        result.push('%');
                    }
                }
            } else {
                result.push('%');
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct MockCloser {
        closed: bool,
    }

    impl MockCloser {
        fn new() -> Self {
            Self { closed: false }
        }
    }

    impl Close for MockCloser {
        fn close(&self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_close_ignore_does_not_panic() {
        let closer = MockCloser::new();
        close_ignore(&closer);
        // Just verify it doesn't panic
    }

    #[test]
    fn test_fprintf_percent() {
        let mut buf = Cursor::new(Vec::new());
        fprintf(&mut buf, "100%%", &[]);
        assert_eq!(buf.into_inner(), b"100%");
    }

    #[test]
    fn test_fprintf_s() {
        let mut buf = Cursor::new(Vec::new());
        fprintf(&mut buf, "Hello, %s!", &[&"World"]);
        assert_eq!(buf.into_inner(), b"Hello, World!");
    }

    #[test]
    fn test_fprintf_multiple_s() {
        let mut buf = Cursor::new(Vec::new());
        fprintf(&mut buf, "%s %s", &[&"Hello", &"World"]);
        assert_eq!(buf.into_inner(), b"Hello World");
    }

    #[test]
    fn test_fprintf_d() {
        let mut buf = Cursor::new(Vec::new());
        fprintf(&mut buf, "Number: %d", &[&42]);
        assert_eq!(buf.into_inner(), b"Number: 42");
    }

    #[test]
    fn test_fprintf_v() {
        let mut buf = Cursor::new(Vec::new());
        fprintf(&mut buf, "Value: %v", &[&"test"]);
        assert_eq!(buf.into_inner(), b"Value: test");
    }

    #[test]
    fn test_fprintf_empty() {
        let mut buf = Cursor::new(Vec::new());
        fprintf(&mut buf, "", &[]);
        assert!(buf.into_inner().is_empty());
    }

    #[test]
    fn test_fprintf_error_writer() {
        struct ErrorWriter;

        impl Write for ErrorWriter {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::Other, "mock write error"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut writer = ErrorWriter;
        // Should not panic
        fprintf(&mut writer, "Hello, %s!", &[&"World"]);
    }

    #[test]
    fn test_fprintf_multiple_placeholders() {
        let mut buf = Cursor::new(Vec::new());
        fprintf(&mut buf, "%s %s %s", &[&"Hello", &"World", &"!"]);
        assert_eq!(buf.into_inner(), b"Hello World !");
    }

    #[test]
    fn test_fprintf_no_args() {
        let mut buf = Cursor::new(Vec::new());
        fprintf(&mut buf, "Hello World", &[]);
        assert_eq!(buf.into_inner(), b"Hello World");
    }

    #[test]
    fn test_fprintf_mixed() {
        let mut buf = Cursor::new(Vec::new());
        fprintf(&mut buf, "Name: %s, Age: %d", &[&"Alice", &30]);
        assert_eq!(buf.into_inner(), b"Name: Alice, Age: 30");
    }
}
