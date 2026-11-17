/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::{env, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use rustok_channel::PostgresChannelIndexIdentity;
use rustok_channel_ports::grpc::ChannelIndexIdentityGrpcService;
use sea_orm::{ConnectOptions, ConnectionTrait, Database};
use tonic::transport::{Identity, Server, ServerTlsConfig};
use url::Url;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    let bind: SocketAddr = env::var("RUSTOK_CHANNEL_SERVICE_BIND")
        .unwrap_or_else(|_| "127.0.0.1:7445".into())
        .parse()
        .context("Invalid Channel service bind address")?;
    let mut server = Server::builder();
    match (
        env::var_os("RUSTOK_CHANNEL_SERVICE_TLS_CERT_PATH"),
        env::var_os("RUSTOK_CHANNEL_SERVICE_TLS_KEY_PATH"),
    ) {
        (Some(cert), Some(key)) => {
            let cert = tokio::fs::read(cert)
                .await
                .context("Unable to read Channel TLS certificate")?;
            let key = tokio::fs::read(key)
                .await
                .context("Unable to read Channel TLS key")?;
            server = server
                .tls_config(ServerTlsConfig::new().identity(Identity::from_pem(cert, key)))
                .context("Invalid Channel TLS configuration")?;
        }
        (None, None)
            if bind.ip().is_loopback()
                && env::var("RUSTOK_CHANNEL_SERVICE_ALLOW_INSECURE_LOOPBACK").as_deref()
                    == Ok("true") => {}
        _ => bail!("Channel gRPC requires both TLS files or explicitly enabled loopback HTTP"),
    }
    let database_url = env::var("RUSTOK_CHANNEL_DATABASE_URL")
        .context("RUSTOK_CHANNEL_DATABASE_URL is required")?;
    if !Url::parse(&database_url).is_ok_and(|url| matches!(url.scheme(), "postgres" | "postgresql"))
    {
        bail!("Channel service requires a PostgreSQL database URL");
    }
    let mut options = ConnectOptions::new(database_url);
    options
        .connect_timeout(Duration::from_secs(5))
        .max_connections(10)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .map_err(|_| anyhow!("Channel PostgreSQL connection failed"))?;
    db.execute_unprepared(
        "SELECT tenant_id, generation FROM channel_index_identity_generations LIMIT 0;
         SELECT tenant_id, id, slug FROM channels LIMIT 0",
    )
    .await
    .context("Channel identity schema is unavailable. Apply the Channel migrations.")?;
    let secret = env::var("RUSTOK_CHANNEL_INDEX_IDENTITY_GRPC_BEARER_TOKEN")
        .context("RUSTOK_CHANNEL_INDEX_IDENTITY_GRPC_BEARER_TOKEN is required")?;
    let tenant_list = env::var("RUSTOK_CHANNEL_INDEX_IDENTITY_TENANTS")
        .context("RUSTOK_CHANNEL_INDEX_IDENTITY_TENANTS is required")?;
    let tenants = tenant_list
        .split(',')
        .map(|value| value.trim().parse::<Uuid>())
        .collect::<Result<Vec<_>, _>>()
        .context("Invalid Channel identity tenant list")?;
    let service = ChannelIndexIdentityGrpcService::new(
        Arc::new(PostgresChannelIndexIdentity::new(db)),
        &secret,
        tenants,
    )?
    .into_server();
    server
        .add_service(service)
        .serve_with_shutdown(bind, async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                tracing::warn!(%error, "Channel shutdown signal unavailable");
                std::future::pending::<()>().await;
            }
        })
        .await
        .context("Channel gRPC service failed")
}
