//! Pure Axum executable host composition.

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

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
        server_bootstrap::{
            bootstrap_application_router, check_production_environment,
            initialize_server_context, known_placeholder_fragment,
        },
        server_runtime_context::{ServerAuthRuntime, ServerRuntimeContext},
    },
};

const DEFAULT_JWT_ISSUER: &str = "rustok";
const DEFAULT_JWT_AUDIENCE: &str = "rustok-admin";
const JWT_KEYRING_FILE_ENV: &str = "RUSTOK_JWT_KEYRING_FILE";
const JWT_RELOAD_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HostConfig {
    server: ServerConfig,
    database: DatabaseConfig,
    auth: AuthConfig,
    #[serde(default)]
    settings: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ServerConfig {
    binding: String,
    port: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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

impl std::fmt::Debug for DatabaseConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseConfig")
            .field("uri_configured", &!self.uri.trim().is_empty())
            .field("auto_migrate", &self.auto_migrate)
            .field("enable_logging", &self.enable_logging)
            .field("connect_timeout", &self.connect_timeout)
            .field("idle_timeout", &self.idle_timeout)
            .field("min_connections", &self.min_connections)
            .field("max_connections", &self.max_connections)
            .finish()
    }
}

impl std::fmt::Debug for HostConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostConfig")
            .field("server", &self.server)
            .field("database", &self.database)
            .field("auth", &self.auth)
            .field("settings_configured", &!self.settings.is_null())
            .finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthConfig {
    jwt: JwtConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JwtConfig {
    #[serde(rename = "secret")]
    mac_root: String,
    expiration: u64,
}

impl std::fmt::Debug for JwtConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JwtConfig")
            .field("mac_root", &"<redacted>")
            .field("expiration", &self.expiration)
            .finish()
    }
}

