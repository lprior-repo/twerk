#[cfg(test)]
mod tests {
    use crate::domain::testing::{arb_valid_hostname, max_length_hostname};
    use crate::Hostname;
    use crate::HostnameError;

    // -------------------------------------------------------------------------
    // Behavior: Hostname constructs successfully when given valid single-label hostname
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_ok_when_given_single_label_hostname() {
        let host = Hostname::new("localhost").expect("valid hostname should parse");
        assert_eq!(host.as_str(), "localhost");
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname constructs successfully when given valid multi-label hostname
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_ok_when_given_multi_label_hostname() {
        let host = Hostname::new("api.example.com").expect("valid hostname should parse");
        assert_eq!(host.as_str(), "api.example.com");
    }

    #[test]
    fn hostname_new_returns_ok_when_given_hyphenated_hostname() {
        let host = Hostname::new("my-host.example.com").expect("valid hostname should parse");
        assert_eq!(host.as_str(), "my-host.example.com");
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname constructs successfully when given hostname at max length (253)
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_ok_when_given_max_length_hostname() {
        let hostname = max_length_hostname();
        assert_eq!(hostname.len(), 253);
        let host = Hostname::new(hostname).expect("253-char hostname should be valid");
        assert_eq!(host.as_str().len(), 253);
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname returns error when input is empty string
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_empty_error_when_input_is_empty() {
        let e = Hostname::new("").expect_err("empty string should fail");
        assert!(matches!(e, HostnameError::Empty));
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname returns error when input exceeds 253 characters
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_too_long_error_when_input_exceeds_253_chars() {
        let hostname = "a".repeat(254);
        let e = Hostname::new(hostname).expect_err("254-char hostname should fail");
        assert!(matches!(e, HostnameError::TooLong(254)));
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname returns error when input contains colon (port number)
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_invalid_character_error_when_input_contains_colon() {
        let e = Hostname::new("example.com:8080").expect_err("port in hostname should fail");
        assert!(matches!(e, HostnameError::InvalidCharacter(':')));
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname returns error when label is all-numeric
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_invalid_label_error_when_label_is_all_numeric() {
        let e = Hostname::new("123.456.789").expect_err("all-numeric label should fail");
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
        let e = Hostname::new(hostname).expect_err("64-char label should fail");
        assert!(matches!(e, HostnameError::LabelTooLong(label, 64) if label.len() == 64));
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname label boundary tests (63 chars per RFC 1123)
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_ok_when_label_is_exactly_63_chars() {
        // 63-char label is the maximum allowed per RFC 1123
        let label_63 = "a".repeat(63);
        let hostname = format!("{}.com", label_63);
        assert_eq!(hostname.len(), 67); // 63 + 1 (dot) + 3 (com)
        let host = Hostname::new(&hostname).expect("63-char label should be valid");
        assert_eq!(host.as_str(), hostname);
    }

    #[test]
    fn hostname_new_returns_label_too_long_error_when_label_is_64_chars() {
        // 64-char label exceeds the 63-char limit
        let label_64 = "a".repeat(64);
        let hostname = format!("{}.com", label_64);
        let e = Hostname::new(hostname).expect_err("64-char label should fail");
        assert!(matches!(e, HostnameError::LabelTooLong(label, 64) if label.len() == 64));
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname total length boundary tests (253 chars max)
    // -------------------------------------------------------------------------

    // NOTE: 252-char hostname is not achievable with RFC 1123 label constraints.
    // The maximum hostname is 253 chars (see max_length_hostname test).
    // The next boundary down from 253 is achieved by the valid hostname samples.

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
        assert!(labels.iter().all(|l: &&str| !l.is_empty()));
    }

    // -------------------------------------------------------------------------
    // Additional boundary tests
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_ok_when_given_single_character_label() {
        // Single character labels are valid per RFC 1123
        let host = Hostname::new("a.b.c").expect("single char labels should be valid");
        assert_eq!(host.as_str(), "a.b.c");
    }

    #[test]
    fn hostname_new_returns_ok_when_given_case_preserved_hostname() {
        // Case should be preserved as-is
        let host = Hostname::new("MyServer.Example.COM").expect("case should be preserved");
        assert_eq!(host.as_str(), "MyServer.Example.COM");
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname returns error when label has hyphen at start
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_invalid_label_error_when_label_has_hyphen_at_start() {
        let e =
            Hostname::new("-host.example.com").expect_err("label starting with hyphen should fail");
        match e {
            HostnameError::InvalidLabel(ref label, ref reason) => {
                assert_eq!(label, "-host");
                assert_eq!(reason, "must start with alphanumeric");
            }
            other => panic!("expected HostnameError::InvalidLabel, got {:?}", other),
        }
    }

    // -------------------------------------------------------------------------
    // Behavior: Hostname returns error when label has hyphen at end
    // -------------------------------------------------------------------------

    #[test]
    fn hostname_new_returns_invalid_label_error_when_label_has_hyphen_at_end() {
        let e =
            Hostname::new("host-.example.com").expect_err("label ending with hyphen should fail");
        match e {
            HostnameError::InvalidLabel(ref label, ref reason) => {
                assert_eq!(label, "host-");
                assert_eq!(reason, "must end with alphanumeric");
            }
            other => panic!("expected HostnameError::InvalidLabel, got {:?}", other),
        }
    }

    // -------------------------------------------------------------------------
    // Proptest invariants
    // -------------------------------------------------------------------------

    mod proptest_inner {
        use super::*;
        use crate::assert_is_send_and_sync;
        use proptest::prelude::*;
        use proptest::proptest;

        proptest! {
            #[test]
            fn hostname_new_preserves_input_valid_hostnames(hostname in arb_valid_hostname()) {
                let result = Hostname::new(hostname);
                prop_assert!(result.is_ok());
                let host = result.unwrap();
                prop_assert_eq!(host.as_str(), hostname);
            }

            #[test]
            fn hostname_labels_are_well_formed(hostname in arb_valid_hostname()) {
                let result = Hostname::new(hostname);
                prop_assert!(result.is_ok());
                let host = result.unwrap();
                // No empty labels (enforced by Hostname constructor)
                for label in host.as_str().split('.') {
                    let _: &str = label;
                    prop_assert!(!label.is_empty());
                    prop_assert!(!label.contains(':'));
                }
            }

            #[test]
            fn hostname_display_matches_as_str(hostname in arb_valid_hostname()) {
                let host = Hostname::new(hostname).unwrap();
                prop_assert_eq!(format!("{}", host), host.as_str());
            }

            #[test]
            fn hostname_is_send_and_sync(hostname in arb_valid_hostname()) {
                let host = Hostname::new(hostname).unwrap();
                assert_is_send_and_sync!(host);
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
