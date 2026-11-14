//! Pure Axum executable host composition.

use std::{net::SocketAddr, path::PathBuf, time::Duration};

use sea_orm::{ConnectOptions, Database};
use sea_orm_migration::MigratorTrait;
use serde::Deserialize;

use crate::{
    common::settings::RustokSettings,
    error::{Error, Result},
    middleware::security_headers::hsts_enabled,
    routes::ServerRouter,
    services::{
        app_lifecycle::shutdown_runtime_workers,
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
    auto_migrate: bool,
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
    let config = load_config().await?;
    let database_url = std::env::var("DATABASE_URL").ok();
    let database_uri = resolve_database_uri(database_url.as_deref(), &config.database.uri);
    let production = is_production_environment();
    validate_database_deployment(&database_uri, production)?;
    validate_https_deployment(production, hsts_enabled())?;
    let db = connect_database(&config.database, &database_uri).await?;
    migrate_database_if_enabled(&db, config.database.auto_migrate).await?;
    let rustok_settings = RustokSettings::from_settings(&Some(config.settings.clone()))
        .map_err(|error| Error::BadRequest(format!("Invalid rustok settings: {error}")))?;
    let runtime_ctx = ServerRuntimeContext::new(db, rustok_settings.clone());
    let auth_config = crate::auth::auth_config_from_host_settings(
        config.auth.jwt.secret.clone(),
        config.auth.jwt.expiration,
        Some(&config.settings),
    )?;
    validate_auth_deployment(&auth_config, production)?;

    let result = with_runtime_shutdown(&runtime_ctx, async {
        initialize_server_context(&runtime_ctx, &config.auth.jwt.secret, &database_uri).await?;

        let router =
            application_router(rustok_settings.runtime.host_mode, &rustok_settings).with_state(
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

        let (binding, port) = resolve_server_endpoint(&config.server)?;
        let listener = tokio::net::TcpListener::bind((binding.as_str(), port)).await?;
        let address = listener.local_addr()?;
        tracing::info!(%address, "RusTok Axum host listening");

        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(wait_for_shutdown_signal())
        .await
        .map_err(Error::Io)
    })
    .await;
    if result.is_ok() {
        tracing::info!("RusTok Axum host shut down cleanly");
    }
    result
}

// Once initialization can spawn work, every ordinary result path must drain
// tracked workers without replacing the original bootstrap, bind, or serve error.
async fn with_runtime_shutdown<T>(
    ctx: &ServerRuntimeContext,
    run: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    let mut shutdown_on_drop = RuntimeShutdownOnDrop::new(ctx.clone());
    let result = run.await;
    shutdown_runtime_workers(ctx).await;
    shutdown_on_drop.disarm();
    result
}

/// Schedules worker shutdown if the host lifecycle future is cancelled or unwinds.
/// Normal completion disarms the guard and performs an awaited drain instead.
struct RuntimeShutdownOnDrop {
    ctx: ServerRuntimeContext,
    armed: bool,
}

impl RuntimeShutdownOnDrop {
    fn new(ctx: ServerRuntimeContext) -> Self {
        Self { ctx, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for RuntimeShutdownOnDrop {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let ctx = self.ctx.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                runtime.spawn(async move {
                    tracing::warn!(
                        "Host lifecycle was cancelled or unwound; scheduling runtime worker shutdown"
                    );
                    shutdown_runtime_workers(&ctx).await;
                });
            }
            Err(error) => tracing::error!(
                %error,
                "Host lifecycle was cancelled outside a Tokio runtime; runtime worker shutdown could not be scheduled"
            ),
        }
    }
}

async fn migrate_database_if_enabled(
    db: &sea_orm::DatabaseConnection,
    auto_migrate: bool,
) -> Result<()> {
    if !auto_migrate {
        return Ok(());
    }

    rustok_migrations::Migrator::up(db, None).await?;
    let pending = rustok_migrations::Migrator::get_pending_migrations(db).await?;
    if !pending.is_empty() {
        return Err(Error::Message(format!(
            "database migration completed with {} migration(s) still pending",
            pending.len()
        )));
    }
    tracing::info!("All database migrations are applied");
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

fn validate_auth_deployment(config: &crate::auth::AuthConfig, production: bool) -> Result<()> {
    if !production {
        return Ok(());
    }

    if config.issuer == DEFAULT_JWT_ISSUER
        || !config.issuer.starts_with("https://")
        || config.issuer.trim().len() <= 8
        || config.issuer.chars().any(char::is_whitespace)
    {
        return Err(Error::BadRequest(
            "Production JWT issuer must be an explicit HTTPS identifier, contain a valid host, contain no whitespace, and must not use the `rustok` default"
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

    if config.access_expiration == 0 {
        return Err(Error::BadRequest(
            "Production JWT access_expiration must be greater than zero".to_string(),
        ));
    }

    if config.refresh_expiration == 0 {
        return Err(Error::BadRequest(
            "Production JWT refresh_expiration must be greater than zero".to_string(),
        ));
    }

    if config.min_password_length < 8 {
        return Err(Error::BadRequest(
            "Production min_password_length must be at least 8 characters".to_string(),
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

fn validate_database_deployment(database_uri: &str, production: bool) -> Result<()> {
    if !production {
        return Ok(());
    }

    let normalized = database_uri.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(Error::BadRequest(
            "Production database URI must not be empty".to_string(),
        ));
    }

    const FORBIDDEN_PROD_DATABASE_PATTERNS: &[&str] = &[
        "rustok:rustok",
        "postgres:postgres",
        "admin:admin",
        "root:root",
        "user:password",
        "sqlite:",
        "localhost:5432/rustok_dev",
        "127.0.0.1:5432/rustok_dev",
    ];

    for pattern in FORBIDDEN_PROD_DATABASE_PATTERNS {
        if normalized.contains(pattern) {
            return Err(Error::BadRequest(format!(
                "Production database configuration must not use development credentials or local placeholders (`{pattern}`)"
            )));
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
    crate::common::settings::is_production_environment()
}

fn application_router(
    host_mode: crate::common::settings::RuntimeHostMode,
    settings: &crate::common::settings::RustokSettings,
) -> ServerRouter {
    crate::routes::build_application_router(host_mode, settings)
}

fn resolve_database_uri(database_url: Option<&str>, configured_uri: &str) -> String {
    database_url.unwrap_or(configured_uri).to_string()
}

async fn connect_database(
    config: &DatabaseConfig,
    uri: &str,
) -> Result<sea_orm::DatabaseConnection> {
    if config.max_connections > 0 && config.min_connections > config.max_connections {
        return Err(Error::BadRequest(format!(
            "Database min_connections ({}) cannot exceed max_connections ({})",
            config.min_connections, config.max_connections
        )));
    }

    let mut options = ConnectOptions::new(uri.to_string());
    options.sqlx_logging(config.enable_logging);
    let timeout = if config.connect_timeout > 0 {
        Duration::from_millis(config.connect_timeout.max(5000))
    } else {
        Duration::from_secs(10)
    };
    options.connect_timeout(timeout);
    options.acquire_timeout(timeout);
    if config.idle_timeout > 0 {
        options.idle_timeout(Duration::from_millis(config.idle_timeout.max(1000)));
    }
    if config.min_connections > 0 {
        options.min_connections(config.min_connections);
    }
    if config.max_connections > 0 {
        options.max_connections(config.max_connections);
    }
    Database::connect(options).await.map_err(Error::Database)
}

async fn load_config() -> Result<HostConfig> {
    let environment =
        crate::common::settings::resolve_environment_name().map_err(Error::BadRequest)?;

    let config_dir = std::env::var("RUSTOK_CONFIG_DIR")
        .ok()
        .map(|s| PathBuf::from(s.trim()))
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|executable| executable.parent().map(PathBuf::from))
                .map(|parent| parent.join("config"))
                .filter(|path| path.is_dir())
        })
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config"));
    let path = config_dir.join(format!("{environment}.yaml"));
    let raw = tokio::fs::read_to_string(&path).await.map_err(|error| {
        Error::Message(format!(
            "failed to read configuration file at '{}': {error}",
            path.display()
        ))
    })?;
    serde_yaml::from_str(&raw).map_err(|error| {
        Error::Message(format!(
            "failed to parse configuration file at '{}': {error}",
            path.display()
        ))
    })
}

fn resolve_server_endpoint(config: &ServerConfig) -> Result<(String, u16)> {
    let binding = std::env::var("RUSTOK_BINDING")
        .ok()
        .or_else(|| std::env::var("HOST").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| config.binding.trim().to_string());
    if binding.is_empty() {
        return Err(Error::BadRequest(
            "Server binding address must not be empty".to_string(),
        ));
    }
    let port = if let Ok(port_str) = std::env::var("RUSTOK_PORT").or_else(|_| std::env::var("PORT"))
    {
        port_str.trim().parse::<u16>().map_err(|error| {
            Error::BadRequest(format!("Invalid port '{port_str}' in environment: {error}"))
        })?
    } else {
        config.port
    };
    Ok((binding, port))
}

async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    result = tokio::signal::ctrl_c() => {
                        if let Err(error) = result {
                            tracing::warn!(error = %error, "failed to receive Ctrl-C shutdown signal");
                        } else {
                            tracing::info!("RusTok Axum host received Ctrl-C");
                        }
                    }
                    _ = terminate.recv() => {
                        tracing::info!("RusTok Axum host received SIGTERM");
                    }
                }
            }
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "failed to register SIGTERM handler; falling back to Ctrl-C shutdown handling"
                );
                if let Err(error) = tokio::signal::ctrl_c().await {
                    tracing::warn!(error = %error, "failed to receive shutdown signal");
                } else {
                    tracing::info!("RusTok Axum host received Ctrl-C");
                }
            }
        }
    }

    #[cfg(not(unix))]
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!(error = %error, "failed to receive shutdown signal");
    } else {
        tracing::info!("RusTok Axum host received Ctrl-C");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HostConfig, application_router, resolve_database_uri, validate_auth_deployment,
        validate_https_deployment,
    };
    use crate::common::settings::RuntimeHostMode;

    #[tokio::test]
    async fn host_shutdown_runs_after_success_and_preserves_startup_error() {
        use crate::common::settings::RustokSettings;
        use crate::services::app_lifecycle::{ModuleWorkSchedulerHandle, StopHandle};
        use crate::services::server_runtime_context::ServerRuntimeContext;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        for fail in [false, true] {
            let ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
            let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
            let result = super::with_runtime_shutdown(&ctx, async {
                let mut stop = StopHandle::ensure(&ctx).subscribe();
                let task = tokio::spawn(async move {
                    stop.changed().await.unwrap();
                    let _ = finished_tx.send(());
                });
                ctx.shared_insert(ModuleWorkSchedulerHandle::new(task));
                if fail {
                    Err(crate::error::Error::Message(
                        "original startup failure".into(),
                    ))
                } else {
                    Ok(42)
                }
            })
            .await;
            finished_rx.await.expect("tracked worker must be drained");
            assert!(StopHandle::ensure(&ctx).is_stopping());
            assert!(!ctx.shared_contains::<ModuleWorkSchedulerHandle>());
            match (fail, result) {
                (false, Ok(value)) => assert_eq!(value, 42),
                (true, Err(crate::error::Error::Message(message))) => {
                    assert_eq!(message, "original startup failure")
                }
                _ => panic!("host result was not preserved"),
            }
        }
    }

    #[tokio::test]
    async fn host_shutdown_is_scheduled_when_lifecycle_future_is_cancelled() {
        use crate::common::settings::RustokSettings;
        use crate::services::app_lifecycle::{ModuleWorkSchedulerHandle, StopHandle};
        use crate::services::server_runtime_context::ServerRuntimeContext;
        use std::time::Duration;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let mut stop = StopHandle::ensure(&ctx).subscribe();
        let (stopped_tx, stopped_rx) = tokio::sync::oneshot::channel();
        let worker = tokio::spawn(async move {
            if stop.changed().await.is_ok() && *stop.borrow() {
                let _ = stopped_tx.send(());
            }
        });
        ctx.shared_insert(ModuleWorkSchedulerHandle::new(worker));

        let host_ctx = ctx.clone();
        let host = tokio::spawn(async move {
            super::with_runtime_shutdown(
                &host_ctx,
                std::future::pending::<crate::error::Result<()>>(),
            )
            .await
        });
        tokio::task::yield_now().await;
        host.abort();
        let _ = host.await;

        tokio::time::timeout(Duration::from_secs(1), stopped_rx)
            .await
            .expect("cancellation must schedule shutdown")
            .expect("tracked worker must receive shutdown");
        assert!(!ctx.shared_contains::<ModuleWorkSchedulerHandle>());
    }

    #[tokio::test]
    async fn host_shutdown_is_scheduled_when_lifecycle_future_unwinds() {
        use crate::common::settings::RustokSettings;
        use crate::services::app_lifecycle::{ModuleWorkSchedulerHandle, StopHandle};
        use crate::services::server_runtime_context::ServerRuntimeContext;
        use std::time::Duration;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let mut stop = StopHandle::ensure(&ctx).subscribe();
        let (stopped_tx, stopped_rx) = tokio::sync::oneshot::channel();
        let worker = tokio::spawn(async move {
            if stop.changed().await.is_ok() && *stop.borrow() {
                let _ = stopped_tx.send(());
            }
        });
        ctx.shared_insert(ModuleWorkSchedulerHandle::new(worker));

        let host_ctx = ctx.clone();
        let trigger_panic = true;
        let host = tokio::spawn(async move {
            super::with_runtime_shutdown(&host_ctx, async move {
                if trigger_panic {
                    panic!("simulated host lifecycle panic");
                }
                Ok::<(), crate::error::Error>(())
            })
            .await
        });
        let panic = host
            .await
            .expect_err("host task should preserve panic behavior");
        assert!(panic.is_panic());

        tokio::time::timeout(Duration::from_secs(1), stopped_rx)
            .await
            .expect("unwind must schedule shutdown")
            .expect("tracked worker must receive shutdown");
        assert!(!ctx.shared_contains::<ModuleWorkSchedulerHandle>());
    }

    #[tokio::test]
    async fn host_shutdown_runs_when_listener_bind_fails() {
        use crate::common::settings::RustokSettings;
        use crate::services::app_lifecycle::{ModuleWorkSchedulerHandle, StopHandle};
        use crate::services::server_runtime_context::ServerRuntimeContext;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let occupied = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = occupied.local_addr().unwrap();
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        let result = super::with_runtime_shutdown(&ctx, async {
            let mut stop = StopHandle::ensure(&ctx).subscribe();
            ctx.shared_insert(ModuleWorkSchedulerHandle::new(tokio::spawn(async move {
                stop.changed().await.unwrap();
                let _ = finished_tx.send(());
            })));
            let _listener = tokio::net::TcpListener::bind(address).await?;
            Ok(())
        })
        .await;
        assert!(matches!(result, Err(crate::error::Error::Io(error))
            if error.kind() == std::io::ErrorKind::AddrInUse));
        finished_rx.await.expect("tracked worker must be drained");
        assert!(!ctx.shared_contains::<ModuleWorkSchedulerHandle>());
    }

    #[test]
    fn registry_only_router_is_composable_without_domain_routes() {
        let _ = application_router(
            RuntimeHostMode::RegistryOnly,
            &crate::common::settings::RustokSettings::default(),
        );
    }

    #[test]
    fn worker_router_is_composable_without_http_application_surfaces() {
        let _ = application_router(
            RuntimeHostMode::Worker,
            &crate::common::settings::RustokSettings::default(),
        );
    }

    #[test]
    fn development_config_enables_canonical_startup_migrations() {
        let config: HostConfig = serde_yaml::from_str(include_str!("../config/development.yaml"))
            .expect("development host configuration must deserialize");

        assert!(config.database.auto_migrate);
    }

    #[test]
    fn explicit_database_url_overrides_configured_uri_without_backend_heuristics() {
        assert_eq!(
            resolve_database_uri(Some("sqlite::memory:"), "postgres://localhost/rustok"),
            "sqlite::memory:"
        );
        assert_eq!(
            resolve_database_uri(None, "postgres://u:p@db:5432/rustok"),
            "postgres://u:p@db:5432/rustok"
        );
        assert_eq!(
            resolve_database_uri(None, "postgres://u:p@localhost:5432/rustok"),
            "postgres://u:p@localhost:5432/rustok"
        );
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

    #[test]
    fn production_rejects_default_dev_database_credentials() {
        let error = super::validate_database_deployment(
            "postgres://rustok:rustok@localhost:5432/rustok_dev",
            true,
        )
        .expect_err("production must reject dev database defaults");
        assert!(error.to_string().contains("development credentials"));
    }

    #[test]
    fn production_rejects_sqlite_database() {
        let error = super::validate_database_deployment("sqlite::memory:", true)
            .expect_err("production must reject sqlite");
        assert!(error.to_string().contains("development credentials"));
    }

    #[test]
    fn production_accepts_secure_database_uri() {
        super::validate_database_deployment(
            "postgres://app_user:K8x#mP9$vL2@prod-db.internal:5432/rustok_prod",
            true,
        )
        .expect("strong production credentials should pass");
    }

    #[test]
    fn non_production_allows_dev_database_credentials() {
        super::validate_database_deployment(
            "postgres://rustok:rustok@localhost:5432/rustok_dev",
            false,
        )
        .expect("development allows dev credentials");
    }

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

    #[test]
    fn resolve_server_endpoint_uses_config_defaults() {
        let _env = EnvGuard::new(&["PORT", "RUSTOK_PORT", "HOST", "RUSTOK_BINDING"]);
        let config = super::ServerConfig {
            binding: "127.0.0.1".to_string(),
            port: 5150,
        };
        let (binding, port) = super::resolve_server_endpoint(&config).expect("resolves defaults");
        assert_eq!(binding, "127.0.0.1");
        assert_eq!(port, 5150);
    }

    #[test]
    fn resolve_server_endpoint_rejects_invalid_port() {
        let _env = EnvGuard::new(&["PORT", "RUSTOK_PORT", "HOST", "RUSTOK_BINDING"]);
        unsafe {
            std::env::set_var("PORT", "not-a-port");
        }
        let config = super::ServerConfig {
            binding: "127.0.0.1".to_string(),
            port: 5150,
        };
        let error = super::resolve_server_endpoint(&config).expect_err("invalid port must fail");
        assert!(error.to_string().contains("Invalid port"));
    }

    #[tokio::test]
    async fn connect_database_rejects_min_connections_exceeding_max() {
        let config = super::DatabaseConfig {
            uri: "sqlite::memory:".to_string(),
            auto_migrate: false,
            enable_logging: false,
            connect_timeout: 5000,
            idle_timeout: 5000,
            min_connections: 20,
            max_connections: 5,
        };
        let error = super::connect_database(&config, "sqlite::memory:")
            .await
            .expect_err("min_connections exceeding max_connections must be rejected");
        assert!(error.to_string().contains("cannot exceed max_connections"));
    }

    #[test]
    fn validate_auth_deployment_rejects_empty_or_whitespace_issuer() {
        let config = crate::auth::AuthConfig::new("aB3!zY7@qW8#eR2$".repeat(5))
            .with_issuer("https://")
            .with_audience("rustok-production-admin");
        let error =
            validate_auth_deployment(&config, true).expect_err("issuer without host must fail");
        assert!(error.to_string().contains("issuer"));

        let config_ws = crate::auth::AuthConfig::new("aB3!zY7@qW8#eR2$".repeat(5))
            .with_issuer("https://api .example.com")
            .with_audience("rustok-production-admin");
        let error_ws = validate_auth_deployment(&config_ws, true)
            .expect_err("issuer with whitespace must fail");
        assert!(error_ws.to_string().contains("issuer"));
    }

    #[test]
    fn validate_auth_deployment_rejects_zero_expiration_and_short_password() {
        let mut config = crate::auth::AuthConfig::new("aB3!zY7@qW8#eR2$".repeat(5))
            .with_issuer("https://api.example.com")
            .with_audience("rustok-production-admin");
        config.access_expiration = 0;
        let error = validate_auth_deployment(&config, true)
            .expect_err("access_expiration of zero must fail");
        assert!(error.to_string().contains("access_expiration"));

        config.access_expiration = 900;
        config.refresh_expiration = 0;
        let error_refresh = validate_auth_deployment(&config, true)
            .expect_err("refresh_expiration of zero must fail");
        assert!(error_refresh.to_string().contains("refresh_expiration"));

        config.refresh_expiration = 604800;
        config.min_password_length = 6;
        let error_pwd = validate_auth_deployment(&config, true)
            .expect_err("min_password_length < 8 must fail in production");
        assert!(error_pwd.to_string().contains("min_password_length"));
    }

    #[test]
    fn validate_database_deployment_rejects_empty_uri_and_user_password_pattern() {
        let error_empty = super::validate_database_deployment("", true)
            .expect_err("empty database URI in production must fail");
        assert!(error_empty.to_string().contains("must not be empty"));

        let error_ws = super::validate_database_deployment("   ", true)
            .expect_err("whitespace database URI in production must fail");
        assert!(error_ws.to_string().contains("must not be empty"));

        let error_pattern = super::validate_database_deployment(
            "postgres://user:password@prod-host:5432/rustok",
            true,
        )
        .expect_err("user:password placeholder pattern must fail");
        assert!(
            error_pattern
                .to_string()
                .contains("development credentials")
        );
    }

    #[test]
    fn resolve_server_endpoint_trims_binding_and_rejects_empty() {
        let _env = EnvGuard::new(&["PORT", "RUSTOK_PORT", "HOST", "RUSTOK_BINDING"]);
        unsafe {
            std::env::set_var("RUSTOK_BINDING", "  10.0.0.1  ");
        }
        let config = super::ServerConfig {
            binding: "127.0.0.1".to_string(),
            port: 8080,
        };
        let (binding, port) =
            super::resolve_server_endpoint(&config).expect("valid trimmed binding");
        assert_eq!(binding, "10.0.0.1");
        assert_eq!(port, 8080);

        unsafe {
            std::env::remove_var("RUSTOK_BINDING");
        }
        let config_empty = super::ServerConfig {
            binding: "   ".to_string(),
            port: 8080,
        };
        let error =
            super::resolve_server_endpoint(&config_empty).expect_err("empty binding must fail");
        assert!(error.to_string().contains("must not be empty"));
    }
}
