use std::{
    env, fmt,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use rustok_api::PortActor;
use rustok_outbox::{OutboxTransport, SysEvents, TransactionalEventBus};
use rustok_product::{
    CatalogService,
    entities::{Product, ProductVariant},
};
use rustok_product_transport::{
    ProductCatalogGrpcBearerInterceptor, ProductCatalogGrpcService,
    proto::product_catalog_read_service_server::ProductCatalogReadServiceServer,
};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, EntityTrait};
use tonic::transport::{Identity, Server, ServerTlsConfig};
use url::Url;

const DEFAULT_BIND: &str = "127.0.0.1:7443";
const DEFAULT_DATABASE_CONNECT_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_DATABASE_MAX_CONNECTIONS: u32 = 20;
const MAX_DATABASE_CONNECT_TIMEOUT_MS: u64 = 30_000;
const MAX_DATABASE_CONNECTIONS: u32 = 200;
const REQUIRED_SCHEMA_TABLES: [&str; 3] = ["products", "product_variants", "sys_events"];

const BIND_ENV: &str = "RUSTOK_PRODUCT_CATALOG_SERVICE_BIND";
const DATABASE_URL_ENV: &str = "RUSTOK_PRODUCT_CATALOG_DATABASE_URL";
const DATABASE_CONNECT_TIMEOUT_MS_ENV: &str = "RUSTOK_PRODUCT_CATALOG_DATABASE_CONNECT_TIMEOUT_MS";
const DATABASE_MAX_CONNECTIONS_ENV: &str = "RUSTOK_PRODUCT_CATALOG_DATABASE_MAX_CONNECTIONS";
const BEARER_TOKEN_ENV: &str = "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN";
const TRUSTED_SERVICE_ACTOR_ENV: &str = "RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR";
const TLS_CERT_PATH_ENV: &str = "RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_CERT_PATH";
const TLS_KEY_PATH_ENV: &str = "RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_KEY_PATH";
const ALLOW_INSECURE_LOOPBACK_ENV: &str = "RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK";

#[derive(Clone, Eq, PartialEq)]
struct RedactedSecret(String);

impl RedactedSecret {
    fn new(value: String) -> Self {
        Self(value)
    }

    fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for RedactedSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ServiceTransport {
    Tls {
        cert_path: PathBuf,
        key_path: PathBuf,
    },
    InsecureLoopback,
}

impl ServiceTransport {
    fn label(&self) -> &'static str {
        match self {
            Self::Tls { .. } => "tls",
            Self::InsecureLoopback => "insecure_loopback",
        }
    }
}

#[derive(Clone, Debug)]
struct ServiceConfig {
    bind: SocketAddr,
    database_url: RedactedSecret,
    database_target: String,
    database_connect_timeout: Duration,
    database_max_connections: u32,
    bearer_token: RedactedSecret,
    trusted_service_actor: String,
    transport: ServiceTransport,
}

