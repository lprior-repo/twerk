#[cfg(test)]
mod tests {
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
        assert_eq!(url.as_url().scheme(), "https");
        assert_eq!(url.as_url().host_str(), Some("example.com"));
        assert_eq!(url.as_url().port(), Some(8080));
        assert_eq!(url.as_url().path(), "/webhook");
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
        assert_eq!(url.as_url().scheme(), "http");
        assert_eq!(url.as_url().host_str(), Some("localhost"));
        assert_eq!(url.as_url().port(), Some(3000));
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
    fn webhook_url_new_returns_missing_host_error_when_host_is_empty() {
        let result = WebhookUrl::new("http://");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, WebhookUrlError::MissingHost));
    }

    #[test]
    fn webhook_url_new_returns_missing_host_error_when_url_has_no_authority() {
        let result = WebhookUrl::new("file:///path/only");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, WebhookUrlError::MissingHost));
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
        let parsed = url.as_url();
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
        let parsed = url.as_url();
        let scheme = parsed.scheme();
        assert!(scheme == "http" || scheme == "https");
    }

    // -------------------------------------------------------------------------
    // Behavior: WebhookUrl invariant: host always Some
    // -------------------------------------------------------------------------

    #[test]
    fn webhook_url_as_url_host_is_always_some() {
        let url = WebhookUrl::new("https://example.com/").unwrap();
        assert!(url.as_url().host().is_some());
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
        assert_eq!(url.as_url().path(), "/");
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
            fn webhook_url_new_preserves_input_valid_urls(url in prop_oneof![
                "https://example.com",
                "http://localhost:8080",
                "https://api.test.co:443/v1",
                "https://example.com:8443/path"
            ].prop_map(|s| s.to_string())) {
                let result = WebhookUrl::new(&url);
                prop_assert!(result.is_ok());
                let url_obj = result.unwrap();
                prop_assert_eq!(url_obj.as_str(), url);
            }

            #[test]
            fn webhook_url_url_components_are_always_valid(url in "https://[a-z0-9.-]+(:[0-9]+)?(/[a-z0-9/-]*)?") {
                let result = WebhookUrl::new(url);
                prop_assert!(result.is_ok());
                let url_obj = result.unwrap();
                let parsed = url_obj.as_url();
                prop_assert!(parsed.scheme() == "http" || parsed.scheme() == "https");
                prop_assert!(parsed.host().is_some());
            }

            #[test]
            fn webhook_url_display_matches_as_str(url in "https://[a-z0-9.-]+(:[0-9]+)?(/[a-z0-9/-]*)?") {
                let url_obj = WebhookUrl::new(url).unwrap();
                prop_assert_eq!(format!("{}", url_obj), url_obj.as_str());
            }

            #[test]
            fn webhook_url_is_send_and_sync(url in "https://[a-z0-9.-]+(:[0-9]+)?(/[a-z0-9/-]*)?") {
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
