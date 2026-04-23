use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
    routing::get,
    Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::sync::Arc;
use tower::ServiceExt;
use twerk_app::engine::coordinator::auth::{
    basic_auth_middleware, key_auth_middleware, BasicAuthConfig, KeyAuthConfig,
};
use twerk_core::{id::UserId, user::User};
use twerk_infrastructure::datastore::{inmemory::InMemoryDatastore, Datastore};

async fn ok_handler() -> &'static str {
    "OK"
}

async fn build_basic_auth_app(users: &[(&str, &str)]) -> anyhow::Result<Router> {
    let datastore = Arc::new(InMemoryDatastore::new());

    for (username, password) in users {
        let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
        let user = User {
            id: Some(UserId::new(format!("user-{username}"))?),
            username: Some((*username).to_string()),
            password_hash: Some(password_hash),
            ..User::default()
        };
        datastore.create_user(&user).await?;
    }

    let config = BasicAuthConfig::new(datastore);

    Ok(Router::new()
        .route("/test", get(ok_handler))
        .layer(axum::middleware::from_fn_with_state(
            config,
            |state, request, next| {
                Box::pin(async move { basic_auth_middleware(state, request, next).await })
            },
        )))
}

fn build_key_auth_app(config: KeyAuthConfig) -> Router {
    Router::new()
        .route("/api/endpoint", get(ok_handler))
        .route("/health", get(ok_handler))
        .route("/healthz", get(ok_handler))
        .layer(axum::middleware::from_fn_with_state(
            config,
            |state, request, next| {
                Box::pin(async move { key_auth_middleware(state, request, next).await })
            },
        ))
}

fn request(method: Method, uri: &str) -> anyhow::Result<Request<Body>> {
    Ok(Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())?)
}

fn request_with_header(
    method: Method,
    uri: &str,
    header_name: header::HeaderName,
    header_value: &str,
) -> anyhow::Result<Request<Body>> {
    Ok(Request::builder()
        .method(method)
        .uri(uri)
        .header(header_name, header_value)
        .body(Body::empty())?)
}

async fn response_status(app: Router, request: Request<Body>) -> anyhow::Result<StatusCode> {
    Ok(app.oneshot(request).await?.status())
}

fn basic_credentials(username: &str, password: &str) -> String {
    STANDARD.encode(format!("{username}:{password}"))
}

#[tokio::test]
async fn test_valid_basic_auth_credentials_pass() -> anyhow::Result<()> {
    let app = build_basic_auth_app(&[("testuser", "testpassword")]).await?;
    let request = request_with_header(
        Method::GET,
        "/test",
        header::AUTHORIZATION,
        &format!("Basic {}", basic_credentials("testuser", "testpassword")),
    )?;

    let status = response_status(app, request).await?;

    assert_eq!(status, StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn test_invalid_basic_auth_credentials_return_401() -> anyhow::Result<()> {
    let app = build_basic_auth_app(&[("testuser", "testpassword")]).await?;
    let request = request_with_header(
        Method::GET,
        "/test",
        header::AUTHORIZATION,
        &format!("Basic {}", basic_credentials("testuser", "wrongpassword")),
    )?;

    let status = response_status(app, request).await?;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn test_missing_basic_auth_credentials_return_401() -> anyhow::Result<()> {
    let app = build_basic_auth_app(&[]).await?;
    let request = request(Method::GET, "/test")?;

    let status = response_status(app, request).await?;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn test_valid_api_key_passes() -> anyhow::Result<()> {
    let app = build_key_auth_app(KeyAuthConfig::new("valid-api-key".to_string()));
    let request = request_with_header(
        Method::GET,
        "/api/endpoint",
        header::HeaderName::from_static("x-api-key"),
        "valid-api-key",
    )?;

    let status = response_status(app, request).await?;

    assert_eq!(status, StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn test_invalid_api_key_returns_401() -> anyhow::Result<()> {
    let app = build_key_auth_app(KeyAuthConfig::new("valid-api-key".to_string()));
    let request = request_with_header(
        Method::GET,
        "/api/endpoint",
        header::HeaderName::from_static("x-api-key"),
        "invalid-api-key",
    )?;

    let status = response_status(app, request).await?;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn test_missing_api_key_returns_401() -> anyhow::Result<()> {
    let app = build_key_auth_app(KeyAuthConfig::new("valid-api-key".to_string()));
    let request = request(Method::GET, "/api/endpoint")?;

    let status = response_status(app, request).await?;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn test_request_matching_skip_path_bypasses_auth() -> anyhow::Result<()> {
    let app = build_key_auth_app(
        KeyAuthConfig::new("valid-api-key".to_string())
            .with_skip_paths(vec!["GET /health".to_string()]),
    );
    let request = request(Method::GET, "/health")?;

    let status = response_status(app, request).await?;

    assert_eq!(status, StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn test_api_key_in_query_param_passes() -> anyhow::Result<()> {
    let app = build_key_auth_app(KeyAuthConfig::new("valid-api-key".to_string()));
    let request = request(Method::GET, "/api/endpoint?api_key=valid-api-key")?;

    let status = response_status(app, request).await?;

    assert_eq!(status, StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn test_wildcard_skip_path_matching() -> anyhow::Result<()> {
    let app = build_key_auth_app(
        KeyAuthConfig::new("valid-api-key".to_string())
            .with_skip_paths(vec!["GET /health*".to_string()]),
    );
    let request = request(Method::GET, "/healthz")?;

    let status = response_status(app, request).await?;

    assert_eq!(status, StatusCode::OK);
    Ok(())
}