impl ServiceConfig {
    fn from_environment() -> Result<Self> {
        let bind = optional_env(BIND_ENV)
            .unwrap_or_else(|| DEFAULT_BIND.to_string())
            .parse::<SocketAddr>()
            .with_context(|| format!("{BIND_ENV} must be an IP socket address"))?;

        let database_url =
            first_required_env(&[DATABASE_URL_ENV, "RUSTOK_DATABASE_URL", "DATABASE_URL"])?;
        let (database_url, database_target) = validate_database_url(database_url)?;

        let database_connect_timeout_ms = optional_u64(
            DATABASE_CONNECT_TIMEOUT_MS_ENV,
            DEFAULT_DATABASE_CONNECT_TIMEOUT_MS,
        )?;
        if !(1..=MAX_DATABASE_CONNECT_TIMEOUT_MS).contains(&database_connect_timeout_ms) {
            bail!(
                "{DATABASE_CONNECT_TIMEOUT_MS_ENV} must be between 1 and {MAX_DATABASE_CONNECT_TIMEOUT_MS}"
            );
        }

        let database_max_connections = optional_u32(
            DATABASE_MAX_CONNECTIONS_ENV,
            DEFAULT_DATABASE_MAX_CONNECTIONS,
        )?;
        if !(1..=MAX_DATABASE_CONNECTIONS).contains(&database_max_connections) {
            bail!(
                "{DATABASE_MAX_CONNECTIONS_ENV} must be between 1 and {MAX_DATABASE_CONNECTIONS}"
            );
        }

        let bearer_token = RedactedSecret::new(required_secret_env(BEARER_TOKEN_ENV)?);
        let trusted_service_actor =
            validate_service_actor(required_env(TRUSTED_SERVICE_ACTOR_ENV)?)?;
        let transport = validate_transport_security(
            bind,
            optional_path_env(TLS_CERT_PATH_ENV),
            optional_path_env(TLS_KEY_PATH_ENV),
            optional_bool(ALLOW_INSECURE_LOOPBACK_ENV, false)?,
        )?;

        Ok(Self {
            bind,
            database_url,
            database_target,
            database_connect_timeout: Duration::from_millis(database_connect_timeout_ms),
            database_max_connections,
            bearer_token,
            trusted_service_actor,
            transport,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let telemetry_config = telemetry_config();
    let has_otel = telemetry_config.otel.is_some();
    let _telemetry = rustok_telemetry::init(telemetry_config)?;

    let result = run().await;
    if has_otel {
        rustok_telemetry::otel::shutdown().await;
    }
    result
}

async fn run() -> Result<()> {
    let config = ServiceConfig::from_environment()?;
    tracing::info!(
        bind = %config.bind,
        database_target = %config.database_target,
        transport = config.transport.label(),
        trusted_service_actor = %config.trusted_service_actor,
        "Product catalog service configuration validated"
    );

    let database = connect_database(&config).await?;
    verify_required_schema(&database).await?;

    let outbox = Arc::new(OutboxTransport::new(database.clone()));
    let event_bus = TransactionalEventBus::new(outbox);
    let provider = Arc::new(CatalogService::new(database, event_bus));
    let service = ProductCatalogGrpcService::new(provider);
    let interceptor = ProductCatalogGrpcBearerInterceptor::from_bearer_token(
        config.bearer_token.expose(),
        PortActor::service(config.trusted_service_actor.clone()),
    )
    .map_err(|error| anyhow!("Product catalog service authentication is invalid: {error}"))?;
    let service = ProductCatalogReadServiceServer::with_interceptor(service, interceptor);

    let mut server = Server::builder();
    if let ServiceTransport::Tls {
        cert_path,
        key_path,
    } = &config.transport
    {
        let identity = load_tls_identity(cert_path, key_path).await?;
        server = server
            .tls_config(ServerTlsConfig::new().identity(identity))
            .context("Product catalog service TLS configuration failed")?;
    }

    tracing::info!(
        bind = %config.bind,
        transport = config.transport.label(),
        "Product catalog gRPC service listening"
    );
    server
        .add_service(service)
        .serve_with_shutdown(config.bind, shutdown_signal())
        .await
        .context("Product catalog gRPC service failed")
}

async fn connect_database(config: &ServiceConfig) -> Result<DatabaseConnection> {
    let mut options = ConnectOptions::new(config.database_url.expose().to_string());
    options.sqlx_logging(false);
    options.connect_timeout(config.database_connect_timeout);
    options.min_connections(1);
    options.max_connections(config.database_max_connections);

    Database::connect(options)
        .await
        .map_err(|_| anyhow!("Product catalog PostgreSQL connection failed"))
}

async fn verify_required_schema(database: &DatabaseConnection) -> Result<()> {
    Product::find()
        .one(database)
        .await
        .map_err(|_| schema_preflight_error("products"))?;
    ProductVariant::find()
        .one(database)
        .await
        .map_err(|_| schema_preflight_error("product_variants"))?;
    SysEvents::find()
        .one(database)
        .await
        .map_err(|_| schema_preflight_error("sys_events"))?;

    tracing::info!(
        required_tables = ?REQUIRED_SCHEMA_TABLES,
        "Product catalog database schema preflight passed"
    );
    Ok(())
}

fn schema_preflight_error(table: &'static str) -> anyhow::Error {
    anyhow!(
        "Product catalog schema preflight failed for `{table}`; run platform migrations before starting the service"
    )
}

async fn load_tls_identity(cert_path: &Path, key_path: &Path) -> Result<Identity> {
    let certificate = tokio::fs::read(cert_path).await.with_context(|| {
        format!(
            "failed to read Product catalog TLS certificate at {}",
            cert_path.display()
        )
    })?;
    let private_key = tokio::fs::read(key_path).await.with_context(|| {
        format!(
            "failed to read Product catalog TLS private key at {}",
            key_path.display()
        )
    })?;
    Ok(Identity::from_pem(certificate, private_key))
}

fn validate_database_url(value: String) -> Result<(RedactedSecret, String)> {
    if value.is_empty() || value != value.trim() {
        bail!("Product catalog database URL must be non-empty and have no surrounding whitespace");
    }
    let parsed = Url::parse(&value).context("Product catalog database URL is invalid")?;
    if !matches!(parsed.scheme(), "postgres" | "postgresql") {
        bail!("Product catalog service requires a PostgreSQL database URL");
    }
    if parsed.fragment().is_some() {
        bail!("Product catalog database URL must not contain a fragment");
    }
    let host = parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .context("Product catalog PostgreSQL URL must contain a host")?;
    let database = parsed.path().trim_matches('/');
    if database.is_empty() {
        bail!("Product catalog PostgreSQL URL must contain a database name");
    }
    let target = format!("{}:{}/{}", host, parsed.port().unwrap_or(5432), database);
    Ok((RedactedSecret::new(value), target))
}

fn validate_service_actor(value: String) -> Result<String> {
    if value.is_empty()
        || value.len() > 128
        || value != value.trim()
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        bail!("{TRUSTED_SERVICE_ACTOR_ENV} must be 1..=128 visible non-whitespace ASCII bytes");
    }
    Ok(value)
}

fn validate_transport_security(
    bind: SocketAddr,
    cert_path: Option<PathBuf>,
    key_path: Option<PathBuf>,
    allow_insecure_loopback: bool,
) -> Result<ServiceTransport> {
    match (cert_path, key_path) {
        (Some(cert_path), Some(key_path)) => Ok(ServiceTransport::Tls {
            cert_path,
            key_path,
        }),
        (Some(_), None) | (None, Some(_)) => {
            bail!("{TLS_CERT_PATH_ENV} and {TLS_KEY_PATH_ENV} must be configured together")
        }
        (None, None) if allow_insecure_loopback && bind.ip().is_loopback() => {
            Ok(ServiceTransport::InsecureLoopback)
        }
        (None, None) if allow_insecure_loopback => {
            bail!("{ALLOW_INSECURE_LOOPBACK_ENV} is valid only for a loopback bind address")
        }
        (None, None) => bail!(
            "Product catalog service TLS is required unless explicit loopback plaintext is enabled"
        ),
    }
}

fn required_env(name: &str) -> Result<String> {
    optional_env(name).ok_or_else(|| anyhow!("{name} must be configured"))
}

fn required_secret_env(name: &str) -> Result<String> {
    env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("{name} must be configured"))
}

fn first_required_env(names: &[&str]) -> Result<String> {
    names
        .iter()
        .find_map(|name| optional_env(name))
        .ok_or_else(|| anyhow!("{} must be configured", names.join(" or ")))
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn optional_path_env(name: &str) -> Option<PathBuf> {
    optional_env(name).map(PathBuf::from)
}

fn optional_u64(name: &str, default: u64) -> Result<u64> {
    optional_env(name).map_or(Ok(default), |value| {
        value
            .parse::<u64>()
            .with_context(|| format!("{name} must be an unsigned integer"))
    })
}

fn optional_u32(name: &str, default: u32) -> Result<u32> {
    optional_env(name).map_or(Ok(default), |value| {
        value
            .parse::<u32>()
            .with_context(|| format!("{name} must be an unsigned integer"))
    })
}

fn optional_bool(name: &str, default: bool) -> Result<bool> {
    let Some(value) = optional_env(name) else {
        return Ok(default);
    };
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => bail!("{name} must be a boolean"),
    }
}

fn telemetry_config() -> rustok_telemetry::TelemetryConfig {
    let log_format = match env::var("RUSTOK_LOG_FORMAT").as_deref() {
        Ok("json") => rustok_telemetry::LogFormat::Json,
        _ => rustok_telemetry::LogFormat::Pretty,
    };
    let metrics = env::var("RUSTOK_METRICS")
        .map(|value| value != "0" && !value.eq_ignore_ascii_case("false"))
        .unwrap_or(false);
    let otel = env::var("OTEL_ENABLED")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
        .then(rustok_telemetry::otel::OtelConfig::from_env);

    rustok_telemetry::TelemetryConfig {
        service_name: "rustok-product-catalog-service".to_string(),
        log_format,
        metrics,
        otel,
    }
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let terminate = async {
        match signal(SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::warn!(error = %error, "failed to register Product catalog SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            if let Err(error) = result {
                tracing::warn!(error = %error, "failed to receive Product catalog Ctrl-C signal");
            }
        }
        _ = terminate => {}
    }
    tracing::info!("Product catalog gRPC service shutdown requested");
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!(error = %error, "failed to receive Product catalog shutdown signal");
    }
    tracing::info!("Product catalog gRPC service shutdown requested");
}

