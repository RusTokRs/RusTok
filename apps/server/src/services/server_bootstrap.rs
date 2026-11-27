//! Framework-neutral server runtime and router bootstrap.

use axum::Router as AxumRouter;

use crate::auth::AuthConfig;
use crate::common::settings::RustokSettings;
use crate::error::Result;
use crate::services::app_lifecycle::{
    ServerBootstrapInitializationLock, connect_runtime_workers_with_runtime,
};
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
use crate::services::stop_handle::{StopHandle, await_worker_startup_or_stop};

/// Runs host-independent startup validation and one-time initialization.
pub async fn initialize_server_context(
    runtime_ctx: &ServerRuntimeContext,
    mac_root: &str,
    database_uri: &str,
) -> Result<()> {
    let stop = StopHandle::ensure(runtime_ctx);
    let initialization = ServerBootstrapInitializationLock::ensure(runtime_ctx);
    let Some(_initialization_guard) =
        await_worker_startup_or_stop(&stop, initialization.lock()).await
    else {
        return Err(crate::error::Error::Message(
            "server shutdown started before runtime initialization".to_string(),
        ));
    };
    check_production_environment(
        mac_root,
        database_uri,
        crate::common::is_production_security_mode(),
    )?;
    // Initialize the Argon2 dummy credential before accepting traffic. The
    // first unknown-email login must not pay a one-time hash-generation cost
    // that would itself reveal account existence.
    crate::auth::dummy_password_hash_async().await?;
    start_rbac_invalidation_generation_watchdog(runtime_ctx).await?;
    let cache = ensure_cache_service(runtime_ctx);
    start_channel_cache_invalidation_listener(runtime_ctx, cache.clone()).await?;
    start_rbac_cache_invalidation_listener(runtime_ctx, cache).await?;
    // The listener starters return Ok when stop cancels their unpublished
    // startup. That is not permission to begin durable account provisioning.
    if stop.is_stopping() {
        return Err(crate::error::Error::Message(
            "server shutdown started before default SuperAdmin initialization".to_string(),
        ));
    }
    if should_initialize_default_superadmin(runtime_ctx) {
        crate::initializers::superadmin::ensure_default_superadmin(runtime_ctx).await?;
    } else {
        tracing::info!(
            host_mode = ?runtime_ctx.settings().runtime.host_mode,
            "Skipping default SuperAdmin provisioning for a host mode without an administrative surface"
        );
    }
    if stop.is_stopping() {
        return Err(crate::error::Error::Message(
            "server shutdown started during runtime initialization".to_string(),
        ));
    }
    Ok(())
}

fn should_initialize_default_superadmin(runtime_ctx: &ServerRuntimeContext) -> bool {
    should_initialize_default_superadmin_for_mode(runtime_ctx.settings().runtime.host_mode)
}

fn should_initialize_default_superadmin_for_mode(
    host_mode: crate::common::settings::RuntimeHostMode,
) -> bool {
    matches!(
        host_mode,
        crate::common::settings::RuntimeHostMode::Full
            | crate::common::settings::RuntimeHostMode::Api
            | crate::common::settings::RuntimeHostMode::AdminSsr
    )
}

