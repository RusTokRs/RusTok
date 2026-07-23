//! Framework-neutral server runtime and router bootstrap.

use axum::Router as AxumRouter;

use crate::auth::AuthConfig;
use crate::common::settings::RustokSettings;
use crate::error::Result;
use crate::services::app_lifecycle::connect_runtime_workers_with_runtime;
use crate::services::app_router::compose_application_router;
use crate::services::app_runtime::bootstrap_app_runtime;
use crate::services::cache_runtime::ensure_cache_service;
use crate::services::channel_cache_invalidation::start_channel_cache_invalidation_listener;
use crate::services::rbac_cache_invalidation::start_rbac_cache_invalidation_listener;
use crate::services::rbac_invalidation_generation::start_rbac_invalidation_generation_watchdog;
use crate::services::server_runtime_context::{ServerAuthRuntime, ServerRuntimeContext};

/// Runs host-independent startup validation and one-time initialization.
pub async fn initialize_server_context(
    runtime_ctx: &ServerRuntimeContext,
    jwt_secret: &str,
    database_uri: &str,
) -> Result<()> {
    check_production_secrets(jwt_secret, database_uri)?;
    start_rbac_invalidation_generation_watchdog(runtime_ctx).await?;
    let cache = ensure_cache_service(runtime_ctx);
    start_channel_cache_invalidation_listener(runtime_ctx, cache.clone()).await?;
    start_rbac_cache_invalidation_listener(runtime_ctx, cache).await?;
    crate::initializers::superadmin::ensure_default_superadmin(runtime_ctx).await
}

fn check_production_secrets(jwt_secret: &str, database_uri: &str) -> Result<()> {
    #[cfg(not(debug_assertions))]
    {
        if let Some(fragment) = known_dev_jwt_fragment(jwt_secret) {
            return Err(crate::error::Error::Message(format!(
                "FATAL: JWT secret contains a known development value (\"{fragment}\"). Set a strong, random secret in your production configuration."
            )));
        }
        if !jwt_secret.is_empty() && jwt_secret.len() < 32 {
            return Err(crate::error::Error::Message(
                "FATAL: JWT secret is too short (< 32 characters) for production use. Generate a cryptographically random secret of at least 32 characters.".to_string(),
            ));
        }
        if let Some(pattern) = sample_database_credentials_pattern(database_uri) {
            return Err(crate::error::Error::Message(format!(
                "FATAL: database URI matches known sample credentials ({pattern}). Set production database credentials before starting the release build."
            )));
        }
        if let Some((variable, password)) = configured_superadmin_password() {
            if let Some(sample) = known_sample_superadmin_password(&password) {
                return Err(crate::error::Error::Message(format!(
                    "FATAL: env var {variable} contains sample superadmin password \"{sample}\". Set a unique secret before starting the release build."
                )));
            }
        }
    }

    let _ = (jwt_secret, database_uri);
    Ok(())
}

#[cfg_attr(debug_assertions, allow(dead_code))]
pub(crate) fn known_dev_jwt_fragment(secret: &str) -> Option<&'static str> {
    const KNOWN_DEV_SUBSTRINGS: &[&str] = &[
        "dev-secret",
        "test-secret",
        "change-in-production",
        "dev_secret",
        "rustok-dev-secret",
    ];
    KNOWN_DEV_SUBSTRINGS
        .iter()
        .copied()
        .find(|fragment| secret.contains(fragment))
}

#[cfg_attr(debug_assertions, allow(dead_code))]
pub(crate) fn sample_database_credentials_pattern(uri: &str) -> Option<&'static str> {
    const SAMPLE_PATTERNS: &[&str] = &["://postgres:postgres@", "://rustok:rustok@"];
    SAMPLE_PATTERNS
        .iter()
        .copied()
        .find(|pattern| uri.contains(pattern))
}

#[cfg_attr(debug_assertions, allow(dead_code))]
fn configured_superadmin_password() -> Option<(&'static str, String)> {
    for key in [
        "SUPERADMIN_PASSWORD",
        "SEED_ADMIN_PASSWORD",
        "RUSTOK_DEV_SEED_PASSWORD",
    ] {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim().to_string();
            if !value.is_empty() {
                return Some((key, value));
            }
        }
    }
    None
}

#[cfg_attr(debug_assertions, allow(dead_code))]
pub(crate) fn known_sample_superadmin_password(password: &str) -> Option<&'static str> {
    const SAMPLE_PASSWORDS: &[&str] =
        &["change-me-in-production", "admin12345", "dev-password-123"];
    SAMPLE_PASSWORDS
        .iter()
        .copied()
        .find(|candidate| password == *candidate)
}

/// Builds the fully composed HTTP router from explicit host-owned inputs.
///
/// No framework-global context crosses this boundary. The Axum entrypoint
/// provides these explicit inputs to the single bootstrap path.
pub async fn bootstrap_application_router(
    router: AxumRouter,
    runtime_ctx: ServerRuntimeContext,
    auth_config: AuthConfig,
    settings_snapshot: serde_json::Value,
    rustok_settings: RustokSettings,
) -> Result<AxumRouter> {
    tracing::info!("RusTok application bootstrap started");
    let runtime =
        bootstrap_app_runtime(runtime_ctx.clone(), auth_config.clone(), &rustok_settings).await?;
    tracing::info!("RusTok app runtime bootstrap completed");

    #[cfg(feature = "mod-notifications")]
    crate::services::notification_outbox_intake_worker::start_notification_outbox_intake_if_enabled(
        &runtime_ctx,
    )?;

    #[cfg(feature = "mod-notifications")]
    crate::services::notification_candidate_worker::start_notification_candidate_worker_if_ready(
        &runtime_ctx,
    )?;

    connect_runtime_workers_with_runtime(runtime_ctx.clone()).await?;
    tracing::info!("RusTok runtime workers connected");

    let router = compose_application_router(
        router,
        runtime_ctx.clone(),
        ServerAuthRuntime::new(runtime_ctx, auth_config),
        settings_snapshot,
        runtime,
        &rustok_settings,
    )?;
    tracing::info!("RusTok application router composed");
    Ok(router)
}
