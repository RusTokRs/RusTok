//! Pure Axum executable host composition.

use std::{path::PathBuf, time::Duration};

use axum::Router;
use sea_orm::{ConnectOptions, Database};
use serde::Deserialize;

use crate::{
    channels,
    common::settings::RustokSettings,
    controllers,
    error::{Error, Result},
    middleware::security_headers::hsts_enabled,
    routes::ServerRouter,
    services::{
        app_lifecycle::{resolve_boot_database_uri, shutdown_runtime_workers},
        server_bootstrap::{bootstrap_application_router, initialize_server_context},
        server_runtime_context::{ServerAuthRuntime, ServerRuntimeContext},
    },
};

const DEFAULT_JWT_ISSUER: &str = "rustok";
const DEFAULT_JWT_AUDIENCE: &str = "rustok-admin";
const MIN_PRODUCTION_HS256_SECRET_BYTES: usize = 64;

#[derive(Debug, Deserialize)]
struct HostConfig {
    server: ServerConfig,
    database: DatabaseConfig,
    auth: AuthConfig,
    #[serde(default)]
    settings: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ServerConfig {
    binding: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct DatabaseConfig {
    uri: String,
    #[serde(default)]
    enable_logging: bool,
    #[serde(default)]
    connect_timeout: u64,
    #[serde(default)]
    idle_timeout: u64,
    #[serde(default)]
    min_connections: u32,
    #[serde(default)]
    max_connections: u32,
}

#[derive(Debug, Deserialize)]
struct AuthConfig {
    jwt: JwtConfig,
}

#[derive(Debug, Deserialize)]
struct JwtConfig {
    secret: String,
    expiration: u64,
}

/// Starts the HTTP-only Axum host.
pub async fn run() -> Result<()> {
    let config = load_config()?;
    let database_uri = resolve_database_uri(&config.database.uri);
    let db = connect_database(&config.database, &database_uri).await?;
    let rustok_settings = RustokSettings::from_settings(&Some(config.settings.clone()))
        .map_err(|error| Error::BadRequest(format!("Invalid rustok settings: {error}")))?;
    let production = is_production_environment();
    validate_https_deployment(production, hsts_enabled())?;
    let runtime_ctx = ServerRuntimeContext::new(db, rustok_settings.clone());
    let auth_config = crate::auth::auth_config_from_host_settings(
        config.auth.jwt.secret.clone(),
        config.auth.jwt.expiration,
        Some(&config.settings),
    )?;
    validate_auth_deployment(&auth_config, production)?;

    initialize_server_context(&runtime_ctx, &config.auth.jwt.secret, &database_uri).await?;

    let router = application_router(rustok_settings.runtime.host_mode).with_state(
        ServerAuthRuntime::new(runtime_ctx.clone(), auth_config.clone()),
    );
    let router = bootstrap_application_router(
        router,
        runtime_ctx.clone(),
        auth_config,
        config.settings,
        rustok_settings,
    )
    .await?;

    let listener =
        tokio::net::TcpListener::bind((config.server.binding.as_str(), config.server.port)).await?;
    let address = listener.local_addr()?;
    tracing::info!(%address, "RusTok Axum host listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal(runtime_ctx))
        .await
        .map_err(Error::Io)
}

fn validate_https_deployment(production: bool, https_declared: bool) -> Result<()> {
    if production && !https_declared {
        return Err(Error::BadRequest(
            "RUSTOK_HTTPS must be set to true for production deployments so HSTS is emitted"
                .to_string(),
        ));
    }

    Ok(())
}

fn validate_auth_deployment(config: &crate::auth::AuthConfig, production: bool) -> Result<()> {
    if !production {
        return Ok(());
    }

    if config.issuer == DEFAULT_JWT_ISSUER || !config.issuer.starts_with("https://") {
        return Err(Error::BadRequest(
            "Production JWT issuer must be an explicit HTTPS identifier and must not use the `rustok` default"
                .to_string(),
        ));
    }

    if config.audience == DEFAULT_JWT_AUDIENCE
        || config.audience.len() < 8
        || config.audience.chars().any(char::is_whitespace)
    {
        return Err(Error::BadRequest(
            "Production JWT audience must be explicit, contain at least 8 characters, contain no whitespace, and must not use the `rustok-admin` default"
                .to_string(),
        ));
    }

    match config.algorithm {
        crate::auth::JwtAlgorithm::HS256 => {
            if config.secret.len() < MIN_PRODUCTION_HS256_SECRET_BYTES {
                return Err(Error::BadRequest(format!(
                    "Production HS256 secret must contain at least {MIN_PRODUCTION_HS256_SECRET_BYTES} bytes"
                )));
            }
            if jwt_secret_looks_like_placeholder(&config.secret) {
                return Err(Error::BadRequest(
                    "Production HS256 secret looks like a placeholder or low-diversity value"
                        .to_string(),
                ));
            }
        }
        crate::auth::JwtAlgorithm::RS256 => {
            if config
                .rsa_private_key_pem
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
                || config
                    .rsa_public_key_pem
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
            {
                return Err(Error::BadRequest(
                    "Production RS256 requires non-empty private and public key material"
                        .to_string(),
                ));
            }
        }
    }

    Ok(())
}

fn jwt_secret_looks_like_placeholder(secret: &str) -> bool {
    let normalized = secret.trim().to_ascii_lowercase();
    const PLACEHOLDER_MARKERS: &[&str] = &[
        "change_me",
        "changeme",
        "replace_me",
        "replace-this",
        "do-not-use",
        "development",
        "example-secret",
        "test-secret",
    ];

    PLACEHOLDER_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
        || normalized
            .chars()
            .collect::<std::collections::HashSet<_>>()
            .len()
            < 16
}

fn is_production_environment() -> bool {
    ["RUSTOK_ENV", "RUST_ENV", "APP_ENV"].iter().any(|key| {
        std::env::var(key)
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "prod" | "production"
                )
            })
            .unwrap_or(false)
    })
}