pub(crate) fn check_production_environment(
    mac_root: &str,
    database_uri: &str,
    production: bool,
) -> Result<()> {
    if !production {
        return Ok(());
    }

    let trimmed_mac_root = mac_root.trim();
    if known_dev_mac_root_fragment(trimmed_mac_root).is_some() {
        return Err(crate::error::Error::Message(
            "FATAL: MAC root contains a known development value. Set an independently generated random MAC root in the production configuration."
                .to_string(),
        ));
    }

    let mac_root_bytes = mac_root
        .chars()
        .filter(|character| !character.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    if mac_root_bytes < 64 {
        return Err(crate::error::Error::Message(
            "FATAL: MAC root must contain at least 64 non-whitespace bytes for production use."
                .to_string(),
        ));
    }

    let trimmed_uri = database_uri.trim();
    if trimmed_uri.is_empty() {
        return Err(crate::error::Error::Message(
            "FATAL: database URI must not be empty in production.".to_string(),
        ));
    }

    if sample_database_credentials_pattern(trimmed_uri).is_some() {
        return Err(crate::error::Error::Message(
            "FATAL: database URI matches known sample credentials. Set production database credentials before starting the release build."
                .to_string(),
        ));
    }

    match std::env::var("RUSTOK_DEV_SEED_PASSWORD") {
        Ok(value) if !value.trim().is_empty() => {
            return Err(crate::error::Error::Message(
                concat!(
                    "FATAL: RUSTOK_DEV_SEED_PASSWORD is development-only; ",
                    "use SUPERADMIN_PASSWORD or SEED_ADMIN_PASSWORD for production provisioning."
                )
                .to_string(),
            ));
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(crate::error::Error::Message(
                "FATAL: RUSTOK_DEV_SEED_PASSWORD must be valid UTF-8 in production."
                    .to_string(),
            ));
        }
        _ => {}
    }

    if let Some((variable, password)) = configured_superadmin_password() {
        let trimmed_password = password.trim();
        if known_sample_superadmin_password(trimmed_password).is_some()
            || trimmed_password.chars().count() < crate::auth::DEFAULT_MIN_PASSWORD_LENGTH_CHARS
        {
            return Err(crate::error::Error::Message(format!(
                "FATAL: env var {variable} contains a weak or known sample superadmin password. Set a unique secret of at least {} characters before starting the release build.",
                crate::auth::DEFAULT_MIN_PASSWORD_LENGTH_CHARS
            )));
        }
    }

    Ok(())
}

pub(crate) fn known_placeholder_fragment(value: &str) -> Option<&'static str> {
    const PLACEHOLDER_FRAGMENTS: &[&str] = &[
        "change_me",
        "change-me",
        "changeme",
        "replace_me",
        "replace-me",
        "placeholder",
    ];
    let normalized = value.trim().to_ascii_lowercase();
    PLACEHOLDER_FRAGMENTS
        .iter()
        .copied()
        .find(|fragment| normalized.contains(fragment))
}

pub(crate) fn known_dev_mac_root_fragment(mac_root: &str) -> Option<&'static str> {
    const KNOWN_DEV_SUBSTRINGS: &[&str] = &[
        "dev-secret",
        "test-secret",
        "change-in-production",
        "dev_secret",
        "rustok-dev-secret",
    ];
    known_placeholder_fragment(mac_root).or_else(|| {
        let normalized = mac_root.trim().to_ascii_lowercase();
        KNOWN_DEV_SUBSTRINGS
            .iter()
            .copied()
            .find(|fragment| normalized.contains(fragment))
    })
}

pub(crate) fn sample_database_credentials_pattern(uri: &str) -> Option<&'static str> {
    const SAMPLE_PATTERNS: &[&str] = &[
        "://postgres:postgres@",
        "://rustok:rustok@",
        "://admin:admin@",
        "://root:root@",
        "://user:password@",
        "user:password",
        "sqlite:",
        "localhost:5432/rustok_dev",
        "127.0.0.1:5432/rustok_dev",
        ".example.com",
        ".example.net",
        ".example.org",
        ".example",
        ".invalid",
        ".test",
    ];
    let normalized = uri.trim().to_ascii_lowercase();
    known_placeholder_fragment(&normalized).or_else(|| {
        SAMPLE_PATTERNS
            .iter()
            .copied()
            .find(|pattern| normalized.contains(pattern))
    })
}

fn configured_superadmin_password() -> Option<(&'static str, String)> {
    for key in [
        "SUPERADMIN_PASSWORD",
        "SEED_ADMIN_PASSWORD",
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
    const SAMPLE_PASSWORDS: &[&str] = &[
        "change-me-in-production",
        "admin12345",
        "dev-password-123",
        "changeme",
        "password",
        "admin",
        "root",
        "12345678",
    ];
    let normalized = password.trim().to_ascii_lowercase();
    SAMPLE_PASSWORDS
        .iter()
        .copied()
        .find(|candidate| normalized == *candidate)
        .or_else(|| known_placeholder_fragment(&normalized))
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
    let stop = StopHandle::ensure(&runtime_ctx);
    let initialization = ServerBootstrapInitializationLock::ensure(&runtime_ctx);
    let Some(_initialization_guard) =
        await_worker_startup_or_stop(&stop, initialization.lock()).await
    else {
        return Err(crate::error::Error::Message(
            "server shutdown started before application router bootstrap".to_string(),
        ));
    };
    tracing::info!("RusTok application bootstrap started");

    let Some(product_catalog_result) =
        await_worker_startup_or_stop(&stop, configure_product_catalog_deployment(&runtime_ctx))
            .await
    else {
        return Err(crate::error::Error::Message(
            "server shutdown started during product catalog deployment bootstrap".to_string(),
        ));
    };
    product_catalog_result?;

    let Some(profile_media_result) = await_worker_startup_or_stop(
        &stop,
        configure_profile_media_public_image_deployment(&runtime_ctx),
    )
    .await
    else {
        return Err(crate::error::Error::Message(
            "server shutdown started during profile media deployment bootstrap".to_string(),
        ));
    };
    profile_media_result?;

    let runtime = bootstrap_app_runtime(runtime_ctx.clone(), auth_config.clone()).await?;
    if stop.is_stopping() {
        return Err(crate::error::Error::Message(
            "server shutdown started after application runtime bootstrap".to_string(),
        ));
    }
    tracing::info!("RusTok app runtime bootstrap completed");

    let router = compose_application_router(
        router,
        runtime_ctx.clone(),
        ServerAuthRuntime::new(runtime_ctx.clone(), auth_config),
        settings_snapshot,
        runtime,
        &rustok_settings,
    )?;
    if stop.is_stopping() {
        return Err(crate::error::Error::Message(
            "server shutdown started during application router composition".to_string(),
        ));
    }
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
    )
    .await?;

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
    if stop.is_stopping() {
        return Err(crate::error::Error::Message(
            "server shutdown started during runtime worker startup".to_string(),
        ));
    }
    tracing::info!("RusTok runtime workers connected");
    Ok(router)
}

