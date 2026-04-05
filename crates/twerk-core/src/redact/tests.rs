#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::module_inception)]
#[allow(unused_imports)]
mod tests {
    use crate::redact::{
        is_secret_key, redact_job, redact_task, redact_task_log_parts, redact_vars, Redacter,
        DEFAULT_KEYS, REDACTED_STR,
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
        assert!(!redacter.contains("NOT_SENSITIVE")); // NOT_SENSITIVE doesn't contain any key
        assert!(!redacter.contains(""));
    }

    #[test]
    fn test_redacter_wildcard_replaces_all_occurrences() {
        let redacter = Redacter::default_redacter();

        // Key "SECRET" matches in "SECRET_KEY" and "SECRET"
        let result = redacter.wildcard("SECRET_KEY=SECRET");
        assert_eq!(result, "[REDACTED]_KEY=[REDACTED]");

        // Key "PASSWORD" matches in "password=abc123" and "password=def456"
        // Note: This replaces just the key "password", not the entire key=value
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
        assert_eq!(result, "SECRET"); // Empty key matches nothing meaningful
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
        let mut task = crate::task::Task {
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

        redact_task(&mut task, &secrets);
        assert_eq!(
            task.env.as_ref().unwrap()["DATABASE_PASSWORD"],
            "[REDACTED]"
        );
    }

    #[test]
    fn test_redact_task_registry_password() {
        let mut task = crate::task::Task {
            registry: Some(crate::task::Registry {
                username: Some("admin".to_string()),
                password: Some("registry_secret".to_string()),
            }),
            ..Default::default()
        };
        let secrets: HashMap<String, String> = HashMap::new();

        redact_task(&mut task, &secrets);
        assert_eq!(
            task.registry.as_ref().unwrap().password.as_ref().unwrap(),
            "[REDACTED]"
        );
    }

    #[test]
    fn test_redact_task_log_parts() {
        let mut parts = vec![crate::task::TaskLogPart {
            contents: Some("Using token abc123 and password secret123".to_string()),
            ..Default::default()
        }];
        let secrets: HashMap<String, String> = [("my_token".to_string(), "abc123".to_string())]
            .into_iter()
            .collect();

        redact_task_log_parts(&mut parts, &secrets);
        assert_eq!(
            parts[0].contents.as_ref().unwrap(),
            "Using token [REDACTED] and password secret123"
        );
    }

    #[test]
    fn test_redact_job() {
        let mut job = crate::job::Job {
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

        redact_job(&mut job);
        let inputs = job.inputs.unwrap();
        assert_eq!(inputs["log_message"], "Using token [REDACTED]");
        let secrets = job.secrets.unwrap();
        assert_eq!(secrets["my_token"], "[REDACTED]");
    }
}
