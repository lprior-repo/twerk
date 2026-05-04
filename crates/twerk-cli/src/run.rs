//! Run command
//!
//! Runs the Twerk engine in the specified mode.
//!
//! Go parity: `cli/run.go` → parses mode arg, calls `engine.SetMode(mode)` then `engine.Run()`.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::net::TcpListener;
use twerk_app::engine::coordinator::auth::KeyAuthConfig;
use twerk_app::engine::coordinator::limits::{BodyLimitConfig, RateLimitConfig};
use twerk_app::engine::coordinator::middleware::HttpLogConfig;
use twerk_app::engine::{Config, Engine, Mode};
use twerk_common::load_config;
use twerk_infrastructure::config as app_config;
use twerk_web::api::{create_router, AppState, Config as ApiConfig};

use crate::commands::RunMode;
use crate::CliError;
use tracing::info;

impl RunMode {
    /// Convert CLI run mode into engine mode.
    #[must_use]
    pub const fn into_engine_mode(self) -> Mode {
        match self {
            Self::Standalone => Mode::Standalone,
            Self::Coordinator => Mode::Coordinator,
            Self::Worker => Mode::Worker,
        }
    }
}

/// Run the Twerk engine in the specified mode.
///
/// Go parity:
/// ```go
/// mode := ctx.Args().First()
/// engine.SetMode(engine.Mode(mode))
/// return engine.Run()
/// ```
///
/// # Arguments
///
/// * `mode` - The engine mode to run in (standalone, coordinator, worker)
/// * `hostname` - Optional hostname for coordinator
///
/// # Errors
///
/// Returns [`CliError::Engine`] if the engine fails to start or run.
/// Returns [`CliError::InvalidHostname`] if hostname format is invalid.
pub async fn run_engine(mode: RunMode, hostname: Option<String>) -> Result<(), CliError> {
    let _ = load_config();

    if let Some(ref host) = hostname {
        validate_hostname(host)?;
    }

    let engine_mode = mode.clone().into_engine_mode();

    info!("Starting Twerk engine in {engine_mode:?} mode");

    let config = Config {
        mode: engine_mode,
        hostname,
        ..Config::default()
    };

    let mut engine = Engine::new(config);
    engine
        .start()
        .await
        .map_err(|e| CliError::Engine(e.to_string()))?;

    let api_server = start_api_server(&engine, mode)
        .await
        .map_err(|e| CliError::Engine(e.to_string()))?;

    engine.await_shutdown().await;

    if let Some(handle) = api_server {
        handle.abort();
    }

    Ok(())
}

fn validate_hostname(hostname: &str) -> Result<(), CliError> {
    if hostname.is_empty() {
        return Err(CliError::InvalidHostname(
            "hostname cannot be empty".to_string(),
        ));
    }
    if hostname.contains("://") {
        return Err(CliError::InvalidHostname(
            "hostname cannot contain scheme".to_string(),
        ));
    }
    if hostname.contains(':') {
        return Err(CliError::InvalidHostname(
            "hostname cannot contain port".to_string(),
        ));
    }
    Ok(())
}

const fn api_enabled(mode: RunMode) -> bool {
    matches!(mode, RunMode::Coordinator | RunMode::Standalone)
}

fn read_api_config() -> ApiConfig {
    let endpoints = app_config::bool_map("coordinator.api.endpoints");
    let enabled = if endpoints.is_empty() {
        HashMap::new()
    } else {
        endpoints
            .into_iter()
            .map(|(key, value)| (key.replace("endpoints.", ""), value))
            .collect()
    };

    let rate_limit = app_config::bool("middleware.web.ratelimit.enabled").then(|| {
        RateLimitConfig::new(
            app_config::int_default("middleware.web.ratelimit.rps", 20) as u32,
            app_config::int_default("middleware.web.ratelimit.burst", 5) as u32,
        )
    });

    let key_auth = app_config::bool("middleware.web.keyauth.enabled")
        .then(|| KeyAuthConfig::new(app_config::string_default("middleware.web.keyauth.key", "")));

    let basic_auth = app_config::bool("middleware.web.basicauth.enabled")
        .then(|| {
            // Note: BasicAuthConfig needs a datastore handle which we don't have here.
            // For CLI startup, basic auth is handled differently via the router factory.
            // Leaving as None for now; basic auth can be enabled via the engine config.
            None
        })
        .flatten();

    let body_limit = app_config::string_default("middleware.web.bodylimit", "500K")
        .parse::<usize>()
        .ok()
        .map(BodyLimitConfig::new);

    let http_log = app_config::bool_default("middleware.web.logger.enabled", true).then(|| {
        HttpLogConfig::new(
            app_config::string_default("middleware.web.logger.level", "DEBUG"),
            app_config::strings_default("middleware.web.logger.skip", &["GET /health"]),
        )
    });

    ApiConfig {
        address: app_config::string_default("coordinator.address", "0.0.0.0:8000"),
        enabled,
        cors_origins: app_config::strings("middleware.web.cors.origins"),
        basic_auth,
        key_auth,
        rate_limit,
        body_limit,
        http_log,
    }
}

async fn start_api_server(
    engine: &Engine,
    mode: RunMode,
) -> Result<Option<tokio::task::JoinHandle<()>>, anyhow::Error> {
    if !api_enabled(mode) {
        return Ok(None);
    }

    let api_config = read_api_config();
    let address = api_config.address.clone();
    let state = AppState::new(
        Arc::new(engine.broker_proxy()),
        Arc::new(engine.datastore_proxy()),
        api_config,
    );
    let app = create_router(state);
    let listener = TcpListener::bind(&address).await?;

    info!("Coordinator API listening on http://{address}");

    Ok(Some(tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            tracing::error!(error = %error, "coordinator API server failed");
        }
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_mode_maps_standalone() {
        assert_eq!(RunMode::Standalone.into_engine_mode(), Mode::Standalone);
    }

    #[test]
    fn test_run_mode_maps_coordinator() {
        assert_eq!(RunMode::Coordinator.into_engine_mode(), Mode::Coordinator);
    }

    #[test]
    fn test_run_mode_maps_worker() {
        assert_eq!(RunMode::Worker.into_engine_mode(), Mode::Worker);
    }
}