#[cfg(test)]
mod tests {
    use super::{
        check_production_environment, known_dev_mac_root_fragment,
        known_sample_superadmin_password, sample_database_credentials_pattern,
        should_initialize_default_superadmin_for_mode,
    };

    static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvGuard {
        _guard: std::sync::MutexGuard<'static, ()>,
        vars: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn new(keys: &[&'static str]) -> Self {
            let guard = TEST_ENV_LOCK.lock().unwrap();
            let mut vars = Vec::new();
            for &k in keys {
                vars.push((k, std::env::var(k).ok()));
                unsafe {
                    std::env::remove_var(k);
                }
            }
            Self {
                _guard: guard,
                vars,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, v) in &self.vars {
                unsafe {
                    if let Some(val) = v {
                        std::env::set_var(k, val);
                    } else {
                        std::env::remove_var(k);
                    }
                }
            }
        }
    }

    const SUPERADMIN_ENV_KEYS: &[&str] = &[
        "SUPERADMIN_PASSWORD",
        "SEED_ADMIN_PASSWORD",
        "RUSTOK_DEV_SEED_PASSWORD",
    ];

    #[tokio::test]
    async fn waiting_context_initialization_exits_on_stop_without_acquiring_lock() {
        let ctx = super::ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            super::RustokSettings::default(),
        );
        let initialization = super::ServerBootstrapInitializationLock::ensure(&ctx);
        let _held = initialization.lock().await;
        let mut startup = Box::pin(super::initialize_server_context(&ctx, "", ""));
        assert!(futures_util::poll!(&mut startup).is_pending());

        super::StopHandle::ensure(&ctx).stop().await;
        let error = tokio::time::timeout(std::time::Duration::from_secs(1), startup)
            .await
            .expect("shutdown interrupts queued context initialization")
            .expect_err("stopped host must reject context initialization");
        assert!(
            error
                .to_string()
                .contains("shutdown started before runtime initialization")
        );
    }

    #[test]
    fn provisioning_is_limited_to_user_serving_host_modes() {
        for host_mode in [
            crate::common::settings::RuntimeHostMode::Full,
            crate::common::settings::RuntimeHostMode::Api,
            crate::common::settings::RuntimeHostMode::AdminSsr,
        ] {
            assert!(
                should_initialize_default_superadmin_for_mode(host_mode),
                "{host_mode:?}"
            );
        }
        for host_mode in [
            crate::common::settings::RuntimeHostMode::RegistryOnly,
            crate::common::settings::RuntimeHostMode::Worker,
            crate::common::settings::RuntimeHostMode::StorefrontSsr,
        ] {
            assert!(
                !should_initialize_default_superadmin_for_mode(host_mode),
                "{host_mode:?}"
            );
        }
    }

    #[test]
    fn production_secret_validation_is_disabled_outside_production() {
        check_production_environment(
            "dev-secret",
            "postgres://postgres:postgres@db/rustok",
            false,
        )
        .expect("non-production startup may use development values");
    }

    #[test]
    fn production_secret_validation_rejects_empty_mac_root_without_exposing_it() {
        let _env = EnvGuard::new(SUPERADMIN_ENV_KEYS);
        let error = check_production_environment("", "postgres://app:strong@db/rustok", true)
            .expect_err("production must reject an empty MAC root");
        let message = error.to_string();
        assert!(message.contains("MAC root must contain at least 64 non-whitespace bytes"));
    }

    #[test]
    fn production_secret_validation_rejects_whitespace_mac_root() {
        let _env = EnvGuard::new(SUPERADMIN_ENV_KEYS);
        let error = check_production_environment("   ", "postgres://app:strong@db/rustok", true)
            .expect_err("production must reject whitespace MAC root");
        let message = error.to_string();
        assert!(message.contains("MAC root must contain at least 64 non-whitespace bytes"));
    }

