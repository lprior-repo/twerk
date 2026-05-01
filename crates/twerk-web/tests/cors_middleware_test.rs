use axum::http::{header, HeaderValue, Method, Request, StatusCode};
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

async fn test_handler() -> axum::response::Json<&'static str> {
    axum::response::Json("OK")
}

fn build_router_with_cors(cors: CorsLayer) -> Router {
    Router::new()
        .route("/test", get(test_handler))
        .layer(cors)
}

fn build_options_request(origin: &str) -> Request<axum::body::Body> {
    Request::builder()
        .method(Method::OPTIONS)
        .uri("/test")
        .header(header::ORIGIN, origin)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
        .body(axum::body::Body::empty())
        .unwrap()
}

fn build_get_request_with_origin(origin: &str) -> Request<axum::body::Body> {
    Request::builder()
        .method(Method::GET)
        .uri("/test")
        .header(header::ORIGIN, origin)
        .body(axum::body::Body::empty())
        .unwrap()
}

#[tokio::test]
async fn cors_options_request_with_registered_origin_returns_allow_origin_header() {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = build_router_with_cors(cors);
    let response = app
        .oneshot(build_options_request("http://localhost:3000"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers().contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        "Expected Access-Control-Allow-Origin header in response"
    );
}

#[tokio::test]
async fn cors_options_request_allow_methods_includes_get_post_put_delete() {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any);

    let app = build_router_with_cors(cors);
    let response = app
        .oneshot(build_options_request("http://localhost:3000"))
        .await
        .unwrap();

    let allow_methods = response.headers().get(header::ACCESS_CONTROL_ALLOW_METHODS);

    assert!(
        allow_methods.is_some(),
        "Expected Access-Control-Allow-Methods header in response"
    );

    let methods_str = allow_methods.unwrap().to_str().unwrap();
    assert!(
        methods_str.contains("GET"),
        "Allow-Methods should include GET, got: {}",
        methods_str
    );
    assert!(
        methods_str.contains("POST"),
        "Allow-Methods should include POST, got: {}",
        methods_str
    );
    assert!(
        methods_str.contains("PUT"),
        "Allow-Methods should include PUT, got: {}",
        methods_str
    );
    assert!(
        methods_str.contains("DELETE"),
        "Allow-Methods should include DELETE, got: {}",
        methods_str
    );
}

#[tokio::test]
async fn cors_get_request_with_registered_origin_returns_allow_origin_header() {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = build_router_with_cors(cors);
    let response = app
        .oneshot(build_get_request_with_origin("http://localhost:3000"))
        .await
        .unwrap();

    assert!(
        response.headers().contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        "Expected Access-Control-Allow-Origin header for GET request with Origin"
    );
}

#[tokio::test]
async fn cors_credentials_flag_sets_allow_credentials_true() {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers([
            axum::http::HeaderName::from_static("content-type"),
            axum::http::HeaderName::from_static("authorization"),
        ])
        .allow_credentials(true);

    let app = build_router_with_cors(cors);
    let response = app
        .oneshot(build_get_request_with_origin("http://localhost:3000"))
        .await
        .unwrap();

    let allow_cred = response.headers().get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS);

    assert!(
        allow_cred.is_some(),
        "Expected Access-Control-Allow-Credentials header when credentials enabled"
    );
    assert_eq!(
        allow_cred.unwrap().to_str().unwrap(),
        "true",
        "Access-Control-Allow-Credentials should be 'true'"
    );
}

#[tokio::test]
async fn cors_allow_origin_predicate_respects_registered_origins() {
    let allowed_origin = HeaderValue::from_static("http://localhost:3000");

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin, _| origin == &allowed_origin))
        .allow_methods(Any)
        .allow_headers(Any);

    let app = build_router_with_cors(cors);

    let valid_response = app.clone()
        .oneshot(build_get_request_with_origin("http://localhost:3000"))
        .await
        .unwrap();

    assert!(
        valid_response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        "Registered origin http://localhost:3000 should get CORS headers"
    );

    let invalid_response = app.clone()
        .oneshot(build_get_request_with_origin("http://malicious-site.com"))
        .await
        .unwrap();

    assert!(
        !invalid_response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        "Unregistered origin should NOT get Access-Control-Allow-Origin header"
    );
}

#[tokio::test]
async fn cors_expose_headers_are_set() {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .expose_headers(Any);

    let app = build_router_with_cors(cors);
    let response = app
        .oneshot(build_get_request_with_origin("http://localhost:3000"))
        .await
        .unwrap();

    assert!(
        response.headers().contains_key(header::ACCESS_CONTROL_EXPOSE_HEADERS),
        "Expose-Headers should be set when expose_headers(Any) is configured"
    );
}
