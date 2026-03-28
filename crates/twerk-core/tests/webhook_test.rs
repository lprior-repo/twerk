use mockito::Server;
use std::collections::HashMap;
use twerk_core::webhook::{
    self, is_retryable, Webhook, WebhookError, EVENT_DEFAULT, EVENT_JOB_PROGRESS,
    EVENT_JOB_STATE_CHANGE, EVENT_TASK_PROGRESS, EVENT_TASK_STATE_CHANGE,
};

fn webhook_url(server: &Server, path: &str) -> String {
    format!("{}{}", server.url(), path)
}

#[test]
fn is_retryable_returns_true_for_429() {
    assert!(is_retryable(429));
}

#[test]
fn is_retryable_returns_true_for_500() {
    assert!(is_retryable(500));
}

#[test]
fn is_retryable_returns_true_for_502() {
    assert!(is_retryable(502));
}

#[test]
fn is_retryable_returns_true_for_503() {
    assert!(is_retryable(503));
}

#[test]
fn is_retryable_returns_true_for_504() {
    assert!(is_retryable(504));
}

#[test]
fn is_retryable_returns_false_for_200() {
    assert!(!is_retryable(200));
}

#[test]
fn is_retryable_returns_false_for_201() {
    assert!(!is_retryable(201));
}

#[test]
fn is_retryable_returns_false_for_400() {
    assert!(!is_retryable(400));
}

#[test]
fn is_retryable_returns_false_for_401() {
    assert!(!is_retryable(401));
}

#[test]
fn is_retryable_returns_false_for_403() {
    assert!(!is_retryable(403));
}

#[test]
fn is_retryable_returns_false_for_404() {
    assert!(!is_retryable(404));
}

#[test]
fn webhook_call_succeeds_on_200() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/webhook")
        .match_header("content-type", "application/json; charset=UTF-8")
        .with_status(200)
        .create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_ok());
    mock.assert();
}

#[test]
fn webhook_call_succeeds_on_201() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/webhook")
        .match_header("content-type", "application/json; charset=UTF-8")
        .with_status(201)
        .create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_ok());
    mock.assert();
}

#[test]
fn webhook_call_returns_error_on_non_retryable_status_400() {
    let mut server = Server::new();
    let mock = server.mock("POST", "/webhook").with_status(400).create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_err());
    match result {
        Err(WebhookError::NonRetryableError(url, status)) => {
            assert_eq!(status, 400);
            assert!(url.contains("/webhook"));
        }
        _ => panic!("Expected NonRetryableError"),
    }
    mock.assert();
}

#[test]
fn webhook_call_returns_error_on_401() {
    let mut server = Server::new();
    let mock = server.mock("POST", "/webhook").with_status(401).create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_err());
    match result {
        Err(WebhookError::NonRetryableError(_, status)) => {
            assert_eq!(status, 401);
        }
        _ => panic!("Expected NonRetryableError"),
    }
    mock.assert();
}

#[test]
fn webhook_call_returns_error_on_403() {
    let mut server = Server::new();
    let mock = server.mock("POST", "/webhook").with_status(403).create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_err());
    match result {
        Err(WebhookError::NonRetryableError(_, status)) => {
            assert_eq!(status, 403);
        }
        _ => panic!("Expected NonRetryableError"),
    }
    mock.assert();
}

#[test]
fn webhook_call_returns_error_on_404() {
    let mut server = Server::new();
    let mock = server.mock("POST", "/webhook").with_status(404).create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_err());
    match result {
        Err(WebhookError::NonRetryableError(_, status)) => {
            assert_eq!(status, 404);
        }
        _ => panic!("Expected NonRetryableError"),
    }
    mock.assert();
}

#[test]
fn webhook_call_retries_on_429_then_succeeds() {
    let mut server = Server::new();
    let _mock1 = server.mock("POST", "/webhook").with_status(429).create();
    let _mock2 = server.mock("POST", "/webhook").with_status(200).create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_ok());
}

#[test]
fn webhook_call_retries_on_500_then_succeeds() {
    let mut server = Server::new();
    let _mock1 = server.mock("POST", "/webhook").with_status(500).create();
    let _mock2 = server.mock("POST", "/webhook").with_status(200).create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_ok());
}

#[test]
fn webhook_call_retries_on_502_then_succeeds() {
    let mut server = Server::new();
    let _mock1 = server.mock("POST", "/webhook").with_status(502).create();
    let _mock2 = server.mock("POST", "/webhook").with_status(200).create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_ok());
}

#[test]
fn webhook_call_retries_on_503_then_succeeds() {
    let mut server = Server::new();
    let _mock1 = server.mock("POST", "/webhook").with_status(503).create();
    let _mock2 = server.mock("POST", "/webhook").with_status(200).create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_ok());
}

#[test]
fn webhook_call_retries_on_504_then_succeeds() {
    let mut server = Server::new();
    let _mock1 = server.mock("POST", "/webhook").with_status(504).create();
    let _mock2 = server.mock("POST", "/webhook").with_status(200).create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_ok());
}

#[test]
fn webhook_call_exhausts_retries_on_persistent_500() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/webhook")
        .expect(5)
        .with_status(500)
        .create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_err());
    match result {
        Err(WebhookError::MaxAttemptsExceeded(url, attempts)) => {
            assert_eq!(attempts, 5);
            assert!(url.contains("/webhook"));
        }
        _ => panic!("Expected MaxAttemptsExceeded"),
    }
    mock.assert();
}

