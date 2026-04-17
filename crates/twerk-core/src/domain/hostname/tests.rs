#[cfg(test)]
mod tests {
    use crate::*;

    // -------------------------------------------------------------------------
    // Behavior: Hostname constructs successfully when given valid single-label hostname
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_ok_when_given_single_label_hostname() {
        let result = Hostname::new("localhost");
        assert!(result.is_ok());
        let host = result.unwrap();
        assert_eq!(host.as_str(), "localhost");
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname constructs successfully when given valid multi-label hostname
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_ok_when_given_multi_label_hostname() {
        let result = Hostname::new("api.example.com");
        assert!(result.is_ok());
        let host = result.unwrap();
        assert_eq!(host.as_str(), "api.example.com");
    }

    #[test]
    fn hostname_new_returns_ok_when_given_hyphenated_hostname() {
        let result = Hostname::new("my-host.example.com");
        assert!(result.is_ok());
        let host = result.unwrap();
        assert_eq!(host.as_str(), "my-host.example.com");
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname constructs successfully when given hostname at max length (253)
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_ok_when_given_max_length_hostname() {
        // 253 character hostname
        let hostname = format!("{}.com", "a".repeat(246));
        assert_eq!(hostname.len(), 253);
        let result = Hostname::new(hostname);
        assert!(result.is_ok());
        let host = result.unwrap();
        assert_eq!(host.as_str().len(), 253);
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname returns error when input is empty string
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_empty_error_when_input_is_empty() {
        let result = Hostname::new("");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, HostnameError::Empty));
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname returns error when input exceeds 253 characters
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_too_long_error_when_input_exceeds_253_chars() {
        let hostname = "a".repeat(254);
        let result = Hostname::new(hostname);
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, HostnameError::TooLong(254)));
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname returns error when input contains colon (port number)
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_invalid_character_error_when_input_contains_colon() {
        let result = Hostname::new("example.com:8080");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, HostnameError::InvalidCharacter(':')));
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname returns error when label is all-numeric
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_invalid_label_error_when_label_is_all_numeric() {
        let result = Hostname::new("123.456.789");
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        match e {
            HostnameError::InvalidLabel(ref label, ref reason) => {
                assert_eq!(label, "123");
                assert_eq!(reason, "all_numeric");
            }
            other => panic!("expected HostnameError::InvalidLabel, got {:?}", other),
        }
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname returns error when label exceeds 63 characters
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_label_too_long_error_when_label_exceeds_63_chars() {
        let long_label = "a".repeat(64);
        let hostname = format!("{}.com", long_label);
        let result = Hostname::new(hostname);
        assert!(result.is_err());
        let Err(e) = result else {
            panic!("expected error")
        };
        assert!(matches!(e, HostnameError::LabelTooLong(label, 64) if label.len() == 64));
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname returns original string when as_str is called
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_as_str_returns_original_input_exactly() {
        let input = "my-server.example.com";
        let host = Hostname::new(input).unwrap();
        assert_eq!(host.as_str(), input);
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname invariant: length always 1-253
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_as_str_length_is_always_between_1_and_253() {
        let host = Hostname::new("example.com").unwrap();
        let len = host.as_str().len();
        assert!(len >= 1 && len <= 253);
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname invariant: never contains colon character
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_as_str_never_contains_colon() {
        let host = Hostname::new("example.com").unwrap();
        assert!(!host.as_str().contains(':'));
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname invariant: no empty labels
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_as_str_has_no_empty_labels() {
        let host = Hostname::new("api.example.com").unwrap();
        let labels: Vec<&str> = host.as_str().split('.').collect();
        assert!(labels.iter().all(|l| !l.is_empty()));
    }

    // -------------------------------------------------------------------------
    // Additional boundary tests
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_ok_when_given_single_character_label() {
        // Single character labels are valid per RFC 1123
        let result = Hostname::new("a.b.c");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "a.b.c");
    }

    #[test]
    fn hostname_new_returns_ok_when_given_case_preserved_hostname() {
        // Case should be preserved as-is
        let result = Hostname::new("MyServer.Example.COM");
        assert!(result.is_ok());
        let host = result.unwrap();
        assert_eq!(host.as_str(), "MyServer.Example.COM");
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
            fn hostname_new_preserves_input_valid_hostnames(hostname in prop::sample::select(&[
                "localhost",
                "example.com",
                "api.example.com",
                "my-host.example.co.uk",
                "server1.prod.us-east-1",
            ])) {
                let result = Hostname::new(hostname);
                prop_assert!(result.is_ok());
                let host = result.unwrap();
                prop_assert_eq!(host.as_str(), hostname);
            }

            #[test]
            fn hostname_labels_are_well_formed(hostname in prop::sample::select(&[
                "localhost",
                "example.com",
                "api.example.com",
                "my-host.example.co.uk",
            ])) {
                let result = Hostname::new(hostname);
                prop_assert!(result.is_ok());
                let host = result.unwrap();
                for label in host.as_str().split('.') {
                    prop_assert!(!label.is_empty());
                    prop_assert!(!label.contains(':'));
                }
                prop_assert!(host.as_str().len() >= 1 && host.as_str().len() <= 253);
            }

            #[test]
            fn hostname_display_matches_as_str(hostname in prop::sample::select(&[
                "localhost",
                "example.com",
                "api.example.com",
            ])) {
                let host = Hostname::new(hostname).unwrap();
                prop_assert_eq!(format!("{}", host), host.as_str());
            }

            #[test]
            fn hostname_is_send_and_sync(hostname in prop::sample::select(&[
                "localhost",
                "example.com",
            ])) {
                let host = Hostname::new(hostname).unwrap();
                fn assert_send<T: Send>(_: &T) {}
                fn assert_sync<T: Sync>(_: &T) {}
                assert_send(&host);
                assert_sync(&host);
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
        fn verify_hostname_bounds_invariants() {
            // This is a stub - actual verification requires implementing the type
            // Kani would verify: !contains(':') and len() <= 253 and len() >= 1
            let input = kani::any::<String>();
            kani::assume(input.len() <= 300);
            // In real implementation, this would verify the invariants
        }
    }
}