fn application_router(host_mode: crate::common::settings::RuntimeHostMode) -> ServerRouter {
    let router = Router::new()
        .merge(controllers::health::router())
        .merge(controllers::metrics::router());

    if host_mode == crate::common::settings::RuntimeHostMode::Worker {
        return router;
    }

    let router = router.merge(controllers::swagger::router());
    if host_mode == crate::common::settings::RuntimeHostMode::RegistryOnly {
        return router.merge(controllers::marketplace_registry::read_only_router());
    }

    router
        .merge(controllers::marketplace_registry::router())
        .merge(controllers::artifact_http::router())
        .merge(controllers::artifact_permissions::router())
        .merge(controllers::admin_events::router())
        .merge(controllers::auth::router())
        .merge(controllers::channel::router())
        .merge(controllers::flex::router())
        .merge(controllers::graphql::router())
        .merge(controllers::installer::router())
        .merge(controllers::mcp::router())
        .merge(controllers::oauth::router())
        .merge(controllers::oauth_metadata::router())
        .merge(controllers::users::router())
        .merge(channels::builds::router())
}

fn resolve_database_uri(configured_uri: &str) -> String {
    match resolve_boot_database_uri(std::env::var("DATABASE_URL").is_ok(), configured_uri) {
        Some(uri) => {
            tracing::info!(
                database_uri = uri,
                "No external database found; using local SQLite"
            );
            uri.to_string()
        }
        None => std::env::var("DATABASE_URL").unwrap_or_else(|_| configured_uri.to_string()),
    }
}