/// Validates the same complete JWT bootstrap used by the executable host.
///
/// This command is intentionally database-free so development migrations can
/// fail closed before changing application schema.
pub async fn validate_jwt_keyring_bootstrap() -> Result<()> {
    let mut config = load_config().await?;
    apply_auth_environment_overrides(&mut config)?;
    let production = crate::common::is_production_security_mode();
    let rustok_settings = RustokSettings::from_settings(&Some(config.settings.clone()))
        .map_err(|error| Error::BadRequest(format!("Invalid rustok settings: {error}")))?;
    let prepared = prepare_jwt_bootstrap(&config, production).await?;
    validate_oauth_metadata_issuer(
        &prepared.auth_config,
        crate::routes::oauth_surface_enabled(rustok_settings.runtime.host_mode, &rustok_settings),
    )?;
    Ok(())
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

    let mut config = config;
    apply_auth_environment_overrides(&mut config)?;
    apply_database_environment_overrides(&mut config.database)?;
    let database_url = read_optional_env("DATABASE_URL")?;
    let database_uri = resolve_database_uri(database_url.as_deref(), &config.database.uri);
    let production = crate::common::is_production_security_mode();
    validate_database_deployment(&database_uri, production)?;
    validate_database_runtime_policy(&config.database, production)?;
    check_production_environment(&config.auth.jwt.mac_root, &database_uri, production)?;
    validate_https_deployment(production, hsts_enabled())?;

    // Validate all settings-derived policy before connecting to the database or
    // applying migrations, so a bad issuer/secret cannot mutate persistent state.
    let rustok_settings = RustokSettings::from_settings(&Some(config.settings.clone()))
        .map_err(|error| Error::BadRequest(format!("Invalid rustok settings: {error}")))?;
    let prepared = prepare_jwt_bootstrap(&config, production).await?;
    let PreparedJwtBootstrap {
        generation,
        auth_config,
        keyring_path,
        digest,
    } = prepared;
    validate_oauth_metadata_issuer(
        &auth_config,
        crate::routes::oauth_surface_enabled(rustok_settings.runtime.host_mode, &rustok_settings),
    )?;

    // Refuse invalid independent authority before app DB migrations and
    // honor early shutdown while opening the authority.
    let authority = rustok_auth::JwtAuthorityConfig::from_env().map_err(crate::auth::auth_err)?;
    let independent_fence = match authority {
        Some(authority) => Some(tokio::select! {
            biased;
            _ = &mut shutdown_signal => return Ok(()),
            result = authority.open_for(&auth_config) => result.map_err(crate::auth::auth_err)?,
        }),
        None if !production => None,
        None => {
            return Err(Error::Message(
                "Independent JWT authority and rollback witness are required in production".into(),
            ));
        }
    };

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

    // Reserve a separate clock writer pool before serving any Auth traffic.
    // It never borrows from the domain pool while a transaction holds locks.
    tokio::select! {
        biased;
        _ = &mut shutdown_signal => return Ok(()),
        result = rustok_auth::application_clock::configure_late_observation_store(&db, &database_uri) => {
            result.map_err(Error::Database)?;
        }
    }

    // Production never takes JWT authority from the restorable application DB.
    let fence =
        independent_fence.unwrap_or_else(|| rustok_auth::JwtGenerationFence::new(db.clone()));
    loop {
        let admission = tokio::select! {
            biased;
            _ = &mut shutdown_signal => return Ok(()),
            result = fence.admit(generation, &auth_config) => result.map_err(crate::auth::auth_err)?,
        };
        if admission == rustok_auth::JwtAdmission::Admitted {
            break;
        }
        tokio::select! {
            biased;
            _ = &mut shutdown_signal => return Ok(()),
            _ = tokio::time::sleep(JWT_RELOAD_INTERVAL) => {}
        }
    }
    let (auth_config, publisher) = auth_config
        .into_cluster_jwt(generation)
        .map_err(crate::auth::auth_err)?;
    let supervisor = JwtRuntimeSupervisor {
        source: JwtFileSource {
            path: keyring_path,
            digest,
        },
        publisher,
        fence,
    };
    if !supervisor
        .fence
        .renew(&supervisor.publisher)
        .await
        .map_err(crate::auth::auth_err)?
    {
        return Err(Error::Message("JWT cluster lease denied at startup".into()));
    }
    tracing::info!(generation, "JWT keyring runtime initialized");

    let runtime_ctx = ServerRuntimeContext::new(db, rustok_settings.clone());
    runtime_ctx.shared_insert(
        supervisor
            .publisher
            .lease_readiness()
            .map_err(crate::auth::auth_err)?,
    );
    runtime_ctx.shared_insert(
        supervisor
            .fence
            .operational_metrics(&supervisor.publisher)
            .map_err(crate::auth::auth_err)?,
    );

    let result = with_runtime_shutdown(&runtime_ctx, async {
        let mut supervisor = Box::pin(supervisor.run(production));
        let startup = async {
            initialize_server_context(&runtime_ctx, &config.auth.jwt.mac_root, &database_uri)
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

    crate::controllers::oauth_metadata::parse_metadata_issuer(&config.issuer)
        .map_err(|error| Error::BadRequest(error.to_string()))?;

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

    config
        .validate_production_mac_root()
        .map_err(crate::auth::auth_err)?;
    rustok_auth::validate_auth_config(config).map_err(crate::auth::auth_err)?;

    Ok(())
}

fn validate_database_deployment(database_uri: &str, production: bool) -> Result<()> {
    if !production {
        return Ok(());
    }

    let database_uri = database_uri.trim();
    if database_uri.is_empty() {
        return Err(Error::BadRequest(
            "Production database URI must not be empty".to_string(),
        ));
    }

    if database_uri
        .split_once(':')
        .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("sqlite"))
    {
        return Err(Error::BadRequest(
            "Production database configuration must not use development credentials or local placeholders"
                .to_string(),
        ));
    }

    let parsed = url::Url::parse(database_uri).map_err(|_| {
        Error::BadRequest("Production database URI must be a valid absolute URL".to_string())
    })?;
    if parsed.fragment().is_some() {
        return Err(Error::BadRequest(
            "Production database URI must not contain a URL fragment; percent-encode reserved credential characters"
                .to_string(),
        ));
    }

    let username = decode_percent_encoded_component(parsed.username()).ok_or_else(|| {
        Error::BadRequest("Production database URI contains invalid percent-encoding".to_string())
    })?;
    let password = match parsed.password() {
        Some(password) => decode_percent_encoded_component(password).ok_or_else(|| {
            Error::BadRequest(
                "Production database URI contains invalid percent-encoding".to_string(),
            )
        })?,
        None => String::new(),
    };

    let username = username.to_ascii_lowercase();
    let password = password.to_ascii_lowercase();
    let uses_development_credentials = match username.as_str() {
        "rustok" => password.starts_with("rustok"),
        "postgres" => password.starts_with("postgres"),
        "admin" => password.starts_with("admin"),
        "root" => password.starts_with("root"),
        "user" => password.starts_with("password"),
        _ => false,
    };
    let uses_placeholder_credentials = known_placeholder_fragment(&username).is_some()
        || known_placeholder_fragment(&password).is_some();

    // PostgreSQL also accepts `host` in the query string. Inspect both URL authority
    // and query-parameter hosts so a hostless URI cannot silently fall back to a
    // local socket/default endpoint, and an override cannot bypass this check.
    let authority_host = parsed
        .host_str()
        .filter(|host| !host.trim().is_empty())
        .ok_or_else(|| {
            Error::BadRequest(
                "Production database URI must specify an explicit host in its authority"
                    .to_string(),
            )
        })?;
    let mut configured_hosts = vec![authority_host.to_owned()];
    configured_hosts.extend(parsed.query_pairs().filter_map(|(key, value)| {
        key.eq_ignore_ascii_case("host").then(|| value.into_owned())
    }));
    if configured_hosts.iter().any(|host| host.trim().is_empty()) {
        return Err(Error::BadRequest(
            "Production database URI contains an invalid host".to_string(),
        ));
    }

    let hosts = configured_hosts
        .iter()
        .flat_map(|host| host.split(','))
        .map(|host| {
            host.trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim_end_matches('.')
                .to_ascii_lowercase()
        })
        .collect::<Vec<_>>();
    if hosts.is_empty() || hosts.iter().any(String::is_empty) {
        return Err(Error::BadRequest(
            "Production database URI contains an invalid host".to_string(),
        ));
    }

    let host_is_local = hosts.iter().any(|host| {
        host.starts_with('/')
            || host.starts_with("unix:")
            || host == "localhost"
            || host == "localhost.localdomain"
            || host.ends_with(".localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback() || address.is_unspecified())
    });
    let host_is_placeholder = hosts.iter().any(|host| {
        matches!(
            host.as_str(),
            "example.com" | "example.net" | "example.org"
        ) || host.ends_with(".example.com")
            || host.ends_with(".example.net")
            || host.ends_with(".example.org")
            || host.ends_with(".example")
            || host.ends_with(".invalid")
            || host.ends_with(".test")
    });
    let database_name = decode_percent_encoded_component(parsed.path().trim_matches('/'))
        .ok_or_else(|| {
            Error::BadRequest(
                "Production database URI contains invalid percent-encoding".to_string(),
            )
        })?;
    let uses_local_development_database =
        host_is_local && database_name.eq_ignore_ascii_case("rustok_dev");

    if uses_development_credentials
        || uses_placeholder_credentials
        || uses_local_development_database
        || host_is_placeholder
        || parsed.scheme().eq_ignore_ascii_case("sqlite")
    {
        return Err(Error::BadRequest(
            "Production database configuration must not use development credentials or local placeholders"
                .to_string(),
        ));
    }

    Ok(())
}

fn decode_percent_encoded_component(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = decode_hex_digit(*bytes.get(index + 1)?)?;
            let low = decode_hex_digit(*bytes.get(index + 2)?)?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).ok()
}

fn decode_hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn application_router(
    host_mode: crate::common::settings::RuntimeHostMode,
    settings: &crate::common::settings::RustokSettings,
) -> ServerRouter {
    crate::routes::build_application_router(host_mode, settings)
}

fn resolve_database_uri(database_url: Option<&str>, configured_uri: &str) -> String {
    database_url.unwrap_or(configured_uri).trim().to_string()
}

fn read_optional_env(name: &str) -> Result<Option<String>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(Error::BadRequest(format!(
            "{name} must be valid UTF-8"
        ))),
    }
}

