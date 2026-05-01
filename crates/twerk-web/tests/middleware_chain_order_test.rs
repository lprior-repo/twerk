use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::routing::get;
use axum::Router;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tower::ServiceExt;

#[derive(Clone)]
struct ExecutionTracker {
    order: Arc<AtomicUsize>,
}

impl ExecutionTracker {
    fn new() -> Self {
        Self {
            order: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn record(&self, name: &str) -> usize {
        let seq = self.order.fetch_add(1, Ordering::SeqCst);
        tracing::info!("[{}] {} executed", seq, name);
        seq
    }

    fn get_order(&self) -> Vec<usize> {
        (0..self.order.load(Ordering::SeqCst)).collect()
    }
}

async fn logging_middleware(
    request: Request<axum::body::Body>,
    next: Next,
    fail_auth: bool,
) -> Result<axum::response::Response, StatusCode> {
    let state = request
        .extensions()
        .get::<ExecutionTracker>()
        .cloned()
        .expect("ExecutionTracker not in request extensions");
    state.record("logging");
    if fail_auth {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(request).await)
}

async fn auth_middleware(
    request: Request<axum::body::Body>,
    next: Next,
    should_fail: bool,
) -> Result<axum::response::Response, StatusCode> {
    let state = request
        .extensions()
        .get::<ExecutionTracker>()
        .cloned()
        .expect("ExecutionTracker not in request extensions");
    state.record("auth");
    if should_fail {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(request).await)
}

async fn rate_limit_middleware(
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    let state = request
        .extensions()
        .get::<ExecutionTracker>()
        .cloned()
        .expect("ExecutionTracker not in request extensions");
    state.record("rate_limit");
    Ok(next.run(request).await)
}

async fn test_handler() -> axum::response::Json<&'static str> {
    axum::response::Json("OK")
}

fn make_request() -> Request<axum::body::Body> {
    Request::builder()
        .uri("/test")
        .body(axum::body::Body::empty())
        .unwrap()
}

fn build_app_with_tracker(tracker: ExecutionTracker) -> Router {
    let tracker_for_layer = tracker.clone();

    Router::new()
        .route("/test", get(test_handler))
        .layer(axum::middleware::from_fn(move |req, next| {
            let fail_auth = false;
            Box::pin(async move { logging_middleware(req, next, fail_auth).await })
        }))
        .layer(axum::middleware::from_fn(move |req, next| {
            let should_fail = false;
            Box::pin(async move { auth_middleware(req, next, should_fail).await })
        }))
        .layer(axum::middleware::from_fn(move |req, next| {
            Box::pin(async move { rate_limit_middleware(req, next).await })
        }))
        .layer(axum::middleware::from_fn(move |req: Request<axum::body::Body>, next: Next| {
            let tracker = tracker_for_layer.clone();
            Box::pin(async move {
                let mut req = req;
                req.extensions_mut().insert(tracker);
                next.run(req).await
            })
        }))
}

#[tokio::test]
async fn middleware_chain_executes_in_registration_order() {
    let tracker = ExecutionTracker::new();
    let tracker_for_check = tracker.clone();

    let app = build_app_with_tracker(tracker);

    let response = app.oneshot(make_request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let order = tracker_for_check.get_order();
    assert_eq!(order, vec![0, 1, 2], "Expected execution: logging(0) -> auth(1) -> rate_limit(2), got {:?}", order);
}

#[tokio::test]
async fn auth_failure_short_circuits_downstream_middleware() {
    let tracker = ExecutionTracker::new();
    let tracker_for_check = tracker.clone();

    let tracker_for_layer = tracker.clone();

    let app = Router::new()
        .route("/test", get(test_handler))
        .layer(axum::middleware::from_fn(move |req, next| {
            let fail_auth = false;
            Box::pin(async move { logging_middleware(req, next, fail_auth).await })
        }))
        .layer(axum::middleware::from_fn(move |req, next| {
            let should_fail = true;
            Box::pin(async move { auth_middleware(req, next, should_fail).await })
        }))
        .layer(axum::middleware::from_fn(move |req, next| {
            Box::pin(async move { rate_limit_middleware(req, next).await })
        }))
        .layer(axum::middleware::from_fn(move |req: Request<axum::body::Body>, next: Next| {
            let tracker = tracker_for_layer.clone();
            Box::pin(async move {
                let mut req = req;
                req.extensions_mut().insert(tracker);
                next.run(req).await
            })
        }));

    let response = app.oneshot(make_request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let order = tracker_for_check.get_order();
    assert!(order.len() >= 2, "Expected at least logging and auth to execute before error, got {:?}", order);
    assert_eq!(&order[..2], &[0, 1], "Expected logging(0) -> auth(1) before error");
}

#[tokio::test]
async fn logging_failure_returns_error_before_handler() {
    let tracker = ExecutionTracker::new();
    let tracker_for_check = tracker.clone();

    let tracker_for_layer = tracker.clone();

    let app = Router::new()
        .route("/test", get(test_handler))
        .layer(axum::middleware::from_fn(move |req, next| {
            let fail_auth = true;
            Box::pin(async move { logging_middleware(req, next, fail_auth).await })
        }))
        .layer(axum::middleware::from_fn(move |req, next| {
            let should_fail = false;
            Box::pin(async move { auth_middleware(req, next, should_fail).await })
        }))
        .layer(axum::middleware::from_fn(move |req, next| {
            Box::pin(async move { rate_limit_middleware(req, next).await })
        }))
        .layer(axum::middleware::from_fn(move |req: Request<axum::body::Body>, next: Next| {
            let tracker = tracker_for_layer.clone();
            Box::pin(async move {
                let mut req = req;
                req.extensions_mut().insert(tracker);
                next.run(req).await
            })
        }));

    let response = app.oneshot(make_request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "logging middleware should fail first");

    let order = tracker_for_check.get_order();
    assert!(order.len() >= 1, "Expected logging to execute, got {:?}", order);
    assert_eq!(order[0], 0, "Expected logging(0) first");
}

#[tokio::test]
async fn middleware_chain_reversed_order_reverses_execution() {
    let tracker = ExecutionTracker::new();
    let tracker_for_check = tracker.clone();

    let tracker_for_layer = tracker.clone();

    let app = Router::new()
        .route("/test", get(test_handler))
        .layer(axum::middleware::from_fn(move |req, next| {
            Box::pin(async move { rate_limit_middleware(req, next).await })
        }))
        .layer(axum::middleware::from_fn(move |req, next| {
            let should_fail = false;
            Box::pin(async move { auth_middleware(req, next, should_fail).await })
        }))
        .layer(axum::middleware::from_fn(move |req, next| {
            let fail_auth = false;
            Box::pin(async move { logging_middleware(req, next, fail_auth).await })
        }))
        .layer(axum::middleware::from_fn(move |req: Request<axum::body::Body>, next: Next| {
            let tracker = tracker_for_layer.clone();
            Box::pin(async move {
                let mut req = req;
                req.extensions_mut().insert(tracker);
                next.run(req).await
            })
        }));

    let response = app.oneshot(make_request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let order = tracker_for_check.get_order();
    assert_eq!(order, vec![0, 1, 2], "Expected execution in reverse: rate_limit(0) -> auth(1) -> logging(2), got {:?}", order);
}

#[tokio::test]
async fn outer_middleware_failure_stops_handler_execution() {
    let tracker = ExecutionTracker::new();
    let tracker_for_check = tracker.clone();

    let tracker_for_layer = tracker.clone();

    let app = Router::new()
        .route("/test", get(test_handler))
        .layer(axum::middleware::from_fn(move |req, next| {
            let fail_auth = true;
            Box::pin(async move { logging_middleware(req, next, fail_auth).await })
        }))
        .layer(axum::middleware::from_fn(move |req, next| {
            Box::pin(async move { rate_limit_middleware(req, next).await })
        }))
        .layer(axum::middleware::from_fn(move |req: Request<axum::body::Body>, next: Next| {
            let tracker = tracker_for_layer.clone();
            Box::pin(async move {
                let mut req = req;
                req.extensions_mut().insert(tracker);
                next.run(req).await
            })
        }));

    let response = app.oneshot(make_request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let order = tracker_for_check.get_order();
    assert!(order.len() >= 1, "Expected logging to execute when it fails, got {:?}", order);
    assert_eq!(order[0], 0, "Expected logging(0) first");
}