//! Pure Axum executable host composition.

use std::{net::SocketAddr, path::{Path, PathBuf}, time::Duration};

use rustok_api::sha256_digest;
use tokio::io::AsyncReadExt;

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
const JWT_KEYRING_FILE_ENV: &str = "RUSTOK_JWT_KEYRING_FILE";
const JWT_RELOAD_INTERVAL: Duration = Duration::from_secs(2);

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

#[derive(Deserialize)]
struct JwtConfig {
    secret: String,
    expiration: u64,
}

impl std::fmt::Debug for JwtConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("JwtConfig")
            .field("secret", &"<redacted>")
            .field("expiration", &self.expiration)
            .finish()
    }
}

/// Starts the HTTP-only Axum host.
pub async fn run() -> Result<()> {
    // Poll this future before the first asynchronous startup operation so
    // SIGTERM/CTRL-C is handled during config loading, DB connection, and
    // migrations—not only after the HTTP listener is ready.
    let mut shutdown_signal = Box::pin(wait_for_shutdown_signal());
    let config = tokio::select! {
        biased;
        _ = &mut shutdown_signal => {
            tracing::info!("Shutdown signal received before server startup");
            return Ok(());
        }
        result = load_config() => result?,
    };

    let database_url = std::env::var("DATABASE_URL").ok();
    let database_uri = resolve_database_uri(database_url.as_deref(), &config.database.uri);
    let production = crate::common::is_production_security_mode();
    validate_database_deployment(&database_uri, production)?;
    validate_https_deployment(production, hsts_enabled())?;

    // Validate all settings-derived policy before connecting to the database or
    // applying migrations, so a bad issuer/secret cannot mutate persistent state.
    let rustok_settings = RustokSettings::from_settings(&Some(config.settings.clone()))
        .map_err(|error| Error::BadRequest(format!("Invalid rustok settings: {error}")))?;
    let keyring_path = jwt_keyring_file_path()?;
    if keyring_path.is_some() {
        validate_keyring_file_source(&config.settings)?;
    }
    let base_auth_config = crate::auth::auth_config_from_host_settings(
        config.auth.jwt.secret.clone(),
        config.auth.jwt.expiration,
        Some(&config.settings),
    )?;
    let (auth_config, reload_source) = if let Some(path) = keyring_path {
        let bytes = read_jwt_keyring_file(&path).await?;
        let digest: [u8; 32] = sha256_digest(&[&bytes]);
        let (generation, candidate) = base_auth_config.prepare_jwt_document(&bytes)
            .map_err(crate::auth::auth_err)?;
        validate_auth_deployment(&candidate, production)?;
        (candidate, Some((path, generation, digest)))
    } else {
        validate_auth_deployment(&base_auth_config, production)?;
        (base_auth_config, None)
    };
    validate_oauth_metadata_issuer(
        &auth_config,
        crate::routes::oauth_surface_enabled(rustok_settings.runtime.host_mode, &rustok_settings),
    )?;

    let db = tokio::select! {
        biased;
        _ = &mut shutdown_signal => {
            tracing::info!("Shutdown signal received before database connection");
            return Ok(());
        }
        result = connect_database(&config.database, &database_uri) => result?,
    };
    // Do not cancel a migration halfway through. If shutdown arrives during
    // migration, let the current migration finish and then exit before starting
    // workers or opening the HTTP listener.
    let shutdown_during_migration = {
        let migration = migrate_database_if_enabled(&db, config.database.auto_migrate);
        tokio::pin!(migration);
        tokio::select! {
            biased;
            result = &mut migration => {
                result?;
                false
            }
            _ = &mut shutdown_signal => {
                tracing::info!("Shutdown signal received during database migration; waiting for migration to finish");
                migration.await?;
                true
            }
        }
    };
    if shutdown_during_migration {
        return Ok(());
    }

    // The database is the durable admission authority. Do not expose a
    // listener or compose request consumers before the startup fence succeeds.
    let fence = rustok_auth::JwtGenerationFence::new(db.clone());
    let generation = if let Some((_, generation, _)) = &reload_source {
        loop {
            let admission = tokio::select! {
                biased;
                _ = &mut shutdown_signal => return Ok(()),
                result = fence.admit(*generation, &auth_config) => result.map_err(crate::auth::auth_err)?,
            };
            if admission == rustok_auth::JwtAdmission::Admitted {
                break *generation;
            }
            tokio::select! {
                biased;
                _ = &mut shutdown_signal => return Ok(()),
                _ = tokio::time::sleep(JWT_RELOAD_INTERVAL) => {}
            }
        }
    } else {
        fence.ensure_static_allowed().await.map_err(crate::auth::auth_err)?;
        0
    };
    let (auth_config, publisher) = auth_config.into_cluster_jwt(generation)
        .map_err(crate::auth::auth_err)?;
    let supervisor = JwtRuntimeSupervisor {
        source: reload_source.map(|(path, _, digest)| JwtFileSource { path, digest }),
        publisher, fence,
    };
    if !supervisor.fence.renew(&supervisor.publisher).await.map_err(crate::auth::auth_err)? {
        return Err(Error::Message("JWT cluster lease denied at startup".into()));
    }
    tracing::info!(generation, "JWT keyring runtime initialized");

    let runtime_ctx = ServerRuntimeContext::new(db, rustok_settings.clone());
    runtime_ctx.shared_insert(
        supervisor.publisher.lease_readiness().map_err(crate::auth::auth_err)?,
    );

    let result = with_runtime_shutdown(&runtime_ctx, async {
        let mut supervisor = Box::pin(supervisor.run(production));
        let startup = async {
            initialize_server_context(&runtime_ctx, &config.auth.jwt.secret, &database_uri)
                .await?;

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
            Ok::<_, Error>((listener, router))
        };

        let (listener, router) = tokio::select! {
            biased;
            _ = &mut shutdown_signal => {
                tracing::info!("Shutdown signal received during server bootstrap");
                return Ok(());
            }
            result = startup => result?,
            result = &mut supervisor => return result,
        };

        let serve = async {
            axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown_signal)
            .await
            .map_err(Error::Io)
        };
        // The supervisor covers bootstrap and serving. Dropping it revokes
        // local JWT authority; the DB deadline still conservatively drains.
        tokio::select! {
            result = serve => result,
            result = &mut supervisor => result,
        }
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
            Err(_error) => tracing::error!(
                failure_code = "host_shutdown_without_runtime",
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

fn validate_oauth_metadata_issuer(
    config: &crate::auth::AuthConfig,
    oauth_surface_enabled: bool,
) -> Result<()> {
    if !oauth_surface_enabled {
        return Ok(());
    }

    let issuer = url::Url::parse(&config.issuer).map_err(|_| {
        Error::BadRequest(
            "OAuth discovery requires an explicit absolute issuer URL that matches JWT issuer claims"
                .to_string(),
        )
    })?;
    let is_local_http = issuer.scheme() == "http"
        && matches!(issuer.host_str(), Some("localhost") | Some("127.0.0.1"));
    let supported_scheme = issuer.scheme() == "https" || is_local_http;
    let root_path = issuer.path().is_empty() || issuer.path() == "/";

    if !supported_scheme
        || issuer.host_str().is_none()
        || config.issuer.contains('@')
        || !issuer.username().is_empty()
        || issuer.password().is_some()
        || !root_path
        || issuer.query().is_some()
        || issuer.fragment().is_some()
        || config.issuer.chars().any(char::is_whitespace)
    {
        return Err(Error::BadRequest(
            "OAuth discovery issuer must be an absolute HTTPS URL (or http://localhost/127.0.0.1 for development), with no userinfo, path, query, or fragment, so the discovery issuer exactly matches JWT issuer claims"
                .to_string(),
        ));
    }

    Ok(())
}

fn validate_auth_deployment(config: &crate::auth::AuthConfig, production: bool) -> Result<()> {
    if !production {
        return Ok(());
    }

    let parsed_issuer = url::Url::parse(&config.issuer).ok();
    let issuer_is_valid_https_url = parsed_issuer.as_ref().is_some_and(|issuer| {
        issuer.scheme() == "https"
            && issuer.host_str().is_some_and(|host| !host.is_empty())
            && issuer.username().is_empty()
            && issuer.password().is_none()
    });
    if config.issuer == DEFAULT_JWT_ISSUER
        || !issuer_is_valid_https_url
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

    // The root protects host MACs and derived JWT keys even for RS256 access.
    // Explicit HS256 keys from every purpose (including history) must meet the
    // same production policy.
    // Inspect the actual candidate, including every purpose and history key.
    config.inspect_hs256_secrets(|secret| {
        if secret.bytes().filter(|b| !b.is_ascii_whitespace()).count()
            < MIN_PRODUCTION_HS256_SECRET_BYTES
        {
            return Err(rustok_auth::AuthError::Validation(format!(
                "Production HS256/root secret must contain at least {MIN_PRODUCTION_HS256_SECRET_BYTES} bytes"
            )));
        }
        if jwt_secret_looks_like_placeholder(secret) {
            return Err(rustok_auth::AuthError::Validation(
                "Production HS256/root secret looks like a placeholder or low-diversity value"
                    .to_string(),
            ));
        }
        Ok(())
    }).map_err(crate::auth::auth_err)?;
    rustok_auth::validate_auth_config(config).map_err(crate::auth::auth_err)?;

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

fn application_router(
    host_mode: crate::common::settings::RuntimeHostMode,
    settings: &crate::common::settings::RustokSettings,
) -> ServerRouter {
    crate::routes::build_application_router(host_mode, settings)
}

fn resolve_database_uri(database_url: Option<&str>, configured_uri: &str) -> String {
    database_url.unwrap_or(configured_uri).to_string()
}

const DEFAULT_DATABASE_MAX_CONNECTIONS: u32 = 10;
const MAX_DATABASE_CONNECTIONS: u32 = 1_024;
const MAX_DATABASE_CONNECT_TIMEOUT_MS: u64 = 300_000;
const MAX_DATABASE_IDLE_TIMEOUT_MS: u64 = 86_400_000;

fn validate_database_connection_settings(config: &DatabaseConfig) -> Result<()> {
    if config.max_connections > MAX_DATABASE_CONNECTIONS {
        return Err(Error::BadRequest(format!(
            "Database max_connections ({}) cannot exceed {MAX_DATABASE_CONNECTIONS}",
            config.max_connections
        )));
    }

    // SeaORM/SQLx uses a default maximum of ten connections when no explicit
    // max_connections value is supplied.
    let effective_max_connections = if config.max_connections == 0 {
        DEFAULT_DATABASE_MAX_CONNECTIONS
    } else {
        config.max_connections
    };
    if config.min_connections > effective_max_connections {
        return Err(Error::BadRequest(format!(
            "Database min_connections ({}) cannot exceed max_connections ({effective_max_connections})",
            config.min_connections
        )));
    }

    if config.connect_timeout > MAX_DATABASE_CONNECT_TIMEOUT_MS {
        return Err(Error::BadRequest(format!(
            "Database connect_timeout ({}) cannot exceed {MAX_DATABASE_CONNECT_TIMEOUT_MS} milliseconds",
            config.connect_timeout
        )));
    }
    if config.idle_timeout > MAX_DATABASE_IDLE_TIMEOUT_MS {
        return Err(Error::BadRequest(format!(
            "Database idle_timeout ({}) cannot exceed {MAX_DATABASE_IDLE_TIMEOUT_MS} milliseconds",
            config.idle_timeout
        )));
    }

    Ok(())
}

async fn connect_database(
    config: &DatabaseConfig,
    uri: &str,
) -> Result<sea_orm::DatabaseConnection> {
    validate_database_connection_settings(config)?;

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

fn jwt_keyring_file_path() -> Result<Option<PathBuf>> {
    match std::env::var(JWT_KEYRING_FILE_ENV) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(PathBuf::from(value))),
        Err(std::env::VarError::NotPresent) => Ok(None),
        _ => Err(Error::BadRequest("Invalid JWT keyring file path".to_string())),
    }
}

fn validate_keyring_file_source(settings: &serde_json::Value) -> Result<()> {
    if let Some(auth) = settings.get("auth").and_then(serde_json::Value::as_object) {
        for field in [
            "algorithm", "jwt_keyring_env", "jwt_special_keyrings_env",
            "rsa_private_key_pem", "rsa_public_key_pem",
            "rsa_private_key_env", "rsa_public_key_env",
        ] {
            if auth.get(field).is_some_and(|value| !value.is_null()) {
                return Err(Error::BadRequest(
                    "JWT keyring file cannot be combined with startup key overrides".to_string(),
                ));
            }
        }
    }
    Ok(())
}

async fn read_jwt_keyring_file(path: &Path) -> Result<Vec<u8>> {
    const MAX: usize = rustok_auth::MAX_JWT_KEYRING_DOCUMENT_BYTES;
    let metadata = tokio::fs::metadata(path).await
        .map_err(|_| Error::Message("Cannot read JWT keyring file".to_string()))?;
    if !metadata.is_file() || metadata.len() > MAX as u64 {
        return Err(Error::BadRequest("Invalid JWT keyring file size or type".to_string()));
    }
    let file = tokio::fs::File::open(path).await
        .map_err(|_| Error::Message("Cannot open JWT keyring file".to_string()))?;
    let mut bytes = Vec::new();
    file.take((MAX + 1) as u64).read_to_end(&mut bytes).await
        .map_err(|_| Error::Message("Cannot read JWT keyring file".to_string()))?;
    if bytes.len() > MAX {
        return Err(Error::BadRequest("JWT keyring file exceeds size limit".to_string()));
    }
    Ok(bytes)
}

struct JwtFileSource {
    path: PathBuf,
    digest: [u8; 32],
}

struct JwtRuntimeSupervisor {
    source: Option<JwtFileSource>,
    publisher: rustok_auth::JwtKeyringPublisher,
    fence: rustok_auth::JwtGenerationFence,
}

impl Drop for JwtRuntimeSupervisor {
    fn drop(&mut self) {
        self.publisher.revoke_lease();
    }
}

impl JwtRuntimeSupervisor {
    async fn run(mut self, production: bool) -> Result<()> {
        let mut interval = tokio::time::interval(JWT_RELOAD_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Some(source) = self.source.as_mut() {
                Self::poll_source(source, &self.publisher, &self.fence, production).await?;
            }
            let granted = self.fence.renew(&self.publisher).await.map_err(crate::auth::auth_err)?;
            if !granted && self.source.is_none() {
                return Err(Error::Message("Static JWT cluster lease denied".into()));
            }
            // A draining or stale file-backed host keeps polling to converge;
            // its JWT operations fail closed when its previous lease expires.
        }
    }

    async fn poll_source(
        source: &mut JwtFileSource,
        publisher: &rustok_auth::JwtKeyringPublisher,
        fence: &rustok_auth::JwtGenerationFence,
        production: bool,
    ) -> Result<()> {
        let bytes = match read_jwt_keyring_file(&source.path).await {
            Ok(bytes) => bytes,
            Err(_) => {
                tracing::warn!("JWT keyring reload source unavailable");
                return Ok(());
            }
        };
        let digest = sha256_digest(&[&bytes]);
        if digest == source.digest {
            return Ok(());
        }
        let (generation, candidate) = match publisher.prepare_document(&bytes) {
            Ok(candidate) => candidate,
            Err(_) => {
                source.digest = digest;
                tracing::warn!("JWT keyring reload rejected");
                return Ok(());
            }
        };
        if generation <= publisher.generation().map_err(crate::auth::auth_err)?
            || validate_auth_deployment(&candidate, production).is_err()
        {
            source.digest = digest;
            tracing::warn!("JWT keyring reload rejected");
            return Ok(());
        }
        match fence.admit(generation, &candidate).await {
            Ok(rustok_auth::JwtAdmission::Admitted) => {}
            // Do not mark the file processed: the same candidate must retry
            // after the old database lease horizon has drained.
            Ok(rustok_auth::JwtAdmission::Draining) => return Ok(()),
            Err(rustok_auth::AuthError::Validation(_)) => {
                source.digest = digest;
                tracing::warn!("JWT keyring generation rejected by durable fence");
                return Ok(());
            }
            Err(error) => return Err(crate::auth::auth_err(error)),
        }
        publisher.publish(generation, candidate).map_err(crate::auth::auth_err)?;
        source.digest = digest;
        tracing::info!(generation, "JWT keyring generation published");
        Ok(())
    }
}

async fn load_config() -> Result<HostConfig> {
    let environment =
        crate::common::settings::resolve_environment_name().map_err(|err| Error::BadRequest(err.to_string()))?;

    let configured_config_dir = std::env::var("RUSTOK_CONFIG_DIR")
        .ok()
        .map(|value| PathBuf::from(value.trim()))
        .filter(|path| !path.as_os_str().is_empty());
    let config_dir = if let Some(path) = configured_config_dir {
        path
    } else {
        let executable_config_dir = std::env::current_exe()
            .ok()
            .and_then(|executable| executable.parent().map(PathBuf::from))
            .map(|parent| parent.join("config"));
        match executable_config_dir {
            Some(path) => match tokio::fs::metadata(&path).await {
                Ok(metadata) if metadata.is_dir() => path,
                _ => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config"),
            },
            None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config"),
        }
    };
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
                        if let Err(_error) = result {
                            tracing::warn!(
                                failure_code = "host_ctrl_c_signal_failed",
                                "Failed to receive Ctrl-C shutdown signal"
                            );
                        } else {
                            tracing::info!("RusTok Axum host received Ctrl-C");
                        }
                    }
                    _ = terminate.recv() => {
                        tracing::info!("RusTok Axum host received SIGTERM");
                    }
                }
            }
            Err(_error) => {
                tracing::warn!(
                    failure_code = "host_sigterm_handler_registration_failed",
                    "Failed to register SIGTERM handler; falling back to Ctrl-C shutdown handling"
                );
                if let Err(_error) = tokio::signal::ctrl_c().await {
                    tracing::warn!(
                        failure_code = "host_shutdown_signal_failed",
                        "Failed to receive shutdown signal"
                    );
                } else {
                    tracing::info!("RusTok Axum host received Ctrl-C");
                }
            }
        }
    }

    #[cfg(not(unix))]
    if let Err(_error) = tokio::signal::ctrl_c().await {
        tracing::warn!(
            failure_code = "host_shutdown_signal_failed",
            "Failed to receive shutdown signal"
        );
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

    #[test]
    fn jwt_file_source_cannot_be_combined_with_startup_crypto_overrides() {
        assert!(super::validate_keyring_file_source(&serde_json::json!({
            "auth": {"issuer": "https://api.example.com", "min_password_length": 15}
        })).is_ok());
        for field in ["algorithm", "jwt_keyring_env", "jwt_special_keyrings_env",
            "rsa_private_key_pem", "rsa_public_key_pem", "rsa_private_key_env",
            "rsa_public_key_env"] {
            let mut value = serde_json::json!({"auth": {}});
            value["auth"][field] = serde_json::json!("configured");
            assert!(super::validate_keyring_file_source(&value).is_err());
        }
        let rendered = format!("{:?}", super::JwtConfig {
            secret: "sensitive-root-value".to_string(), expiration: 900,
        });
        assert!(!rendered.contains("sensitive-root-value"));
    }

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
    fn oauth_metadata_requires_an_explicit_issuer_when_enabled() {
        let config = crate::auth::AuthConfig::new(
            "development-secret-value-with-more-than-thirty-two-bytes".to_string(),
        );

        let error = validate_oauth_metadata_issuer(&config, true)
            .expect_err("OAuth discovery must not publish an issuer different from JWT issuer claims");
        assert!(error.to_string().contains("absolute issuer URL"));
    }

    #[test]
    fn oauth_metadata_accepts_https_root_and_localhost_http_issuers() {
        for issuer in [
            "https://api.example.com",
            "https://api.example.com/",
            "http://localhost:3000",
            "http://127.0.0.1:3000",
        ] {
            let config = crate::auth::AuthConfig::new(
                "development-secret-value-with-more-than-thirty-two-bytes".to_string(),
            )
            .with_issuer(issuer);
            validate_oauth_metadata_issuer(&config, true)
                .unwrap_or_else(|error| panic!("supported issuer must be accepted ({issuer}): {error}"));
        }
    }

    #[test]
    fn oauth_metadata_rejects_unsupported_issuer_forms_when_enabled() {
        for issuer in [
            "rustok",
            "http://api.example.com",
            "https://api.example.com/issuer",
            "https://api.example.com?tenant=one",
            "https://api.example.com#tenant-one",
            "https://user:password@api.example.com",
            "https://@api.example.com",
            "https://api.example.com ",
        ] {
            let config = crate::auth::AuthConfig::new(
                "development-secret-value-with-more-than-thirty-two-bytes".to_string(),
            )
            .with_issuer(issuer);
            assert!(
                validate_oauth_metadata_issuer(&config, true).is_err(),
                "unsupported issuer must be rejected: {issuer}"
            );
        }
    }

    #[test]
    fn oauth_metadata_issuer_validation_is_skipped_when_oauth_is_disabled() {
        let config = crate::auth::AuthConfig::new(
            "development-secret-value-with-more-than-thirty-two-bytes".to_string(),
        )
        .with_issuer("https://api.example.com/issuer");

        validate_oauth_metadata_issuer(&config, false)
            .expect("JWT-only deployments do not publish OAuth discovery metadata");
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
    fn production_rejects_short_special_jwt_key_material() {
        let config = rustok_auth::build_auth_config_with_env(
            "aB3!zY7@qW8#eR2$".repeat(5),
            900,
            crate::auth::AuthSettingsOverrides {
                issuer: Some("https://api.example.com".into()),
                audience: Some("rustok-production-admin".into()),
                jwt_special_keyrings_env: Some("SPECIAL_KEYS".into()),
                ..Default::default()
            },
            |_| Ok(serde_json::json!({
                "invite": {"keyring": {
                    "active_kid": "invite-next",
                    "active_hs256_secret": "distinct-special-signing-key-with-32-bytes"
                }}
            }).to_string()),
        ).expect("owner accepts the development key-strength floor");
        let error = validate_auth_deployment(&config, true)
            .expect_err("production must validate all purpose keys");
        assert!(error.to_string().contains("at least 64 bytes"));
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

    fn database_config_for_validation() -> super::DatabaseConfig {
        super::DatabaseConfig {
            uri: "postgres://user:password@localhost:5432/rustok".to_string(),
            auto_migrate: false,
            enable_logging: false,
            connect_timeout: 0,
            idle_timeout: 0,
            min_connections: 0,
            max_connections: 0,
        }
    }

    #[test]
    fn validate_auth_deployment_rejects_https_issuer_without_a_host() {
        let config = crate::auth::AuthConfig::new("aB3!zY7@qW8#eR2$".repeat(5))
            .with_issuer("https://?")
            .with_audience("rustok-production-admin");
        let error = validate_auth_deployment(&config, true)
            .expect_err("an HTTPS issuer without a host must fail");
        assert!(error.to_string().contains("issuer"));
    }

    #[test]
    fn database_minimum_is_checked_against_the_effective_default_maximum() {
        let mut config = database_config_for_validation();
        config.min_connections = super::DEFAULT_DATABASE_MAX_CONNECTIONS + 1;
        let error = super::validate_database_connection_settings(&config)
            .expect_err("minimum above the implicit default maximum must fail");
        assert!(error.to_string().contains("cannot exceed max_connections"));

        config.min_connections = super::DEFAULT_DATABASE_MAX_CONNECTIONS;
        super::validate_database_connection_settings(&config)
            .expect("minimum equal to the implicit default maximum is valid");
    }

    #[test]
    fn database_pool_sizes_and_timeouts_have_safe_upper_bounds() {
        let mut config = database_config_for_validation();
        config.max_connections = super::MAX_DATABASE_CONNECTIONS + 1;
        assert!(
            super::validate_database_connection_settings(&config)
                .expect_err("excessive max_connections must fail")
                .to_string()
                .contains("max_connections")
        );

        config.max_connections = 30;
        config.min_connections = 5;
        config.connect_timeout = super::MAX_DATABASE_CONNECT_TIMEOUT_MS + 1;
        assert!(
            super::validate_database_connection_settings(&config)
                .expect_err("excessive connect timeout must fail")
                .to_string()
                .contains("connect_timeout")
        );

        config.connect_timeout = 0;
        config.idle_timeout = super::MAX_DATABASE_IDLE_TIMEOUT_MS + 1;
        assert!(
            super::validate_database_connection_settings(&config)
                .expect_err("excessive idle timeout must fail")
                .to_string()
                .contains("idle_timeout")
        );
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
