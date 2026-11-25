use std::{env, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use rustok_taxonomy::{
    TaxonomyService,
    entities::{taxonomy_term, taxonomy_term_translation},
};
use rustok_taxonomy_ports::grpc::TaxonomyTermNamesGrpcService;
use sea_orm::{ConnectOptions, Database, EntityTrait, QuerySelect};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Identity, Server, ServerTlsConfig};
use url::Url;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    rustok_telemetry::init(rustok_telemetry::TelemetryConfig {
        service_name: "rustok-taxonomy-service".into(),
        log_format: rustok_telemetry::LogFormat::Json,
        metrics: false,
        otel: None,
    })?;
    let bind: SocketAddr = env::var("RUSTOK_TAXONOMY_SERVICE_BIND")
        .unwrap_or_else(|_| "127.0.0.1:7446".into())
        .parse()
        .context("Invalid Taxonomy service bind address")?;
    let mut server = Server::builder();
    match (
        env::var_os("RUSTOK_TAXONOMY_SERVICE_TLS_CERT_PATH"),
        env::var_os("RUSTOK_TAXONOMY_SERVICE_TLS_KEY_PATH"),
    ) {
        (Some(cert), Some(key)) => {
            let cert = tokio::fs::read(cert)
                .await
                .context("Unable to read Taxonomy TLS certificate")?;
            let key = tokio::fs::read(key)
                .await
                .context("Unable to read Taxonomy TLS key")?;
            server = server
                .tls_config(ServerTlsConfig::new().identity(Identity::from_pem(cert, key)))
                .context("Invalid Taxonomy TLS configuration")?;
        }
        (None, None)
            if env::var("RUSTOK_TAXONOMY_SERVICE_ALLOW_INSECURE_LOOPBACK").as_deref()
                == Ok("true")
                && bind.ip().is_loopback() => {}
        _ => bail!("Taxonomy gRPC requires both TLS files or explicitly enabled IP loopback HTTP"),
    }

    let url = env::var("RUSTOK_TAXONOMY_DATABASE_URL")
        .context("RUSTOK_TAXONOMY_DATABASE_URL is required")?;
    if !Url::parse(&url).is_ok_and(|url| matches!(url.scheme(), "postgres" | "postgresql")) {
        bail!("Taxonomy service requires a PostgreSQL database URL");
    }
    let token = env::var("RUSTOK_TAXONOMY_TERM_NAMES_GRPC_BEARER_TOKEN")
        .context("RUSTOK_TAXONOMY_TERM_NAMES_GRPC_BEARER_TOKEN is required")?;
    let tenant_config = env::var("RUSTOK_TAXONOMY_TERM_NAMES_GRPC_TENANTS")
        .context("RUSTOK_TAXONOMY_TERM_NAMES_GRPC_TENANTS is required")?;
    let tenants = tenant_config
        .split(',')
        .take(129)
        .map(|value| Uuid::parse_str(value.trim()))
        .collect::<Result<Vec<_>, _>>()
        .context("Invalid Taxonomy tenant allowlist")?;

    let mut options = ConnectOptions::new(url);
    options
        .connect_timeout(Duration::from_secs(5))
        .max_connections(10)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .map_err(|_| anyhow!("Taxonomy PostgreSQL connection failed"))?;
    taxonomy_term::Entity::find()
        .limit(0)
        .all(&db)
        .await
        .map_err(|_| anyhow!("Taxonomy terms are unreadable. Apply the owner migrations."))?;
    taxonomy_term_translation::Entity::find()
        .limit(0)
        .all(&db)
        .await
        .map_err(|_| {
            anyhow!("Taxonomy term translations are unreadable. Apply the owner migrations.")
        })?;
    let service =
        TaxonomyTermNamesGrpcService::new(Arc::new(TaxonomyService::new(db)), &token, tenants)?
            .into_server();
    let listener = TcpListener::bind(bind)
        .await
        .context("Unable to bind Taxonomy service")?;
    tracing::info!(bind = %listener.local_addr()?, "Taxonomy term name service ready");
    server
        .add_service(service)
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener), shutdown())
        .await
        .context("Taxonomy gRPC service failed")
}

async fn shutdown() {
    #[cfg(unix)]
    match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
        Ok(mut terminate) => {
            tokio::select! {
                _ = terminate.recv() => return,
                result = tokio::signal::ctrl_c() => {
                    if let Err(error) = result {
                        tracing::error!(%error, "Taxonomy shutdown signal unavailable");
                    }
                    return;
                }
            }
        }
        Err(error) => tracing::error!(%error, "Taxonomy SIGTERM handler unavailable"),
    }
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "Taxonomy shutdown signal unavailable");
    }
}