    #[test]
    fn production_mac_root_requires_64_non_whitespace_bytes() {
        let _env = EnvGuard::new(SUPERADMIN_ENV_KEYS);
        let short = "x".repeat(63);
        let error = check_production_environment(&short, "postgres://app:strong@db/rustok", true)
            .expect_err("63 non-whitespace bytes must fail the production MAC-root floor");
        assert!(error.to_string().contains("64 non-whitespace bytes"));

        let sufficient = "x".repeat(64);
        check_production_environment(&sufficient, "postgres://app:strong@db/rustok", true)
            .expect("64 non-whitespace bytes pass the length floor");
    }

    #[test]
    fn production_secret_validation_rejects_padded_placeholder_mac_root() {
        let _env = EnvGuard::new(SUPERADMIN_ENV_KEYS);
        let secret = format!("CHANGE_ME{}", "x".repeat(64));
        let error = check_production_environment(
            &secret,
            "postgres://app:strong@prod-db.internal/rustok_prod",
            true,
        )
        .expect_err("padding a placeholder must not make it a valid production MAC root");
        assert!(error.to_string().contains("known development value"));
    }

    #[test]
    fn production_mac_root_does_not_count_unicode_whitespace_bytes() {
        let _env = EnvGuard::new(SUPERADMIN_ENV_KEYS);
        let secret = format!("{}\u{00a0}", "x".repeat(63));
        let error = check_production_environment(
            &secret,
            "postgres://app:strong@prod-db.internal/rustok_prod",
            true,
        )
        .expect_err("Unicode whitespace must not satisfy the non-whitespace byte floor");
        assert!(error.to_string().contains("64 non-whitespace bytes"));
    }

    #[test]
    fn production_secret_validation_rejects_change_me_database_password() {
        let _env = EnvGuard::new(SUPERADMIN_ENV_KEYS);
        let secret = "aB3!zY7@qW8#eR2$".repeat(5);
        let error = check_production_environment(
            &secret,
            "postgres://rustok:CHANGE_ME@prod-db.internal:5432/rustok_prod",
            true,
        )
        .expect_err("sample database password must be rejected");
        assert!(error.to_string().contains("known sample credentials"));
    }

    #[test]
    fn production_secret_validation_rejects_change_me_superadmin_password() {
        let _env = EnvGuard::new(SUPERADMIN_ENV_KEYS);
        unsafe {
            std::env::set_var("SUPERADMIN_PASSWORD", "CHANGE_ME");
        }
        let secret = "aB3!zY7@qW8#eR2$".repeat(5);
        let error = check_production_environment(
            &secret,
            "postgres://app:strong@prod-db.internal:5432/rustok_prod",
            true,
        )
        .expect_err("placeholder superadmin password must be rejected");
        assert!(error.to_string().contains("weak or known sample"));
    }

    #[test]
    fn production_secret_validation_rejects_development_seed_password_variable() {
        let _env = EnvGuard::new(SUPERADMIN_ENV_KEYS);
        unsafe {
            std::env::set_var("RUSTOK_DEV_SEED_PASSWORD", "a-strong-but-development-only-password");
        }
        let secret = "aB3!zY7@qW8#eR2$".repeat(5);
        let error = check_production_environment(
            &secret,
            "postgres://app:strong@prod-db.internal:5432/rustok_prod",
            true,
        )
        .expect_err("development-only seed variable must not be accepted in production");
        assert!(error.to_string().contains("RUSTOK_DEV_SEED_PASSWORD"));
    }

    #[test]
    fn production_secret_validation_rejects_empty_and_whitespace_database_uri() {
        let _env = EnvGuard::new(SUPERADMIN_ENV_KEYS);
        let secret = "aB3!zY7@qW8#eR2$".repeat(5);
        let error_empty = check_production_environment(&secret, "", true)
            .expect_err("production must reject empty database URI");
        assert!(
            error_empty
                .to_string()
                .contains("database URI must not be empty")
        );

        let error_ws = check_production_environment(&secret, "   ", true)
            .expect_err("production must reject whitespace database URI");
        assert!(
            error_ws
                .to_string()
                .contains("database URI must not be empty")
        );
    }

    #[test]
    fn production_secret_validation_redacts_development_mac_root_fragment() {
        let _env = EnvGuard::new(SUPERADMIN_ENV_KEYS);
        let secret = "prefix-dev-secret-suffix";
        let error = check_production_environment(secret, "postgres://app:strong@db/rustok", true)
            .expect_err("known development MAC root material must be rejected");
        let message = error.to_string();
        assert!(message.contains("known development value"));
        assert!(!message.contains("dev-secret"));
    }