fn parse_env_value<T>(name: &str, value: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value.trim().parse().map_err(|error| {
        Error::BadRequest(format!("Invalid {name} value: {error}"))
    })
}

fn validate_server_port(name: &str, port: u16) -> Result<u16> {
    if port == 0 {
        return Err(Error::BadRequest(format!(
            "Invalid port '0' in {name}: port must be between 1 and 65535"
        )));
    }

    Ok(port)
}

fn parse_server_port(name: &str, value: &str) -> Result<u16> {
    let port = value.trim().parse::<u16>().map_err(|error| {
        Error::BadRequest(format!("Invalid port '{value}' in {name}: {error}"))
    })?;
    validate_server_port(name, port)
}

fn parse_env_bool(name: &str, value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(Error::BadRequest(format!(
            "Invalid boolean value for {name}; expected true or false"
        ))),
    }
}

fn apply_auth_environment_overrides(config: &mut HostConfig) -> Result<()> {
    if let Some(secret) = read_optional_env("RUSTOK_JWT_SECRET")? {
        config.auth.jwt.mac_root = secret;
    } else if let Some(secret) = read_optional_env("JWT_SECRET")? {
        config.auth.jwt.mac_root = secret;
    }

    if let Some(expiration) = read_optional_env("JWT_EXPIRATION")? {
        config.auth.jwt.expiration = parse_env_value("JWT_EXPIRATION", &expiration)?;
    }

    let mut overrides = serde_json::Map::new();
    if let Some(expiration) = read_optional_env("REFRESH_TOKEN_EXPIRATION")? {
        overrides.insert(
            "refresh_expiration".to_string(),
            serde_json::Value::from(parse_env_value::<u64>(
                "REFRESH_TOKEN_EXPIRATION",
                &expiration,
            )?),
        );
    }
    if let Some(issuer) = read_optional_env("JWT_ISSUER")? {
        overrides.insert("issuer".to_string(), serde_json::Value::String(issuer));
    }
    if let Some(audience) = read_optional_env("JWT_AUDIENCE")? {
        overrides.insert("audience".to_string(), serde_json::Value::String(audience));
    }

    if !overrides.is_empty() {
        if config.settings.is_null() {
            config.settings = serde_json::Value::Object(serde_json::Map::new());
        }
        let settings = config.settings.as_object_mut().ok_or_else(|| {
            Error::BadRequest("Server settings must be an object for auth environment overrides".into())
        })?;
        let auth = settings
            .entry("auth".to_string())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        let auth = auth.as_object_mut().ok_or_else(|| {
            Error::BadRequest("settings.auth must be an object for auth environment overrides".into())
        })?;
        auth.extend(overrides);
    }

    Ok(())
}

fn apply_database_environment_overrides(config: &mut DatabaseConfig) -> Result<()> {
    if let Some(value) = read_optional_env("DATABASE_MIN_CONNECTIONS")? {
        config.min_connections = parse_env_value("DATABASE_MIN_CONNECTIONS", &value)?;
    }
    if let Some(value) = read_optional_env("DATABASE_MAX_CONNECTIONS")? {
        config.max_connections = parse_env_value("DATABASE_MAX_CONNECTIONS", &value)?;
    }
    if let Some(value) = read_optional_env("DATABASE_CONNECT_TIMEOUT")? {
        config.connect_timeout = parse_env_value("DATABASE_CONNECT_TIMEOUT", &value)?;
    }
    if let Some(value) = read_optional_env("DATABASE_IDLE_TIMEOUT")? {
        config.idle_timeout = parse_env_value("DATABASE_IDLE_TIMEOUT", &value)?;
    }
    if let Some(value) = read_optional_env("DATABASE_ENABLE_LOGGING")? {
        config.enable_logging = parse_env_bool("DATABASE_ENABLE_LOGGING", &value)?;
    }
    if let Some(value) = read_optional_env("DATABASE_AUTO_MIGRATE")? {
        config.auto_migrate = parse_env_bool("DATABASE_AUTO_MIGRATE", &value)?;
    }
    Ok(())
}

const DEFAULT_DATABASE_MAX_CONNECTIONS: u32 = 10;
const MAX_DATABASE_CONNECTIONS: u32 = 1_024;
const MAX_DATABASE_CONNECT_TIMEOUT_MS: u64 = 300_000;
const MAX_DATABASE_IDLE_TIMEOUT_MS: u64 = 86_400_000;

fn database_connect_timeout(config: &DatabaseConfig) -> Duration {
    if config.connect_timeout > 0 {
        Duration::from_millis(config.connect_timeout)
    } else {
        Duration::from_secs(10)
    }
}

fn database_idle_timeout(config: &DatabaseConfig) -> Option<Duration> {
    (config.idle_timeout > 0).then(|| Duration::from_millis(config.idle_timeout))
}

fn validate_database_runtime_policy(
    config: &DatabaseConfig,
    production: bool,
) -> Result<()> {
    validate_database_connection_settings(config)?;

    if production && config.auto_migrate {
        return Err(Error::BadRequest(
            concat!(
                "database.auto_migrate must be false in production; ",
                "apply migrations as a controlled deployment step"
            )
            .to_string(),
        ));
    }
    if production && config.enable_logging {
        return Err(Error::BadRequest(
            "database.enable_logging must be false in production to avoid logging sensitive SQL parameters"
                .to_string(),
        ));
    }

    Ok(())
}

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
    let timeout = database_connect_timeout(config);
    options.connect_timeout(timeout);
    options.acquire_timeout(timeout);
    if let Some(idle_timeout) = database_idle_timeout(config) {
        options.idle_timeout(idle_timeout);
    }
    if config.min_connections > 0 {
        options.min_connections(config.min_connections);
    }
    if config.max_connections > 0 {
        options.max_connections(config.max_connections);
    }
    Database::connect(options).await.map_err(Error::Database)
}