#[test]
fn webhook_call_returns_error_when_url_is_none() {
    let webhook = Webhook {
        url: None,
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_err());
    match result {
        Err(WebhookError::NonRetryableError(msg, 0)) => {
            assert_eq!(msg, "missing url");
        }
        _ => panic!("Expected NonRetryableError"),
    }
}

#[test]
fn webhook_call_sends_custom_headers() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/webhook")
        .match_header("x-custom-header", "custom-value")
        .match_header("x-another-header", "another-value")
        .with_status(200)
        .create();

    let mut headers = HashMap::new();
    headers.insert("x-custom-header".to_string(), "custom-value".to_string());
    headers.insert("x-another-header".to_string(), "another-value".to_string());

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        headers: Some(headers),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_ok());
    mock.assert();
}

#[test]
fn webhook_call_sends_json_content_type() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/webhook")
        .match_header("content-type", "application/json; charset=UTF-8")
        .with_status(200)
        .create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_ok());
    mock.assert();
}

#[test]
fn webhook_call_serializes_body_to_json() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/webhook")
        .match_body(r#"{"key":"value","number":42}"#)
        .with_status(200)
        .create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    #[derive(serde::Serialize)]
    struct TestBody {
        key: String,
        number: i32,
    }

    let result = webhook::call(
        &webhook,
        &TestBody {
            key: "value".to_string(),
            number: 42,
        },
    );

    assert!(result.is_ok());
    mock.assert();
}

#[test]
fn webhook_call_handles_connection_error() {
    let webhook = Webhook {
        url: Some("http://localhost:99999/webhook".to_string()),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_err());
}

#[test]
fn event_constants_match_expected_values() {
    assert_eq!(EVENT_JOB_STATE_CHANGE, "job.StateChange");
    assert_eq!(EVENT_JOB_PROGRESS, "job.Progress");
    assert_eq!(EVENT_TASK_STATE_CHANGE, "task.StateChange");
    assert_eq!(EVENT_TASK_PROGRESS, "task.Progress");
    assert_eq!(EVENT_DEFAULT, "");
}

#[test]
fn webhook_default_is_empty() {
    let webhook = Webhook::default();
    assert!(webhook.url.is_none());
    assert!(webhook.headers.is_none());
    assert!(webhook.event.is_none());
    assert!(webhook.r#if.is_none());
}

#[test]
fn webhook_serialize_deserialize_roundtrip() {
    let webhook = Webhook {
        url: Some("https://example.com/hook".to_string()),
        event: Some("job.StateChange".to_string()),
        headers: Some(
            [("Authorization".to_string(), "Bearer token".to_string())]
                .into_iter()
                .collect(),
        ),
        r#if: Some("job_state == \"RUNNING\"".to_string()),
    };

    let json = serde_json::to_string(&webhook).unwrap();
    let deserialized: Webhook = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.url, webhook.url);
    assert_eq!(deserialized.event, webhook.event);
    assert_eq!(deserialized.headers, webhook.headers);
    assert_eq!(deserialized.r#if, webhook.r#if);
}

#[test]
fn webhook_serde_skips_none_fields() {
    let webhook = Webhook {
        url: Some("https://example.com/hook".to_string()),
        event: None,
        headers: None,
        r#if: None,
    };

    let json = serde_json::to_string(&webhook).unwrap();

    assert!(!json.contains("\"event\""));
    assert!(!json.contains("\"headers\""));
    assert!(!json.contains("\"if\""));
    assert!(json.contains("\"url\""));
}

#[test]
fn webhook_call_succeeds_with_empty_body() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/webhook")
        .match_header("content-type", "application/json; charset=UTF-8")
        .with_status(200)
        .create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({}));

    assert!(result.is_ok());
    mock.assert();
}

#[test]
fn webhook_call_multiple_retries_then_success() {
    let mut server = Server::new();
    let _mock1 = server.mock("POST", "/webhook").with_status(503).create();
    let _mock2 = server.mock("POST", "/webhook").with_status(503).create();
    let _mock3 = server.mock("POST", "/webhook").with_status(503).create();
    let _mock4 = server.mock("POST", "/webhook").with_status(200).create();

    let webhook = Webhook {
        url: Some(webhook_url(&server, "/webhook")),
        ..Default::default()
    };

    let result = webhook::call(&webhook, &serde_json::json!({"test": "data"}));

    assert!(result.is_ok());
}

#[test]
fn webhook_error_display_format_non_retryable() {
    let error = WebhookError::NonRetryableError("http://example.com".to_string(), 404);
    let display = format!("{}", error);
    assert!(display.contains("404"));
    assert!(display.contains("example.com"));
}

#[test]
fn webhook_error_display_format_max_attempts() {
    let error = WebhookError::MaxAttemptsExceeded("http://example.com".to_string(), 5);
    let display = format!("{}", error);
    assert!(display.contains("example.com"));
    assert!(display.contains("5"));
}

#[test]
fn webhook_error_display_format_serialization() {
    let error = WebhookError::SerializationError;
    let display = format!("{}", error);
    assert!(display.contains("serializing body"));
}