#[cfg(test)]
mod tests {
    use super::{
        REQUIRED_SCHEMA_TABLES, RedactedSecret, ServiceTransport, validate_database_url,
        validate_service_actor, validate_transport_security,
    };
    use std::{net::SocketAddr, path::PathBuf};

    #[test]
    fn secrets_are_redacted_from_debug_output() {
        let secret = RedactedSecret::new("catalog-secret".to_string());
        assert_eq!(format!("{secret:?}"), "[REDACTED]");
        assert!(!format!("{secret:?}").contains("catalog-secret"));
    }

    #[test]
    fn database_must_be_postgresql_and_debug_target_excludes_credentials() {
        let (secret, target) = validate_database_url(
            "postgres://catalog_user:catalog_password@db.internal:5433/rustok".to_string(),
        )
        .expect("valid PostgreSQL URL should be accepted");
        assert_eq!(target, "db.internal:5433/rustok");
        assert!(!target.contains("catalog_user"));
        assert!(!target.contains("catalog_password"));
        assert_eq!(format!("{secret:?}"), "[REDACTED]");
        assert!(validate_database_url("sqlite::memory:".to_string()).is_err());
    }

    #[test]
    fn required_schema_tables_are_owner_and_outbox_tables() {
        assert_eq!(
            REQUIRED_SCHEMA_TABLES,
            ["products", "product_variants", "sys_events"]
        );
    }