fn required_jwt_keyring_file_path_from(value: Option<&str>) -> Result<PathBuf> {
    let value = value
        .filter(|value| !value.trim().is_empty() && value.trim() == *value)
        .ok_or_else(|| {
            Error::BadRequest(format!(
                "{JWT_KEYRING_FILE_ENV} is required and must be a non-empty path"
            ))
        })?;
    Ok(PathBuf::from(value))
}

fn jwt_keyring_file_path() -> Result<PathBuf> {
    let value = read_optional_env(JWT_KEYRING_FILE_ENV)?;
    required_jwt_keyring_file_path_from(value.as_deref())
}

struct PreparedJwtBootstrap {
    generation: u64,
    auth_config: crate::auth::AuthConfig,
    keyring_path: PathBuf,
    digest: [u8; 32],
}

async fn prepare_jwt_bootstrap(
    config: &HostConfig,
    production: bool,
) -> Result<PreparedJwtBootstrap> {
    let keyring_path = jwt_keyring_file_path()?;
    let bytes = read_jwt_keyring_file(&keyring_path).await?;
    let digest = sha256_digest(&[&bytes]);
    let (generation, auth_config) = crate::auth::auth_config_from_host_document(
        config.auth.jwt.mac_root.clone(),
        config.auth.jwt.expiration,
        Some(&config.settings),
        &bytes,
    )?;
    validate_auth_deployment(&auth_config, production)?;
    Ok(PreparedJwtBootstrap {
        generation,
        auth_config,
        keyring_path,
        digest,
    })
}

async fn read_jwt_keyring_file(path: &Path) -> Result<Vec<u8>> {
    const MAX: usize = rustok_auth::MAX_JWT_KEYRING_DOCUMENT_BYTES;
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|_| Error::Message("Cannot read JWT keyring file".to_string()))?;
    if !metadata.is_file() || metadata.len() > MAX as u64 {
        return Err(Error::BadRequest(
            "Invalid JWT keyring file size or type".to_string(),
        ));
    }
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|_| Error::Message("Cannot open JWT keyring file".to_string()))?;
    let mut bytes = Vec::new();
    file.take((MAX + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| Error::Message("Cannot read JWT keyring file".to_string()))?;
    if bytes.len() > MAX {
        return Err(Error::BadRequest(
            "JWT keyring file exceeds size limit".to_string(),
        ));
    }
    Ok(bytes)
}

struct JwtFileSource {
    path: PathBuf,
    digest: [u8; 32],
}

struct JwtRuntimeSupervisor {
    source: JwtFileSource,
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
            // Check the durable revocation/lease authority even when the
            // file has not changed, before potentially slow file processing.
            let _granted = self
                .fence
                .renew(&self.publisher)
                .await
                .map_err(crate::auth::auth_err)?;
            Self::poll_source(&mut self.source, &self.publisher, &self.fence, production).await?;
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
                fence.record_reload(rustok_auth::JwtReloadOutcome::SourceUnavailable);
                tracing::warn!("JWT keyring reload source unavailable");
                return Ok(());
            }
        };
        let digest = sha256_digest(&[&bytes]);
        if digest == source.digest {
            fence.record_reload(rustok_auth::JwtReloadOutcome::Unchanged);
            return Ok(());
        }
        let (generation, candidate) = match publisher.prepare_document(&bytes) {
            Ok(candidate) => candidate,
            Err(_) => {
                source.digest = digest;
                fence.record_reload(rustok_auth::JwtReloadOutcome::Rejected);
                tracing::warn!("JWT keyring reload rejected");
                return Ok(());
            }
        };
        if generation <= publisher.generation().map_err(crate::auth::auth_err)?
            || validate_auth_deployment(&candidate, production).is_err()
        {
            source.digest = digest;
            fence.record_reload(rustok_auth::JwtReloadOutcome::Rejected);
            tracing::warn!("JWT keyring reload rejected");
            return Ok(());
        }
        match fence.admit(generation, &candidate).await {
            Ok(rustok_auth::JwtAdmission::Admitted) => {}
            // Do not mark the file processed: the same candidate must retry
            // after the old database lease horizon has drained.
            Ok(rustok_auth::JwtAdmission::Draining) => {
                fence.record_reload(rustok_auth::JwtReloadOutcome::WaitingDrain(generation));
                return Ok(());
            }
            Ok(rustok_auth::JwtAdmission::AwaitingReplicas) => {
                fence.record_reload(rustok_auth::JwtReloadOutcome::WaitingReplicas(generation));
                return Ok(());
            }
            Err(rustok_auth::AuthError::Validation(_)) => {
                source.digest = digest;
                fence.record_reload(rustok_auth::JwtReloadOutcome::Rejected);
                tracing::warn!("JWT keyring generation rejected by durable fence");
                return Ok(());
            }
            Err(error) => return Err(crate::auth::auth_err(error)),
        }
        if !fence
            .activate(publisher, generation, candidate)
            .await
            .map_err(crate::auth::auth_err)?
        {
            // The current local lease remains intact. Retry this exact file
            // until a matching grant is obtained or a newer file arrives.
            fence.record_reload(rustok_auth::JwtReloadOutcome::WaitingGrant(generation));
            return Ok(());
        }
        source.digest = digest;
        fence.record_reload(rustok_auth::JwtReloadOutcome::Published);
        tracing::info!(generation, "JWT keyring generation published");
        Ok(())
    }
}