async fn connect_database(
    config: &DatabaseConfig,
    uri: &str,
) -> Result<sea_orm::DatabaseConnection> {
    let mut options = ConnectOptions::new(uri.to_string());
    options.sqlx_logging(config.enable_logging);
    if config.connect_timeout > 0 {
        options.connect_timeout(Duration::from_millis(config.connect_timeout));
    }
    if config.idle_timeout > 0 {
        options.idle_timeout(Duration::from_millis(config.idle_timeout));
    }
    if config.min_connections > 0 {
        options.min_connections(config.min_connections);
    }
    if config.max_connections > 0 {
        options.max_connections(config.max_connections);
    }
    Database::connect(options).await.map_err(Error::Database)
}

fn load_config() -> Result<HostConfig> {
    let environment = std::env::var("RUSTOK_ENV")
        .or_else(|_| std::env::var("APP_ENV"))
        .unwrap_or_else(|_| "development".to_string());
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config")
        .join(format!("{environment}.yaml"));
    let raw = std::fs::read_to_string(&path)?;
    serde_yaml::from_str(&raw).map_err(Error::Yaml)
}

async fn shutdown_signal(runtime_ctx: ServerRuntimeContext) {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!(error = %error, "failed to receive shutdown signal");
    }
    shutdown_runtime_workers(&runtime_ctx).await;
    tracing::info!("RusTok Axum host shut down cleanly");
}

#[cfg(test)]
mod tests {
    use super::{
        application_router, resolve_database_uri, validate_auth_deployment,
        validate_https_deployment,
    };
    use crate::common::settings::RuntimeHostMode;

    #[test]
    fn registry_only_router_is_composable_without_domain_routes() {
        let _ = application_router(RuntimeHostMode::RegistryOnly);
    }

    #[test]
    fn worker_router_is_composable_without_http_application_surfaces() {
        let _ = application_router(RuntimeHostMode::Worker);
    }

    #[test]
    fn explicit_database_url_overrides_local_development_default() {
        unsafe {
            std::env::set_var("DATABASE_URL", "sqlite::memory:");
        }
        assert_eq!(
            resolve_database_uri("postgres://localhost/rustok"),
            "sqlite::memory:"
        );
        unsafe {
            std::env::remove_var("DATABASE_URL");
        }
    }

    #[test]
    fn production_requires_explicit_https_declaration() {
        let error = validate_https_deployment(true, false)
            .expect_err("production without HSTS declaration must fail closed");
        assert!(error.to_string().contains("RUSTOK_HTTPS"));
    }

    #[test]
    fn non_production_can_run_without_hsts() {
        validate_https_deployment(false, false).expect("local development may use plaintext HTTP");
    }

    #[test]
    fn production_accepts_https_declaration() {
        validate_https_deployment(true, true)
            .expect("production HTTPS declaration enables the HSTS contract");
    }

    #[test]
    fn non_production_allows_default_jwt_claims() {
        let config = crate::auth::AuthConfig::new(
            "development-secret-value-with-more-than-thirty-two-bytes".to_string(),
        );
        validate_auth_deployment(&config, false)
            .expect("development may use the framework claim defaults");
    }

    #[test]
    fn production_rejects_default_jwt_claims() {
        let config = crate::auth::AuthConfig::new("aB3!".repeat(20));
        let error = validate_auth_deployment(&config, true)
            .expect_err("production must not share the framework claim namespace");
        assert!(error.to_string().contains("issuer"));
    }

    #[test]
    fn production_rejects_short_hs256_secret() {
        let config = crate::auth::AuthConfig::new("aB3!".repeat(10))
            .with_issuer("https://api.example.com")
            .with_audience("rustok-production-admin");
        let error = validate_auth_deployment(&config, true)
            .expect_err("production HS256 needs a stronger secret floor");
        assert!(error.to_string().contains("at least 64 bytes"));
    }

    #[test]
    fn production_accepts_explicit_strong_hs256_policy() {
        let config = crate::auth::AuthConfig::new("aB3!zY7@qW8#eR2$".repeat(5))
            .with_issuer("https://api.example.com")
            .with_audience("rustok-production-admin");
        validate_auth_deployment(&config, true)
            .expect("explicit claims and a high-entropy secret satisfy production policy");
    }
}
