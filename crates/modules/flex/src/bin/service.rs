use std::{env, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};
use flex::FlexProductSchemaReadService;
use flex_ports::grpc::{FlexAttachedSchemaGrpcService, parse_snapshot_tenants};
use sea_orm::{ConnectOptions, Database};
use tonic::transport::{Identity, Server, ServerTlsConfig};
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    let bind: SocketAddr = env::var("RUSTOK_FLEX_SERVICE_BIND")
        .unwrap_or_else(|_| "127.0.0.1:7444".into())
        .parse()
        .context("Invalid Flex service bind address")?;
    let cert_path = env::var_os("RUSTOK_FLEX_SERVICE_TLS_CERT_PATH");
    let key_path = env::var_os("RUSTOK_FLEX_SERVICE_TLS_KEY_PATH");
    let insecure = env::var("RUSTOK_FLEX_SERVICE_ALLOW_INSECURE_LOOPBACK").as_deref() == Ok("true");
    let mut server = Server::builder();
    match (cert_path, key_path) {
        (Some(cert), Some(key)) => {
            let cert = tokio::fs::read(cert)
                .await
                .context("Unable to read Flex TLS certificate")?;
            let key = tokio::fs::read(key)
                .await
                .context("Unable to read Flex TLS key")?;
            server = server
                .tls_config(ServerTlsConfig::new().identity(Identity::from_pem(cert, key)))
                .context("Invalid Flex TLS configuration")?;
        }
        (None, None) if insecure && bind.ip().is_loopback() => {}
        _ => bail!("Flex gRPC requires both TLS files or explicitly enabled loopback HTTP"),
    }

    let url =
        env::var("RUSTOK_FLEX_DATABASE_URL").context("RUSTOK_FLEX_DATABASE_URL is required")?;
    if !Url::parse(&url).is_ok_and(|url| matches!(url.scheme(), "postgres" | "postgresql")) {
        bail!("Flex service requires a PostgreSQL database URL");
    }
    let tenant_config = env::var("RUSTOK_FLEX_PRODUCT_SCHEMA_TENANTS")
        .context("RUSTOK_FLEX_PRODUCT_SCHEMA_TENANTS is required")?;
    let tenants = parse_snapshot_tenants(&tenant_config)?;
    let token = env::var("RUSTOK_FLEX_PRODUCT_SCHEMA_GRPC_BEARER_TOKEN")
        .context("RUSTOK_FLEX_PRODUCT_SCHEMA_GRPC_BEARER_TOKEN is required")?;
    let mut options = ConnectOptions::new(url);
    options
        .connect_timeout(Duration::from_secs(5))
        .max_connections(10)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .map_err(|_| anyhow::anyhow!("Flex PostgreSQL connection failed"))?;
    let owner = Arc::new(FlexProductSchemaReadService::new(db));
    owner.verify_storage().await.context("Flex Product definitions or schema revisions are unavailable. Apply the platform migrations.")?;
    let service = FlexAttachedSchemaGrpcService::new(owner, &token, tenants)?.into_server();
    server
        .add_service(service)
        .serve_with_shutdown(bind, async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                eprintln!("Flex shutdown signal unavailable: {error}");
            }
        })
        .await
        .context("Flex gRPC service failed")
}
