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
use crate::services::product_catalog_deployment::configure_product_catalog_deployment;
#[cfg(feature = "mod-product")]
use crate::services::product_index_refresh_worker;
use crate::services::profile_media_public_image_deployment::configure_profile_media_public_image_deployment;
use crate::services::rbac_cache_invalidation::start_rbac_cache_invalidation_listener;
use crate::services::rbac_invalidation_generation::start_rbac_invalidation_generation_watchdog;
use crate::services::server_runtime_context::{ServerAuthRuntime, ServerRuntimeContext};

/// Runs host-independent startup validation and one-time initialization.
pub async fn initialize_server_context(
    runtime_ctx: &ServerRuntimeContext,
    jwt_secret: &str,
    database_uri: &str,
) -> Result<()> {
    check_production_secrets(
        jwt_secret,
        database_uri,
        crate::common::is_production_environment() || !cfg!(debug_assertions),
    )?;
    start_rbac_invalidation_generation_watchdog(runtime_ctx).await?;
    let cache = ensure_cache_service(runtime_ctx);
    start_channel_cache_invalidation_listener(runtime_ctx, cache.clone()).await?;
    start_rbac_cache_invalidation_listener(runtime_ctx, cache).await?;
    crate::initializers::superadmin::ensure_default_superadmin(runtime_ctx).await
}

fn check_production_secrets(
    jwt_secret: &str,
    database_uri: &str,
    production: bool,
) -> Result<()> {
    if !production {
        return Ok(());
    }

    if known_dev_jwt_fragment(jwt_secret).is_some() {
        return Err(crate::error::Error::Message(
            "FATAL: JWT secret contains a known development value. Set a strong, random secret in the production configuration."
                .to_string(),
        ));
    }

    if jwt_secret.is_empty() || jwt_secret.len() < 32 {
        return Err(crate::error::Error::Message(
            "FATAL: JWT secret is too short for production use. Generate a cryptographically random secret of at least 32 bytes."
                .to_string(),
        ));
    }

    if sample_database_credentials_pattern(database_uri).is_some() {
        return Err(crate::error::Error::Message(
            "FATAL: database URI matches known sample credentials. Set production database credentials before starting the release build."
                .to_string(),
        ));
    }

    if let Some((variable, password)) = configured_superadmin_password() {
        if known_sample_superadmin_password(&password).is_some() {
            return Err(crate::error::Error::Message(format!(
                "FATAL: env var {variable} contains a known sample superadmin password. Set a unique secret before starting the release build."
            )));
        }
    }

    Ok(())
}

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