    #[test]
    fn plaintext_requires_explicit_loopback_bind() {
        let loopback: SocketAddr = "127.0.0.1:7443".parse().unwrap();
        assert_eq!(
            validate_transport_security(loopback, None, None, true).unwrap(),
            ServiceTransport::InsecureLoopback
        );

        let public: SocketAddr = "0.0.0.0:7443".parse().unwrap();
        assert!(validate_transport_security(public, None, None, true).is_err());
        assert!(validate_transport_security(loopback, None, None, false).is_err());
    }

    #[test]
    fn tls_requires_certificate_and_key_as_a_pair() {
        let bind: SocketAddr = "0.0.0.0:7443".parse().unwrap();
        let cert = PathBuf::from("server.crt");
        let key = PathBuf::from("server.key");
        assert_eq!(
            validate_transport_security(bind, Some(cert.clone()), Some(key.clone()), false)
                .unwrap(),
            ServiceTransport::Tls {
                cert_path: cert,
                key_path: key,
            }
        );
        assert!(
            validate_transport_security(bind, Some(PathBuf::from("server.crt")), None, false,)
                .is_err()
        );
    }

    #[test]
    fn trusted_service_actor_is_server_configured_and_bounded() {
        assert_eq!(
            validate_service_actor("rustok-server".to_string()).unwrap(),
            "rustok-server"
        );
        assert!(validate_service_actor(" caller".to_string()).is_err());
        assert!(validate_service_actor("caller service".to_string()).is_err());
    }
}
