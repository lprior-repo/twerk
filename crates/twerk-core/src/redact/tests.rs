#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::module_inception)]
#[allow(unused_imports)]
mod tests {
    use crate::redact::{
        is_secret_key, redact_job, redact_task, redact_task_log_parts, redact_vars, Redacter,
    };
    use std::collections::HashMap;

    #[test]
    fn test_redacter_contains_case_insensitive() {
        let redacter = Redacter::default_redacter();

        assert!(redacter.contains("MY_SECRET_TOKEN"));
        assert!(redacter.contains("my_secret_token"));
        assert!(redacter.contains("My_Secret_Token"));
        assert!(redacter.contains("PASSWORD_FIELD"));
        assert!(redacter.contains("AWS_ACCESS_KEY_ID"));
        assert!(!redacter.contains("NOT_SENSITIVE"));
        assert!(!redacter.contains(""));
    }

    #[test]
    fn test_redacter_wildcard_replaces_all_occurrences() {
        let redacter = Redacter::default_redacter();

        let result = redacter.wildcard("SECRET_KEY=SECRET");
        assert_eq!(result, "[REDACTED]_KEY=[REDACTED]");

        let result = redacter.wildcard("password=abc123 password=def456");
        assert_eq!(result, "[REDACTED]=abc123 [REDACTED]=def456");
    }

    #[test]
    fn test_redacter_wildcard_no_match() {
        let redacter = Redacter::default_redacter();

        let result = redacter.wildcard("normal_value");
        assert_eq!(result, "normal_value");
    }

    #[test]
    fn test_redacter_wildcard_empty_key() {
        let redacter = Redacter::new(vec![String::new()]);

        let result = redacter.wildcard("SECRET");
        assert_eq!(result, "SECRET");
    }

    #[test]
    fn test_is_secret_key() {
        assert!(is_secret_key("SECRET"));
        assert!(is_secret_key("MY_PASSWORD"));
        assert!(is_secret_key("AWS_ACCESS_KEY"));
        assert!(is_secret_key("secret"));
        assert!(is_secret_key("password"));
        assert!(!is_secret_key("normal_key"));
        assert!(!is_secret_key(""));
    }

    #[test]
    fn test_redact_vars_key_match() {
        let m: HashMap<String, String> = [("DB_PASSWORD".to_string(), "secret_val".to_string())]
            .into_iter()
            .collect();
        let secrets: HashMap<String, String> = HashMap::new();

        let result = redact_vars(&m, &secrets);
        assert_eq!(result["DB_PASSWORD"], "[REDACTED]");
    }

    #[test]
    fn test_redact_vars_value_match() {
        let m: HashMap<String, String> = [("config".to_string(), "api_key=sk-12345".to_string())]
            .into_iter()
            .collect();
        let secrets: HashMap<String, String> = [("key1".to_string(), "sk-12345".to_string())]
            .into_iter()
            .collect();

        let result = redact_vars(&m, &secrets);
        assert_eq!(result["config"], "api_key=[REDACTED]");
    }

    #[test]
    fn test_redact_vars_no_match() {
        let m: HashMap<String, String> = [("normal_key".to_string(), "normal_value".to_string())]
            .into_iter()
            .collect();
        let secrets: HashMap<String, String> = HashMap::new();

        let result = redact_vars(&m, &secrets);
        assert_eq!(result["normal_key"], "normal_value");
    }

    #[test]
    fn test_redact_vars_handles_empty_secrets() {
        let m: HashMap<String, String> = [("api_key".to_string(), "should_stay".to_string())]
            .into_iter()
            .collect();
        let secrets: HashMap<String, String> = HashMap::new();

        let result = redact_vars(&m, &secrets);
        assert_eq!(result["api_key"], "should_stay");
    }

    #[test]
    fn test_redact_task_env() {
        let task = crate::task::Task {
            env: Some(
                [("DATABASE_PASSWORD".to_string(), "db_pass_123".to_string())]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        };
        let secrets: HashMap<String, String> = [("db_key".to_string(), "db_pass_123".to_string())]
            .into_iter()
            .collect();

        let redacted = redact_task(task, &secrets);
        assert_eq!(
            redacted.env.as_ref().unwrap()["DATABASE_PASSWORD"],
            "[REDACTED]"
        );
    }

    #[test]
    fn test_redact_task_registry_password() {
        let task = crate::task::Task {
            registry: Some(crate::task::Registry {
                username: Some("admin".to_string()),
                password: Some("registry_secret".to_string()),
            }),
            ..Default::default()
        };
        let secrets: HashMap<String, String> = HashMap::new();

        let redacted = redact_task(task, &secrets);
        assert_eq!(
            redacted
                .registry
                .as_ref()
                .unwrap()
                .password
                .as_ref()
                .unwrap(),
            "[REDACTED]"
        );
    }

    #[test]
    fn test_redact_task_log_parts() {
        let parts = vec![crate::task::TaskLogPart {
            contents: Some("Using token abc123 and password secret123".to_string()),
            ..Default::default()
        }];
        let secrets: HashMap<String, String> = [("my_token".to_string(), "abc123".to_string())]
            .into_iter()
            .collect();

        let redacted = redact_task_log_parts(parts, &secrets);
        assert_eq!(
            redacted[0].contents.as_ref().unwrap(),
            "Using token [REDACTED] and password secret123"
        );
    }

    #[test]
    fn test_redact_job() {
        let job = crate::job::Job {
            inputs: Some(
                [("log_message".to_string(), "Using token abc123".to_string())]
                    .into_iter()
                    .collect(),
            ),
            secrets: Some(
                [("my_token".to_string(), "abc123".to_string())]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        };

        let redacted = redact_job(job);
        let inputs = redacted.inputs.unwrap();
        assert_eq!(inputs["log_message"], "Using token [REDACTED]");
        let secrets = redacted.secrets.unwrap();
        assert_eq!(secrets["my_token"], "[REDACTED]");
    }
}

#[cfg(test)]
mod proptest_tests {
    use crate::redact::{is_secret_key, redact_vars};
    use proptest::prelude::*;
    use std::collections::HashMap;

    proptest! {
        #[test]
        fn redact_vars_preserves_keys(
            keys in proptest::collection::vec("[a-zA-Z_]{1,20}", 0..10),
            vals in proptest::collection::vec(".{0,20}", 0..10)
        ) {
            let m: HashMap<String, String> = keys
                .iter()
                .zip(vals.iter())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let secrets = HashMap::new();
            let result = redact_vars(&m, &secrets);
            prop_assert_eq!(result.len(), m.len());
            for k in m.keys() {
                prop_assert!(result.contains_key(k));
            }
        }

        #[test]
        fn is_secret_key_case_insensitive(key in "[a-zA-Z_]{1,20}") {
            let upper = key.to_uppercase();
            let contains_secret = upper.contains("SECRET")
                || upper.contains("PASSWORD")
                || upper.contains("ACCESS_KEY");
            prop_assert_eq!(is_secret_key(&key), contains_secret);
        }

        #[test]
        fn redact_vars_secret_keys_always_redacted(
            value in ".{0,50}"
        ) {
            let mut m = HashMap::new();
            m.insert("DB_PASSWORD".to_string(), value.clone());
            let secrets = HashMap::new();
            let result = redact_vars(&m, &secrets);
            prop_assert_eq!(&result["DB_PASSWORD"], "[REDACTED]");
        }
    }
}