pub(crate) fn sample_database_credentials_pattern(uri: &str) -> Option<&'static str> {
    const SAMPLE_PATTERNS: &[&str] = &["://postgres:postgres@", "://rustok:rustok@"];
    SAMPLE_PATTERNS
        .iter()
        .copied()
        .find(|pattern| uri.contains(pattern))
}

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
    configure_product_catalog_deployment(&runtime_ctx).await?;
    configure_profile_media_public_image_deployment(&runtime_ctx).await?;
    let runtime =
        bootstrap_app_runtime(runtime_ctx.clone(), auth_config.clone()).await?;
    tracing::info!("RusTok app runtime bootstrap completed");

    let router = compose_application_router(
        router,
        runtime_ctx.clone(),
        ServerAuthRuntime::new(runtime_ctx.clone(), auth_config),
        settings_snapshot,
        runtime,
        &rustok_settings,
    )?;
    tracing::info!("RusTok application router composed");

    #[cfg(feature = "mod-comments")]
    crate::services::comments_provider_runtime::start_comments_tcp_listener_if_enabled(
        &runtime_ctx,
    )
    .await?;

    #[cfg(feature = "mod-comments")]
    crate::services::comments_provider_runtime::start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled(
        &runtime_ctx,
    )?;

    crate::services::event_dlq_duplicate_alert_observer::start_event_dlq_duplicate_alert_observer(
        &runtime_ctx,
    )
    .await;
    crate::services::event_dlq_duplicate_alert_observability::start_event_dlq_duplicate_alert_observability(
        &runtime_ctx,
    );

    #[cfg(feature = "mod-notifications")]
    crate::services::notification_outbox_intake_worker::start_notification_outbox_intake_if_enabled(
        &runtime_ctx,
    )?;

    #[cfg(feature = "mod-notifications")]
    crate::services::notification_fanout_worker::start_notification_fanout_worker_if_ready(
        &runtime_ctx,
    )?;

    #[cfg(feature = "mod-notifications")]
    crate::services::notification_candidate_worker::start_notification_candidate_worker_if_ready(
        &runtime_ctx,
    )?;

    crate::services::search_product_channel_reconciliation::start_product_channel_projection_reconciliation_if_ready(
        &runtime_ctx,
    )?;

    #[cfg(feature = "mod-forum")]
    crate::services::forum_search_inbox_worker::start_forum_search_inbox_worker_if_ready(
        &runtime_ctx,
    )?;

    #[cfg(feature = "mod-forum")]
    crate::services::forum_search_inbox_worker::start_forum_search_contract_consumer_if_enabled(
        &runtime_ctx,
    )
    .await?;

    #[cfg(feature = "mod-product")]
    product_index_refresh_worker::start_product_index_refresh_worker_if_enabled(&runtime_ctx)
        .await?;

    #[cfg(feature = "mod-social_graph")]
    crate::services::social_graph_index_worker::start_social_graph_index_worker_if_enabled(
        &runtime_ctx,
    )
    .await?;

    #[cfg(feature = "mod-social_graph")]
    crate::services::social_graph_index_position_observer::start_social_graph_index_position_observer_if_enabled(
        &runtime_ctx,
    )
    .await?;

    #[cfg(feature = "mod-social_graph")]
    crate::services::social_graph_index_poison_observer::start_social_graph_index_poison_observer_if_enabled(
        &runtime_ctx,
    )
    .await?;

    connect_runtime_workers_with_runtime(runtime_ctx.clone()).await?;
    tracing::info!("RusTok runtime workers connected");
    Ok(router)
}


#[cfg(test)]
mod tests {
    use super::{
        check_production_secrets, known_dev_jwt_fragment, known_sample_superadmin_password,
        sample_database_credentials_pattern,
    };

    #[test]
    fn production_secret_validation_is_disabled_outside_production() {
        check_production_secrets("dev-secret", "postgres://postgres:postgres@db/rustok", false)
            .expect("non-production startup may use development values");
    }

    #[test]
    fn production_secret_validation_rejects_empty_jwt_secret_without_exposing_it() {
        let error = check_production_secrets("", "postgres://app:strong@db/rustok", true)
            .expect_err("production must reject an empty JWT secret");
        let message = error.to_string();
        assert!(message.contains("JWT secret is too short"));
    }

    #[test]
    fn production_secret_validation_redacts_development_jwt_fragment() {
        let secret = "prefix-dev-secret-suffix";
        let error = check_production_secrets(secret, "postgres://app:strong@db/rustok", true)
            .expect_err("known development JWT material must be rejected");
        let message = error.to_string();
        assert!(message.contains("known development value"));
        assert!(!message.contains("dev-secret"));
    }

    #[test]
    fn production_secret_validation_redacts_database_sample_credentials() {
        let uri = "postgres://postgres:postgres@db/rustok";
        let error = check_production_secrets(
            "aB3!zY7@qW8#eR2$".repeat(5).as_str(),
            uri,
            true,
        )
        .expect_err("sample database credentials must be rejected");
        let message = error.to_string();
        assert!(message.contains("known sample credentials"));
        assert!(!message.contains("postgres:postgres"));
    }

    #[test]
    fn sample_superadmin_matcher_identifies_sample_values_without_log_disclosure() {
        assert_eq!(
            known_sample_superadmin_password("change-me-in-production"),
            Some("change-me-in-production")
        );
    }

    #[test]
    fn helper_matchers_still_identify_known_samples_without_disclosing_them() {
        assert_eq!(known_dev_jwt_fragment("dev-secret-value"), Some("dev-secret"));
        assert_eq!(
            sample_database_credentials_pattern("postgres://postgres:postgres@db/rustok"),
            Some("://postgres:postgres@")
        );
        assert_eq!(
            known_sample_superadmin_password("dev-password-123"),
            Some("dev-password-123")
        );
    }
}
