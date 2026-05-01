use axum::http::{header, StatusCode};
use axum::Router;
use std::sync::Arc;
use twerk_app::engine::coordinator::limits::{rate_limit_middleware, RateLimitConfig};
use tower::ServiceExt;

#[tokio::test]
async fn rate_limit_allows_requests_under_limit() {
    let config = RateLimitConfig::new(5);
    let app = Router::new()
        .route("/test", tower::handler::Handler::get(test_handler))
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
        .route("/test", tower::handler::Handler::get(test_handler))
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
async fn rate_limit_per_ip_tracking_different_ip_succeeds() {
    let config = RateLimitConfig::new(5);
    let app = Router::new()
        .route("/test", tower::handler::Handler::get(test_handler))
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
                .header("x-forwarded-for", "192.168.1.2")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Different IP should not be rate limited when first IP is at limit"
    );
}

async fn test_handler() -> axum::response::Json<&'static str> {
    axum::response::Json("OK")
}