async fn load_config() -> Result<HostConfig> {
    let environment = crate::common::settings::resolve_environment_name()
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let configured_config_dir = std::env::var_os("RUSTOK_CONFIG_DIR")
        .map(|value| match value.to_str() {
            Some(value) => PathBuf::from(value.trim()),
            None => PathBuf::from(value.as_os_str()),
        })
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
    let binding = match read_optional_env("RUSTOK_BINDING")? {
        Some(binding) => binding,
        None => read_optional_env("HOST")?.unwrap_or_else(|| config.binding.clone()),
    }
    .trim()
    .to_string();
    if binding.is_empty() {
        return Err(Error::BadRequest(
            "Server binding address must not be empty".to_string(),
        ));
    }

    let port = match read_optional_env("RUSTOK_PORT")? {
        Some(port) => parse_server_port("RUSTOK_PORT", &port)?,
        None => match read_optional_env("PORT")? {
            Some(port) => parse_server_port("PORT", &port)?,
            None => config.port,
        },
    };
    let port = validate_server_port("server.port", port)?;
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
        HostConfig, application_router, required_jwt_keyring_file_path_from, resolve_database_uri,
        validate_auth_deployment, validate_https_deployment, validate_oauth_metadata_issuer,
    };
    use crate::auth::test_auth_config;
    use crate::common::settings::RuntimeHostMode;

    #[test]
    fn jwt_keyring_file_path_is_mandatory_and_canonical() {
        for invalid in [
            None,
            Some(""),
            Some("   "),
            Some(" /run/rustok-jwt/jwt.json"),
            Some("/run/rustok-jwt/jwt.json "),
        ] {
            assert!(required_jwt_keyring_file_path_from(invalid).is_err());
        }
        assert_eq!(
            required_jwt_keyring_file_path_from(Some("/run/rustok-jwt/jwt.json")).unwrap(),
            std::path::PathBuf::from("/run/rustok-jwt/jwt.json"),
        );
    }

    #[test]
    fn host_config_debug_redacts_database_uri_and_raw_settings() {
        let config = HostConfig {
            server: super::ServerConfig {
                binding: "127.0.0.1".to_string(),
                port: 8080,
            },
            database: super::DatabaseConfig {
                uri: "postgres://db-user:db-password@db.example/rustok".to_string(),
                auto_migrate: false,
                enable_logging: false,
                connect_timeout: 5,
                idle_timeout: 30,
                min_connections: 1,
                max_connections: 10,
            },
            auth: super::AuthConfig {
                jwt: super::JwtConfig {
                    mac_root: "jwt-secret-value".to_string(),
                    expiration: 900,
                },
            },
            settings: serde_json::json!({
                "rustok": {
                    "cache": {
                        "redis_url": "redis://:settings-secret@cache.example/0"
                    }
                }
            }),
        };

        let debug = format!("{config:?}");
        for secret in [
            "db-user",
            "db-password",
            "jwt-secret-value",
            "settings-secret",
        ] {
            assert!(
                !debug.contains(secret),
                "Debug output leaked {secret}: {debug}"
            );
        }
    }

    #[test]
    fn jwt_config_debug_redacts_root() {
        let rendered = format!(
            "{:?}",
            super::JwtConfig {
                mac_root: "sensitive-root-value".to_string(),
                expiration: 900,
            }
        );
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
    fn host_auth_config_rejects_legacy_algorithm_and_key_source_fields() {
        let algorithm_error = serde_yaml::from_str::<super::AuthConfig>(
            "jwt:\n  secret: test-mac-root\n  expiration: 3600\n  algorithm: HS256\n",
        )
        .expect_err("legacy JWT algorithm selectors must be rejected");
        assert!(algorithm_error.to_string().contains("unknown field"));

        let key_source_error = serde_yaml::from_str::<super::AuthConfig>(
            "jwt:\n  secret: test-mac-root\n  expiration: 3600\nkeyring_file: /tmp/keys.json\n",
        )
        .expect_err("alternate auth key sources must be rejected");
        assert!(key_source_error.to_string().contains("unknown field"));
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
        assert_eq!(
            resolve_database_uri(
                Some("  postgres://u:p@db:5432/rustok  "),
                "postgres://fallback/rustok"
            ),
            "postgres://u:p@db:5432/rustok"
        );
        assert_eq!(
            resolve_database_uri(None, "  postgres://u:p@db:5432/rustok  "),
            "postgres://u:p@db:5432/rustok"
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
        let config = test_auth_config(
            "development-secret-value-with-more-than-thirty-two-bytes".to_string(),
        );
        validate_auth_deployment(&config, false)
            .expect("development may use the framework claim defaults");
    }

    #[test]
    fn oauth_metadata_requires_an_explicit_issuer_when_enabled() {
        let config = test_auth_config(
            "development-secret-value-with-more-than-thirty-two-bytes".to_string(),
        );

        let error = validate_oauth_metadata_issuer(&config, true).expect_err(
            "OAuth discovery must not publish an issuer different from JWT issuer claims",
        );
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
            let config = test_auth_config(
                "development-secret-value-with-more-than-thirty-two-bytes".to_string(),
            )
            .with_issuer(issuer);
            validate_oauth_metadata_issuer(&config, true).unwrap_or_else(|error| {
                panic!("supported issuer must be accepted ({issuer}): {error}")
            });
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
            "https:api.example.com",
            "https:///api.example.com",
            "https://api.example.com/issuer/..",
            "https://api.example.com/%2e",
            "https://api.example.com\\",
            "https://api.example.com\u{0001}",
        ] {
            let config = test_auth_config(
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
        let config = test_auth_config(
            "development-secret-value-with-more-than-thirty-two-bytes".to_string(),
        )
        .with_issuer("https://api.example.com/issuer");

        validate_oauth_metadata_issuer(&config, false)
            .expect("JWT-only deployments do not publish OAuth discovery metadata");
    }

    #[test]
    fn production_rejects_default_jwt_claims() {
        let config = test_auth_config("aB3!".repeat(20));
        let error = validate_auth_deployment(&config, true)
            .expect_err("production must not share the framework claim namespace");
        assert!(error.to_string().contains("issuer"));
    }

    #[test]
    fn production_rejects_short_mac_root() {
        let config = test_auth_config("aB3!".repeat(10))
            .with_issuer("https://api.example.com")
            .with_audience("rustok-production-admin");
        let error = validate_auth_deployment(&config, true)
            .expect_err("production MAC root needs a stronger secret floor");
        assert!(error.to_string().contains("at least 64 bytes"));
    }

    #[test]
    fn production_accepts_explicit_rsa_and_strong_mac_policy() {
        let config = test_auth_config("aB3!zY7@qW8#eR2$".repeat(5))
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
            "postgres://app_user:K8x%23mP9%24vL2@prod-db.internal:5432/rustok_prod",
            true,
        )
        .expect("strong production credentials should pass");
    }

    #[test]
    fn production_rejects_placeholder_database_credentials_before_connecting() {
        for uri in [
            "postgres://app_user:CHANGE_ME@prod-db.internal:5432/rustok_prod",
            "postgres://app_user:CHANGE%5FME@prod-db.internal:5432/rustok_prod",
        ] {
            let error = super::validate_database_deployment(uri, true)
                .expect_err("placeholder credentials must fail even when percent-encoded");
            assert!(error.to_string().contains("development credentials"));
        }
    }

    #[test]
    fn production_rejects_reserved_example_database_hosts() {
        let error = super::validate_database_deployment(
            "postgres://app_user:strong-secret@db.example.com:5432/rustok_prod",
            true,
        )
        .expect_err("reserved example domains are documentation placeholders");
        assert!(error.to_string().contains("local placeholders"));
    }

    #[test]
    fn production_rejects_unspecified_database_addresses() {
        for uri in [
            "postgres://app_user:strong-secret@0.0.0.0:5432/rustok_prod",
            "postgres://app_user:strong-secret@[::]:5432/rustok_prod",
            "postgres://app_user:strong-secret@[::1]:5432/rustok_prod",
        ] {
            let error = super::validate_database_deployment(uri, true)
                .expect_err("unspecified addresses are not production database endpoints");
            assert!(error.to_string().contains("local placeholders"));
        }
    }

    #[test]
    fn production_rejects_database_uris_without_an_explicit_host() {
        for uri in [
            "postgres:///rustok_prod",
            "postgres:///rustok_prod?application_name=rustok",
            "postgres:///rustok_prod?host=prod-db.internal",
        ] {
            let error = super::validate_database_deployment(uri, true).expect_err(
                "hostless production database URIs must not use implicit local defaults",
            );
            assert!(error.to_string().contains("explicit host"));
        }
    }

    #[test]
    fn production_validates_libpq_host_query_parameters() {
        for uri in [
            "postgres://app:secret@prod-db.internal/rustok_prod?host=localhost",
            "postgres://app:secret@prod-db.internal/rustok_prod?host=%2Fvar%2Frun%2Fpostgresql",
            "postgres://app:secret@prod-db.internal/rustok_prod?host=db.example.com",
            "postgres://app:secret@prod-db.internal/rustok_prod?host=prod-db.internal,localhost",
        ] {
            let error = super::validate_database_deployment(uri, true)
                .expect_err("local, placeholder, or mixed host targets must fail closed");
            assert!(error.to_string().contains("local placeholders"));
        }

        super::validate_database_deployment(
            "postgres://app:secret@prod-db.internal/rustok_prod?host=other-prod.internal",
            true,
        )
        .expect("explicit remote URI and query hosts are valid production targets");

        let error = super::validate_database_deployment(
            "postgres:///rustok_prod?host=prod-db.internal",
            true,
        )
        .expect_err("a query-only host must not replace the URI authority");
        assert!(error.to_string().contains("explicit host"));
    }

    #[test]
    fn production_rejects_percent_encoded_default_database_credentials() {
        let error = super::validate_database_deployment(
            "postgres://r%75stok:r%75stok@prod-db.internal:5432/rustok_prod",
            true,
        )
        .expect_err("URL-encoded development credentials must still be rejected");
        assert!(error.to_string().contains("development credentials"));
    }

    #[test]
    fn production_rejects_percent_encoded_local_development_database_name() {
        let error = super::validate_database_deployment(
            "postgres://app_user:strong-secret@localhost:5433/rustok%5Fdev",
            true,
        )
        .expect_err("URL-encoded local development database names must be rejected");
        assert!(error.to_string().contains("development credentials"));
    }

    #[test]
    fn production_does_not_reject_safe_credentials_containing_placeholder_text() {
        super::validate_database_deployment(
            "postgres://app_user:secret-rustok:rustok-prod@prod-db.internal:5432/rustok_prod",
            true,
        )
        .expect("placeholder-like text in a different credential must not cause a false positive");
    }

    #[test]
    fn production_rejects_database_uri_fragments() {
        let error = super::validate_database_deployment(
            "postgres://app_user:strong-secret@prod-db.internal:5432/rustok_prod#fragment",
            true,
        )
        .expect_err("database URI fragments are ambiguous and must be rejected");
        assert!(error.to_string().contains("fragment"));
    }

    #[test]
    fn production_rejects_malformed_database_uri() {
        let error = super::validate_database_deployment("not-a-database-url", true)
            .expect_err("production database URI must be absolute");
        assert!(error.to_string().contains("valid absolute URL"));
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
        vars: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl EnvGuard {
        fn new(keys: &[&'static str]) -> Self {
            let guard = TEST_ENV_LOCK.lock().unwrap();
            let mut vars = Vec::new();
            for &k in keys {
                vars.push((k, std::env::var_os(k)));
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
    fn auth_environment_overrides_yaml_with_canonical_secret_precedence() {
        let _env = EnvGuard::new(&[
            "RUSTOK_JWT_SECRET",
            "JWT_SECRET",
            "JWT_EXPIRATION",
            "REFRESH_TOKEN_EXPIRATION",
            "JWT_ISSUER",
            "JWT_AUDIENCE",
        ]);
        let mut config = HostConfig {
            server: super::ServerConfig {
                binding: "127.0.0.1".to_string(),
                port: 5150,
            },
            database: database_config_for_validation(),
            auth: super::AuthConfig {
                jwt: super::JwtConfig {
                    mac_root: "yaml-secret".to_string(),
                    expiration: 900,
                },
            },
            settings: serde_json::json!({
                "auth": {
                    "refresh_expiration": 3600,
                    "issuer": "https://yaml.example",
                    "audience": "yaml-audience"
                }
            }),
        };

        unsafe {
            std::env::set_var("RUSTOK_JWT_SECRET", "canonical-secret");
            std::env::set_var("JWT_SECRET", "legacy-secret");
            std::env::set_var("JWT_EXPIRATION", "1800");
            std::env::set_var("REFRESH_TOKEN_EXPIRATION", "7200");
            std::env::set_var("JWT_ISSUER", "https://env.example");
            std::env::set_var("JWT_AUDIENCE", "env-audience");
        }
        super::apply_auth_environment_overrides(&mut config)
            .expect("valid auth environment overrides");

        assert_eq!(config.auth.jwt.mac_root, "canonical-secret");
        assert_eq!(config.auth.jwt.expiration, 1800);
        assert_eq!(
            config.settings["auth"]["refresh_expiration"],
            serde_json::json!(7200)
        );
        assert_eq!(
            config.settings["auth"]["issuer"],
            serde_json::json!("https://env.example")
        );
        assert_eq!(
            config.settings["auth"]["audience"],
            serde_json::json!("env-audience")
        );

        unsafe {
            std::env::remove_var("RUSTOK_JWT_SECRET");
        }
        let mut legacy_config = HostConfig {
            server: super::ServerConfig {
                binding: "127.0.0.1".to_string(),
                port: 5150,
            },
            database: database_config_for_validation(),
            auth: super::AuthConfig {
                jwt: super::JwtConfig {
                    mac_root: "yaml-secret".to_string(),
                    expiration: 900,
                },
            },
            settings: serde_json::Value::Null,
        };
        super::apply_auth_environment_overrides(&mut legacy_config)
            .expect("legacy JWT_SECRET compatibility alias");
        assert_eq!(legacy_config.auth.jwt.mac_root, "legacy-secret");
    }

    #[test]
    fn auth_environment_overrides_reject_invalid_expiration() {
        let _env = EnvGuard::new(&[
            "RUSTOK_JWT_SECRET",
            "JWT_SECRET",
            "JWT_EXPIRATION",
            "REFRESH_TOKEN_EXPIRATION",
            "JWT_ISSUER",
            "JWT_AUDIENCE",
        ]);
        let mut config = HostConfig {
            server: super::ServerConfig {
                binding: "127.0.0.1".to_string(),
                port: 5150,
            },
            database: database_config_for_validation(),
            auth: super::AuthConfig {
                jwt: super::JwtConfig {
                    mac_root: "yaml-secret".to_string(),
                    expiration: 900,
                },
            },
            settings: serde_json::Value::Null,
        };
        unsafe {
            std::env::set_var("JWT_EXPIRATION", "not-a-number");
        }
        let error = super::apply_auth_environment_overrides(&mut config)
            .expect_err("invalid expiration must not silently fall back to YAML");
        assert!(error.to_string().contains("JWT_EXPIRATION"));
    }

    #[test]
    fn database_environment_overrides_yaml_and_rejects_invalid_values() {
        let _env = EnvGuard::new(&[
            "DATABASE_MIN_CONNECTIONS",
            "DATABASE_MAX_CONNECTIONS",
            "DATABASE_CONNECT_TIMEOUT",
            "DATABASE_IDLE_TIMEOUT",
            "DATABASE_ENABLE_LOGGING",
            "DATABASE_AUTO_MIGRATE",
            "RUSTOK_JWT_SECRET",
            "JWT_SECRET",
            "JWT_EXPIRATION",
            "REFRESH_TOKEN_EXPIRATION",
            "JWT_ISSUER",
            "JWT_AUDIENCE",
        ]);
        let mut config = database_config_for_validation();
        config.min_connections = 1;
        config.max_connections = 4;
        config.connect_timeout = 10_000;
        config.idle_timeout = 30_000;
        config.enable_logging = true;
        config.auto_migrate = true;

        unsafe {
            std::env::set_var("DATABASE_MIN_CONNECTIONS", "2");
            std::env::set_var("DATABASE_MAX_CONNECTIONS", "8");
            std::env::set_var("DATABASE_CONNECT_TIMEOUT", "500");
            std::env::set_var("DATABASE_IDLE_TIMEOUT", "250");
            std::env::set_var("DATABASE_ENABLE_LOGGING", "false");
            std::env::set_var("DATABASE_AUTO_MIGRATE", "no");
        }
        super::apply_database_environment_overrides(&mut config)
            .expect("valid environment overrides");
        assert_eq!(config.min_connections, 2);
        assert_eq!(config.max_connections, 8);
        assert_eq!(config.connect_timeout, 500);
        assert_eq!(config.idle_timeout, 250);
        assert!(!config.enable_logging);
        assert!(!config.auto_migrate);
        super::validate_database_runtime_policy(&config, true)
            .expect("explicit false environment overrides satisfy production policy");

        unsafe {
            std::env::set_var("DATABASE_AUTO_MIGRATE", "true");
            std::env::set_var("DATABASE_ENABLE_LOGGING", "true");
        }
        super::apply_database_environment_overrides(&mut config)
            .expect("valid true database overrides");
        let production_error = super::validate_database_runtime_policy(&config, true)
            .expect_err("environment overrides must not bypass production policy");
        assert!(production_error.to_string().contains("auto_migrate"));

        unsafe {
            std::env::set_var("DATABASE_AUTO_MIGRATE", "sometimes");
        }
        let error = super::apply_database_environment_overrides(&mut config)
            .expect_err("invalid boolean must not silently fall back to YAML");
        assert!(error.to_string().contains("DATABASE_AUTO_MIGRATE"));
    }

    #[test]
    fn host_config_examples_deserialize_without_ignored_fields() {
        for (name, source) in [
            ("development", include_str!("../config/development.yaml")),
            ("test", include_str!("../config/test.yaml")),
            ("browser-smoke", include_str!("../config/browser-smoke.yaml")),
            (
                "production.redis.example",
                include_str!("../config/production.redis.example.yaml"),
            ),
        ] {
            serde_yaml::from_str::<HostConfig>(source)
                .unwrap_or_else(|error| panic!("{name} host config is invalid: {error}"));
        }
    }

    #[test]
    fn host_config_rejects_unknown_top_level_server_and_database_fields() {
        let valid = r#"
server:
  binding: 127.0.0.1
  port: 5150
database:
  uri: sqlite::memory:
auth:
  jwt:
    secret: test-secret
    expiration: 3600
"#;
        let with_logger = format!("logger:\n  level: debug\n{valid}");
        let logger_error = serde_yaml::from_str::<HostConfig>(&with_logger)
            .expect_err("unknown top-level logger settings must fail");
        assert!(logger_error.to_string().contains("logger"));

        let with_server_host = valid.replace(
            "  port: 5150",
            "  port: 5150\n  host: http://localhost",
        );
        let host_error = serde_yaml::from_str::<HostConfig>(&with_server_host)
            .expect_err("unsupported server.host must fail");
        assert!(host_error.to_string().contains("host"));

        let with_database_typo = valid.replace(
            "  uri: sqlite::memory:",
            "  uri: sqlite::memory:\n  max_connection: 8",
        );
        let database_error = serde_yaml::from_str::<HostConfig>(&with_database_typo)
            .expect_err("unknown database fields must fail");
        assert!(database_error.to_string().contains("max_connection"));
    }

    #[test]
    fn explicit_empty_binding_override_fails_closed() {
        let _env = EnvGuard::new(&["HOST", "RUSTOK_BINDING"]);
        unsafe {
            std::env::set_var("HOST", "0.0.0.0");
            std::env::set_var("RUSTOK_BINDING", "  ");
        }
        let config = super::ServerConfig {
            binding: "127.0.0.1".to_string(),
            port: 5150,
        };
        let error = super::resolve_server_endpoint(&config)
            .expect_err("an explicitly empty canonical binding must not fall back");
        assert!(error.to_string().contains("must not be empty"));
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_environment_values_fail_closed() {
        use std::os::unix::ffi::OsStringExt;

        let _env = EnvGuard::new(&["DATABASE_URL"]);
        unsafe {
            std::env::set_var("DATABASE_URL", std::ffi::OsString::from_vec(vec![0xff]));
        }
        let error = super::read_optional_env("DATABASE_URL")
            .expect_err("non-Unicode database URLs must not be silently ignored");
        assert!(error.to_string().contains("DATABASE_URL"));
        assert!(error.to_string().contains("UTF-8"));
    }

    #[test]
    fn resolve_server_endpoint_uses_config_defaults() {
        let _env = EnvGuard::new(&[
            "PORT",
            "RUSTOK_PORT",
            "HOST",
            "RUSTOK_BINDING",
            "DATABASE_MIN_CONNECTIONS",
            "DATABASE_MAX_CONNECTIONS",
            "DATABASE_CONNECT_TIMEOUT",
            "DATABASE_IDLE_TIMEOUT",
            "DATABASE_ENABLE_LOGGING",
            "DATABASE_AUTO_MIGRATE",
        ]);
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

    #[test]
    fn parse_server_port_rejects_zero() {
        let error = super::parse_server_port("PORT", "0")
            .expect_err("port zero must not select an ephemeral listener");
        assert!(error.to_string().contains("between 1 and 65535"));
    }

    #[test]
    fn resolve_server_endpoint_rejects_zero_configured_port() {
        let _env = EnvGuard::new(&["PORT", "RUSTOK_PORT", "HOST", "RUSTOK_BINDING"]);
        let config = super::ServerConfig {
            binding: "127.0.0.1".to_string(),
            port: 0,
        };
        let error = super::resolve_server_endpoint(&config)
            .expect_err("configured port zero must not select an ephemeral listener");
        assert!(error.to_string().contains("server.port"));
        assert!(error.to_string().contains("between 1 and 65535"));
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
        let config = test_auth_config("aB3!zY7@qW8#eR2$".repeat(5))
            .with_issuer("https://")
            .with_audience("rustok-production-admin");
        let error =
            validate_auth_deployment(&config, true).expect_err("issuer without host must fail");
        assert!(error.to_string().contains("issuer"));

        let config_ws = test_auth_config("aB3!zY7@qW8#eR2$".repeat(5))
            .with_issuer("https://api .example.com")
            .with_audience("rustok-production-admin");
        let error_ws = validate_auth_deployment(&config_ws, true)
            .expect_err("issuer with whitespace must fail");
        assert!(error_ws.to_string().contains("issuer"));
    }

    #[test]
    fn validate_auth_deployment_rejects_zero_expiration_and_short_password() {
        let mut config = test_auth_config("aB3!zY7@qW8#eR2$".repeat(5))
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
        let config = test_auth_config("aB3!zY7@qW8#eR2$".repeat(5))
            .with_issuer("https://?")
            .with_audience("rustok-production-admin");
        let error = validate_auth_deployment(&config, true)
            .expect_err("an HTTPS issuer without a host must fail");
        assert!(error.to_string().contains("issuer"));
    }

    #[test]
    fn production_database_runtime_policy_rejects_automatic_migrations_and_sql_logging() {
        let mut config = database_config_for_validation();
        config.auto_migrate = true;
        let migration_error = super::validate_database_runtime_policy(&config, true)
            .expect_err("production must not run automatic migrations");
        assert!(migration_error.to_string().contains("auto_migrate"));
        super::validate_database_runtime_policy(&config, false)
            .expect("development may opt into automatic migrations");

        config.auto_migrate = false;
        config.enable_logging = true;
        let logging_error = super::validate_database_runtime_policy(&config, true)
            .expect_err("production must not log SQL");
        assert!(logging_error.to_string().contains("enable_logging"));
        super::validate_database_runtime_policy(&config, false)
            .expect("development may opt into SQL logging");

        config.enable_logging = false;
        super::validate_database_runtime_policy(&config, true)
            .expect("safe production database policy");
        super::validate_database_runtime_policy(&config, false)
            .expect("development may opt into automatic migrations and SQL logging");
    }

    #[test]
    fn database_connection_timeouts_honor_positive_configured_values() {
        let mut config = database_config_for_validation();
        config.connect_timeout = 500;
        config.idle_timeout = 250;

        assert_eq!(
            super::database_connect_timeout(&config),
            std::time::Duration::from_millis(500)
        );
        assert_eq!(
            super::database_idle_timeout(&config),
            Some(std::time::Duration::from_millis(250))
        );

        config.connect_timeout = 0;
        config.idle_timeout = 0;
        assert_eq!(
            super::database_connect_timeout(&config),
            std::time::Duration::from_secs(10)
        );
        assert_eq!(super::database_idle_timeout(&config), None);
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
