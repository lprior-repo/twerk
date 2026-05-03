//! XSS sanitization for user-provided text fields.
//!
//! Escapes HTML special characters to prevent stored XSS attacks.

/// Escape HTML special characters in text input.
///
/// Replaces:
/// - `&` with `&amp;`
/// - `<` with `&lt;`
/// - `>` with `&gt;`
/// - `"` with `&quot;`
/// - `'` with `&#x27;`
pub fn sanitize_text(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#x27;"),
            _ => output.push(ch),
        }
    }
    output
}

/// Sanitize an optional string field.
pub fn sanitize_option(input: Option<String>) -> Option<String> {
    input.map(|s| sanitize_text(&s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_text_basic() {
        assert_eq!(sanitize_text("hello"), "hello");
    }

    #[test]
    fn test_sanitize_text_html_tags() {
        assert_eq!(
            sanitize_text("<script>alert('xss')</script>"),
            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
        );
    }

    #[test]
    fn test_sanitize_text_ampersand() {
        assert_eq!(sanitize_text("a & b"), "a &amp; b");
    }

    #[test]
    fn test_sanitize_text_quotes() {
        assert_eq!(
            sanitize_text("\"quoted\" and 'single'"),
            "&quot;quoted&quot; and &#x27;single&#x27;"
        );
    }

    #[test]
    fn test_sanitize_option_some() {
        assert_eq!(
            sanitize_option(Some("<b>test</b>".to_string())),
            Some("&lt;b&gt;test&lt;/b&gt;".to_string())
        );
    }

    #[test]
    fn test_sanitize_option_none() {
        assert_eq!(sanitize_option(None), None);
    }
}
