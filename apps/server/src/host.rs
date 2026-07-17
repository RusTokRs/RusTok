//! Pure Axum executable host composition.

use std::{path::PathBuf, time::Duration};

use axum::Router;
use sea_orm::{ConnectOptions, Database};
use serde::Deserialize;

use crate::{
    channels,
    common::settings::{RustokSettings, TenantFallbackMode},
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
    validate_tenant_routing_settings(&rustok_settings, production)?;
    validate_https_deployment(production, hsts_enabled())?;
    let runtime_ctx = ServerRuntimeContext::new(db, rustok_settings.clone());
    let auth_config = crate::auth::auth_config_from_host_settings(
        config.auth.jwt.secret.clone(),
        config.auth.jwt.expiration,
        Some(&config.settings),
    )?;

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

fn validate_tenant_routing_settings(
    settings: &RustokSettings,
    production: bool,
) -> Result<()> {
    if !settings.tenant.enabled {
        return Ok(());
    }

    match settings.tenant.resolution.as_str() {
        "header" | "host" | "domain" | "subdomain" => {}
        invalid => {
            return Err(Error::BadRequest(format!(
                "Invalid rustok.tenant.resolution `{invalid}`; expected `header`, `host`, `domain`, or `subdomain`"
            )));
        }
    }

    if settings.tenant.resolution == "subdomain" && settings.tenant.base_domains.is_empty() {
        return Err(Error::BadRequest(
            "rustok.tenant.base_domains must contain at least one domain when resolution=subdomain"
                .to_string(),
        ));
    }

    if matches!(
        settings.tenant.fallback_mode,
        TenantFallbackMode::DefaultTenant
    ) {
        if settings.tenant.resolution != "header" {
            return Err(Error::BadRequest(
                "rustok.tenant.fallback_mode=default_tenant is only valid with resolution=header"
                    .to_string(),
            ));
        }

        if production {
            return Err(Error::BadRequest(
                "rustok.tenant.fallback_mode=default_tenant is forbidden in production; use strict tenant identification"
                    .to_string(),
            ));
        }
    }

    Ok(())
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

fn is_production_environment() -> bool {
    ["RUST_ENV", "APP_ENV"].iter().any(|key| {
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
        application_router, resolve_database_uri, validate_https_deployment,
        validate_tenant_routing_settings,
    };
    use crate::common::settings::{RuntimeHostMode, RustokSettings, TenantFallbackMode};

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
    fn tenant_routing_rejects_unknown_resolution() {
        let mut settings = RustokSettings::default();
        settings.tenant.resolution = "automatic".to_string();

        let error = validate_tenant_routing_settings(&settings, false)
            .expect_err("unknown tenant resolution must fail closed");
        assert!(error.to_string().contains("Invalid rustok.tenant.resolution"));
    }

    #[test]
    fn tenant_routing_rejects_default_fallback_in_production() {
        let mut settings = RustokSettings::default();
        settings.tenant.resolution = "header".to_string();
        settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;

        let error = validate_tenant_routing_settings(&settings, true)
            .expect_err("production must not silently select the default tenant");
        assert!(error.to_string().contains("forbidden in production"));
    }

    #[test]
    fn tenant_routing_rejects_default_fallback_outside_header_mode() {
        let mut settings = RustokSettings::default();
        settings.tenant.resolution = "host".to_string();
        settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;

        let error = validate_tenant_routing_settings(&settings, false)
            .expect_err("fallback must only apply to header resolution");
        assert!(error.to_string().contains("only valid with resolution=header"));
    }

    #[test]
    fn tenant_routing_requires_base_domains_for_subdomain_mode() {
        let mut settings = RustokSettings::default();
        settings.tenant.resolution = "subdomain".to_string();
        settings.tenant.base_domains.clear();

        let error = validate_tenant_routing_settings(&settings, false)
            .expect_err("subdomain routing without a base domain is ambiguous");
        assert!(error.to_string().contains("base_domains"));
    }

    #[test]
    fn tenant_routing_allows_strict_header_mode() {
        let settings = RustokSettings::default();
        validate_tenant_routing_settings(&settings, true)
            .expect("strict header routing is valid in production");
    }

    #[test]
    fn production_requires_explicit_https_declaration() {
        let error = validate_https_deployment(true, false)
            .expect_err("production without HSTS declaration must fail closed");
        assert!(error.to_string().contains("RUSTOK_HTTPS"));
    }

    #[test]
    fn non_production_can_run_without_hsts() {
        validate_https_deployment(false, false)
            .expect("local development may use plaintext HTTP");
    }

    #[test]
    fn production_accepts_https_declaration() {
        validate_https_deployment(true, true)
            .expect("production HTTPS declaration enables the HSTS contract");
    }
}
