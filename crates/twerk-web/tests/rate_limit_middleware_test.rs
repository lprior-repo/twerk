use axum::http::{header, StatusCode};
use axum::routing::get;
use axum::Router;
use twerk_app::engine::coordinator::limits::{rate_limit_middleware, RateLimitConfig};
use tower::ServiceExt;

#[tokio::test]
async fn rate_limit_allows_requests_under_limit() {
    let config = RateLimitConfig::new(5);
    let app = Router::new()
        .route("/test", get(test_handler))
        .layer(axum::middleware::from_fn_with_state(
            config,
            |st, req, next| Box::pin(async move { rate_limit_middleware(st, req, next).await }),
        ));

    for _ in 0..5 {
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("x-forwarded-for", "192.168.1.1")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "First 5 requests should succeed"
        );
    }
}

#[tokio::test]
async fn rate_limit_blocks_excessive_requests() {
    let config = RateLimitConfig::new(5);
    let app = Router::new()
        .route("/test", get(test_handler))
        .layer(axum::middleware::from_fn_with_state(
            config,
            |st, req, next| Box::pin(async move { rate_limit_middleware(st, req, next).await }),
        ));

    for _ in 0..5 {
        app.clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("x-forwarded-for", "192.168.1.1")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/test")
                .header("x-forwarded-for", "192.168.1.1")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "6th request should be rate limited"
    );
}

#[tokio::test]
async fn rate_limit_returns_429_with_retry_after_header_and_recovers() {
    let config = RateLimitConfig::new(5);
    let app = Router::new()
        .route("/test", get(test_handler))
        .layer(axum::middleware::from_fn_with_state(
            config,
            |st, req, next| Box::pin(async move { rate_limit_middleware(st, req, next).await }),
        ));

    for i in 0..5 {
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("x-forwarded-for", "10.0.0.1")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Request {} should succeed (within limit)",
            i + 1
        );
    }

    let rate_limited_response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/test")
                .header("x-forwarded-for", "10.0.0.1")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        rate_limited_response.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "6th request should be rate limited"
    );
    let retry_after = rate_limited_response
        .headers()
        .get(header::RETRY_AFTER)
        .expect("Retry-After header must be present on 429 response");
    let retry_secs: u64 = retry_after
        .to_str()
        .expect("Retry-After header value must be valid string")
        .parse()
        .expect("Retry-After header must be valid u64");
    assert!(
        retry_secs > 0,
        "Retry-After header value must be > 0, got {}",
        retry_secs
    );

    let wait_duration = std::time::Duration::from_secs(retry_secs + 1);
    tokio::time::sleep(wait_duration).await;

    let recovered_response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/test")
                .header("x-forwarded-for", "10.0.0.1")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        recovered_response.status(),
        StatusCode::OK,
        "Request after wait period should succeed"
    );
}

async fn test_handler() -> axum::response::Json<&'static str> {
    axum::response::Json("OK")
}