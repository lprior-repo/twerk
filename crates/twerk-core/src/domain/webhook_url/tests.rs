#![allow(unexpected_cfgs)]
#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::domain::testing::arb_valid_webhook_url;
    use crate::WebhookUrl;
    use crate::WebhookUrlError;

    // -------------------------------------------------------------------------
    // Behavior: WebhookUrl constructs successfully when given valid https URL
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_new_returns_ok_when_given_valid_https_url() {
        let result = WebhookUrl::new("https://example.com:8080/webhook");
        assert!(result.is_ok());
        let url = result.unwrap();
        assert_eq!(url.as_str(), "https://example.com:8080/webhook");
        assert_eq!(url.as_url().unwrap().scheme(), "https");
        assert_eq!(url.as_url().unwrap().host_str(), Some("example.com"));
        assert_eq!(url.as_url().unwrap().port(), Some(8080));
        assert_eq!(url.as_url().unwrap().path(), "/webhook");
    }

    // -------------------------------------------------------------------------
    // Behavior: WebhookUrl constructs successfully when given valid http URL
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_new_returns_ok_when_given_valid_http_url() {
        let result = WebhookUrl::new("http://localhost:3000/");
        assert!(result.is_ok());
        let url = result.unwrap();
        assert_eq!(url.as_str(), "http://localhost:3000/");
        assert_eq!(url.as_url().unwrap().scheme(), "http");
        assert_eq!(url.as_url().unwrap().host_str(), Some("localhost"));
        assert_eq!(url.as_url().unwrap().port(), Some(3000));
    }

    // -------------------------------------------------------------------------
    // Behavior: WebhookUrl returns error when input fails URL parsing
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_new_returns_url_parse_error_when_input_is_invalid() {
        let result = WebhookUrl::new("not a url");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, WebhookUrlError::UrlParseError(_)));
        // In RED phase, the actual error variant extraction is unreachable
        // because new() returns todo!(). But we structure it properly for when
        // implementation exists.
        if let WebhookUrlError::UrlParseError(s) = e {
            assert!(!s.is_empty());
        }
    }

    // -------------------------------------------------------------------------
    // Behavior: WebhookUrl returns error when scheme is not http or https
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_new_returns_invalid_scheme_error_when_scheme_is_ftp() {
        let result = WebhookUrl::new("ftp://example.com/file");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, WebhookUrlError::InvalidScheme(_)));
        if let WebhookUrlError::InvalidScheme(scheme) = e {
            assert_eq!(scheme, "ftp");
        }
    }

    #[test]
    fn webhook_url_new_returns_invalid_scheme_error_when_scheme_is_file() {
        let result = WebhookUrl::new("file:///path/to/file");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, WebhookUrlError::InvalidScheme(_)));
        if let WebhookUrlError::InvalidScheme(s) = e {
            assert_eq!(s, "file");
        }
    }

    #[test]
    fn webhook_url_new_returns_invalid_scheme_error_when_scheme_is_ws() {
        let result = WebhookUrl::new("ws://example.com/socket");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, WebhookUrlError::InvalidScheme(_)));
        if let WebhookUrlError::InvalidScheme(s) = e {
            assert_eq!(s, "ws");
        }
    }

    #[test]
    fn webhook_url_new_returns_invalid_scheme_error_when_scheme_is_wss() {
        let result = WebhookUrl::new("wss://secure.example.com/socket");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, WebhookUrlError::InvalidScheme(_)));
        if let WebhookUrlError::InvalidScheme(s) = e {
            assert_eq!(s, "wss");
        }
    }

    // -------------------------------------------------------------------------
    // Behavior: WebhookUrl returns error when URL has no host component
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_new_returns_url_parse_error_when_host_is_empty() {
        // "http://" is rejected by the url crate with "empty host" because
        // it cannot parse URLs with missing authority sections
        let result = WebhookUrl::new("http://");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, WebhookUrlError::UrlParseError(_)));
        if let WebhookUrlError::UrlParseError(s) = &e {
            assert!(s.contains("empty host"));
        }
    }

    #[test]
    fn webhook_url_new_returns_invalid_scheme_error_when_url_has_no_authority() {
        // "file:///path/only" has scheme "file" which is not http/https,
        // so scheme validation fails before we can check for missing host
        let result = WebhookUrl::new("file:///path/only");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, WebhookUrlError::InvalidScheme(_)));
        if let WebhookUrlError::InvalidScheme(s) = &e {
            assert_eq!(s, "file");
        }
    }

    // -------------------------------------------------------------------------
    // Behavior: WebhookUrl returns original string when as_str is called
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_as_str_returns_original_input_exactly() {
        let input = "https://example.com:443/path?query=1#fragment";
        let url = WebhookUrl::new(input).unwrap();
        assert_eq!(url.as_str(), input);
    }

    // -------------------------------------------------------------------------
    // Behavior: WebhookUrl returns parsed URL when as_url is called
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_as_url_returns_parsed_url_components() {
        let url = WebhookUrl::new("https://api.example.com:9090/v1/users?id=42").unwrap();
        let parsed = url.as_url().unwrap();
        assert_eq!(parsed.scheme(), "https");
        assert_eq!(parsed.host_str(), Some("api.example.com"));
        assert_eq!(parsed.port(), Some(9090));
        assert_eq!(parsed.path(), "/v1/users");
        assert_eq!(parsed.query(), Some("id=42"));
    }

    // -------------------------------------------------------------------------
    // Behavior: WebhookUrl invariant: as_str always returns non-empty string
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_as_str_never_returns_empty_string() {
        let url = WebhookUrl::new("https://example.com/").unwrap();
        assert!(!url.as_str().is_empty());
    }

    // -------------------------------------------------------------------------
    // Behavior: WebhookUrl invariant: scheme always http or https
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_as_url_scheme_is_always_http_or_https() {
        let url = WebhookUrl::new("https://example.com/").unwrap();
        let parsed = url.as_url().unwrap();
        let scheme = parsed.scheme();
        assert!(scheme == "http" || scheme == "https");
    }

    // -------------------------------------------------------------------------
    // Behavior: WebhookUrl invariant: host always Some
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_as_url_host_is_always_some() {
        let url = WebhookUrl::new("https://example.com/").unwrap();
        assert!(url.as_url().unwrap().host().is_some());
    }

    // -------------------------------------------------------------------------
    // Additional boundary tests
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_new_returns_ok_when_given_minimal_valid_url() {
        // Minimal valid URL with just scheme and host
        let result = WebhookUrl::new("https://a.b");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "https://a.b");
    }

    #[test]
    fn webhook_url_new_returns_ok_when_path_is_root() {
        let url = WebhookUrl::new("https://example.com/").unwrap();
        assert_eq!(url.as_url().unwrap().path(), "/");
    }

    // -------------------------------------------------------------------------
    // Behavior: WebhookUrl returns error when URL exceeds 2048 characters
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_new_returns_url_too_long_error_when_input_is_2049_chars() {
        // 2049 character URL (one over the 2048 limit)
        // "https://example.com/" = 20 chars, so path = 2049 - 20 = 2029
        let long_url = format!("https://example.com/{}", "a".repeat(2029));
        assert_eq!(long_url.len(), 2049);
        let result = WebhookUrl::new(long_url);
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, WebhookUrlError::UrlTooLong));
    }

    #[test]
    fn webhook_url_new_returns_ok_when_input_is_exactly_2048_chars() {
        // Exactly 2048 character URL (at the limit)
        // "https://example.com/" = 20 chars, so path = 2048 - 20 = 2028
        let max_url = format!("https://example.com/{}", "a".repeat(2028));
        assert_eq!(max_url.len(), 2048);
        let result = WebhookUrl::new(max_url);
        assert!(result.is_ok());
    }

    // -------------------------------------------------------------------------
    // Behavior: WebhookUrl Display trait returns same as as_str
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_display_returns_same_as_as_str() {
        let url = WebhookUrl::new("https://example.com:8080/webhook").unwrap();
        assert_eq!(format!("{}", url), url.as_str());
        assert_eq!(format!("{}", url), "https://example.com:8080/webhook");
    }

    // -------------------------------------------------------------------------
    // Proptest invariants
    // -------------------------------------------------------------------------

    mod proptest_inner {
        use super::*;
        use proptest::prelude::*;
        use proptest::proptest;

        proptest! {
            #[test]
            fn webhook_url_new_preserves_input_valid_urls(url in arb_valid_webhook_url()) {
                let result = WebhookUrl::new(url);
                prop_assert!(result.is_ok());
                let url_obj = result.unwrap();
                prop_assert_eq!(url_obj.as_str(), url);
            }

            #[test]
            fn webhook_url_url_components_are_always_valid(url in arb_valid_webhook_url()) {
                let result = WebhookUrl::new(url);
                prop_assert!(result.is_ok());
                let url_obj = result.unwrap();
                let parsed = url_obj.as_url().unwrap();
                prop_assert!(parsed.scheme() == "http" || parsed.scheme() == "https");
                prop_assert!(parsed.host().is_some());
            }

            #[test]
            fn webhook_url_display_matches_as_str(url in arb_valid_webhook_url()) {
                let url_obj = WebhookUrl::new(url).unwrap();
                prop_assert_eq!(format!("{}", url_obj), url_obj.as_str());
            }

            #[test]
            fn webhook_url_is_send_and_sync(url in arb_valid_webhook_url()) {
                let url_obj = WebhookUrl::new(url).unwrap();
                fn assert_send<T: Send>(_: &T) {}
                fn assert_sync<T: Sync>(_: &T) {}
                assert_send(&url_obj);
                assert_sync(&url_obj);
            }
        }
    }

    // -------------------------------------------------------------------------
    // Kani harnesses
    // -------------------------------------------------------------------------

    #[cfg(kani)]
    mod kani {
        use super::*;

        #[kani::proof]
        fn verify_webhook_url_as_str_never_empty() {
            // This is a stub - actual verification requires implementing the type
            // Kani would verify: for any WebhookUrl created via new(), as_str().is_empty() == false
            let input = kani::any::<String>();
            kani::assume(!input.is_empty() && input.len() <= 2048);
            // In real implementation, this would verify the invariant
        }

        #[kani::proof]
        fn verify_webhook_url_scheme_and_host_invariants() {
            // This is a stub - actual verification requires implementing the type
            // Kani would verify: scheme is in {"http", "https"} and host is Some
        }
    }
}