    #[test]
    fn production_secret_validation_redacts_database_sample_credentials() {
        let _env = EnvGuard::new(SUPERADMIN_ENV_KEYS);
        let uri = "postgres://postgres:postgres@db/rustok";
        let error = check_production_environment("aB3!zY7@qW8#eR2$".repeat(5).as_str(), uri, true)
            .expect_err("sample database credentials must be rejected");
        let message = error.to_string();
        assert!(message.contains("known sample credentials"));
        assert!(!message.contains("postgres:postgres"));
    }

    #[test]
    fn production_secret_validation_rejects_user_password_database_uri() {
        let _env = EnvGuard::new(SUPERADMIN_ENV_KEYS);
        let uri = "postgres://user:password@prod-host:5432/rustok";
        let error = check_production_environment("aB3!zY7@qW8#eR2$".repeat(5).as_str(), uri, true)
            .expect_err("user:password credentials must be rejected");
        assert!(error.to_string().contains("known sample credentials"));
    }

    #[test]
    fn production_secret_validation_rejects_weak_or_short_superadmin_password() {
        let _env = EnvGuard::new(SUPERADMIN_ENV_KEYS);
        let secret = "aB3!zY7@qW8#eR2$".repeat(5);
        unsafe {
            std::env::set_var("SUPERADMIN_PASSWORD", "short");
        }
        let error_short =
            check_production_environment(&secret, "postgres://app:strong@db/rustok", true)
                .expect_err("short superadmin password must fail");
        assert!(
            error_short
                .to_string()
                .contains("weak or known sample superadmin password")
        );

        let multibyte_short = "🔐".repeat(crate::auth::DEFAULT_MIN_PASSWORD_LENGTH_CHARS - 1);
        unsafe {
            std::env::set_var("SUPERADMIN_PASSWORD", &multibyte_short);
        }
        let error_multibyte =
            check_production_environment(&secret, "postgres://app:strong@db/rustok", true)
                .expect_err("passwords below the character limit must fail regardless of UTF-8 byte length");
        assert!(
            error_multibyte
                .to_string()
                .contains("weak or known sample superadmin password")
        );

        unsafe {
            std::env::set_var("SUPERADMIN_PASSWORD", "admin12345");
        }
        let error_sample =
            check_production_environment(&secret, "postgres://app:strong@db/rustok", true)
                .expect_err("sample superadmin password must fail");
        assert!(
            error_sample
                .to_string()
                .contains("weak or known sample superadmin password")
        );
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
        assert_eq!(
            known_dev_mac_root_fragment("dev-secret-value"),
            Some("dev-secret")
        );
        assert_eq!(
            known_dev_mac_root_fragment("MY-DEV_SECRET-TOKEN"),
            Some("dev_secret")
        );
        assert_eq!(
            known_dev_mac_root_fragment("something-CHANGEME-token"),
            Some("changeme")
        );
        assert_eq!(
            known_dev_mac_root_fragment("prefix-CHANGE_ME-suffix"),
            Some("change_me")
        );
        assert_eq!(
            known_dev_mac_root_fragment("prefix-change-me-suffix"),
            Some("change-me")
        );
        assert_eq!(
            sample_database_credentials_pattern("postgres://postgres:postgres@db/rustok"),
            Some("://postgres:postgres@")
        );
        assert_eq!(
            sample_database_credentials_pattern("postgres://admin:admin@db/rustok"),
            Some("://admin:admin@")
        );
        assert_eq!(
            sample_database_credentials_pattern("sqlite://local.db"),
            Some("sqlite:")
        );
        assert_eq!(
            sample_database_credentials_pattern(
                "postgres://rustok:CHANGE_ME@db.example.com:5432/rustok_prod"
            ),
            Some("change_me")
        );
        assert_eq!(
            sample_database_credentials_pattern(
                "postgres://app:strong@db.example.com:5432/rustok_prod"
            ),
            Some(".example.com")
        );
        assert_eq!(
            known_sample_superadmin_password("dev-password-123"),
            Some("dev-password-123")
        );
        assert_eq!(
            known_sample_superadmin_password("ADMIN12345"),
            Some("admin12345")
        );
        assert_eq!(
            known_sample_superadmin_password("CHANGE_ME"),
            Some("change_me")
        );
        assert_eq!(
            known_sample_superadmin_password("prefix-replace-me-suffix"),
            Some("replace-me")
        );
    }
}
