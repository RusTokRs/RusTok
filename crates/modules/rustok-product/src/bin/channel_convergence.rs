/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::{collections::HashSet, convert::Infallible, sync::Arc, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_channel_ports::{
    CHANNEL_INDEX_IDENTITY_READ_CLAIM, ChannelIndexIdentityReadPort, ChannelIndexIdentitySelector,
    ChannelIndexIdentitySnapshot, grpc::GrpcChannelIndexIdentity,
};
use rustok_product::{
    ProductSalesChannelIndexRelationConvergenceClaimOutcome,
    ProductSalesChannelIndexRelationConvergenceError,
    ProductSalesChannelIndexRelationConvergenceStore, ProductSalesChannelIndexRelationObservation,
    services::{
        ProductSalesChannelObservationSource, ProductSalesChannelRelationResolver,
        ProductSalesChannelRelationResolverError, bind_channel_observation,
        channel_identity_selector, load_product_visibility,
    },
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use tonic::transport::{Certificate, ClientTlsConfig};
use uuid::Uuid;

const ENDPOINT_ENV: &str = "RUSTOK_PRODUCT_CHANNEL_INDEX_GRPC_ENDPOINT";
const TENANTS_ENV: &str = "RUSTOK_PRODUCT_CHANNEL_INDEX_TENANTS";
const TOKEN_ENV: &str = "RUSTOK_CHANNEL_INDEX_IDENTITY_GRPC_BEARER_TOKEN";
const POLL_ENV: &str = "RUSTOK_PRODUCT_CHANNEL_INDEX_POLL_MS";
const CA_ENV: &str = "RUSTOK_PRODUCT_CHANNEL_INDEX_CA_CERT_PATH";
const LOOPBACK_ENV: &str = "RUSTOK_PRODUCT_CHANNEL_INDEX_ALLOW_INSECURE_LOOPBACK";
const PAGE_SIZE: usize = 8;
const CONCURRENT_TENANTS: usize = 4;
const RPC_TIMEOUT: Duration = Duration::from_secs(3);
const STORE_TIMEOUT: Duration = Duration::from_secs(5);
const LEASE: Duration = Duration::from_secs(300);
const WORK_TIMEOUT: Duration = Duration::from_secs(180);
const RETRY_DELAY: Duration = Duration::from_secs(5);

pub(super) struct ChannelConvergence {
    provider: GrpcChannelIndexIdentity,
    tenants: Vec<Uuid>,
    poll: Duration,
}

impl ChannelConvergence {
    pub(super) fn identity_port(&self) -> Arc<dyn ChannelIndexIdentityReadPort> {
        Arc::new(self.provider.clone())
    }

    pub(super) fn from_environment() -> Result<Option<Self>> {
        let endpoint = match std::env::var(ENDPOINT_ENV) {
            Ok(endpoint) => endpoint,
            Err(std::env::VarError::NotPresent) => {
                for setting in [TENANTS_ENV, POLL_ENV, CA_ENV, LOOPBACK_ENV] {
                    if std::env::var_os(setting).is_some() {
                        bail!("{setting} requires {ENDPOINT_ENV}");
                    }
                }
                return Ok(None);
            }
            Err(error) => return Err(error).context(ENDPOINT_ENV),
        };
        let mut tenants = Vec::new();
        let mut seen = HashSet::new();
        for entry in super::required_env(TENANTS_ENV)?.split(',') {
            let tenant = Uuid::parse_str(entry.trim())
                .ok()
                .filter(|tenant| !tenant.is_nil() && seen.insert(*tenant) && tenants.len() < 128)
                .ok_or_else(|| anyhow!("{TENANTS_ENV} requires 1 to 128 distinct tenant UUIDs"))?;
            tenants.push(tenant);
        }
        let poll_ms = super::optional_u64(POLL_ENV, 1000)?;
        if !(100..=60_000).contains(&poll_ms) {
            bail!("{POLL_ENV} must be in 100..=60000");
        }
        let ca = std::env::var_os(CA_ENV)
            .map(std::fs::read)
            .transpose()
            .context(CA_ENV)?;
        let tls = ca.map(|pem| ClientTlsConfig::new().ca_certificate(Certificate::from_pem(pem)));
        let provider = GrpcChannelIndexIdentity::connect_lazy(
            endpoint,
            &super::required_secret_env(TOKEN_ENV)?,
            tls,
            super::optional_bool(LOOPBACK_ENV, false)?,
        )
        .context("Product Channel identity provider configuration failed")?;
        Ok(Some(Self {
            provider,
            tenants,
            poll: Duration::from_millis(poll_ms),
        }))
    }

    pub(super) async fn verify_schema(&self, db: &DatabaseConnection) -> Result<()> {
        db.execute_unprepared(
            "SELECT tenant_id, visibility_cursor, sweep_generation, sweep_required, lease_token, available_at
                FROM product_sales_channel_index_relation_convergence_state LIMIT 0;
             SELECT sequence_no, product_source_version
                FROM product_sales_channel_index_relation_convergence_requests LIMIT 0;
             SELECT relation_epoch, channel_ids
                FROM product_sales_channel_index_relation_snapshots LIMIT 0;
             SELECT relation_epoch, channel_identity_generation
                FROM product_sales_channel_index_relation_freshness_snapshots LIMIT 0;
             SELECT projection_epoch, product_source_version
                FROM product_index_graph_projection_snapshots LIMIT 0;
             SELECT index_revision FROM products LIMIT 0",
        )
        .await
        .map_err(|_| super::schema_preflight_error("Product Channel convergence"))?;
        let ordered = db.query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT EXISTS (
                SELECT 1 FROM pg_trigger WHERE NOT tgisinternal AND tgenabled IN ('O', 'A')
                  AND tgname = 'trg_product_channel_convergence_sequence'
                  AND tgrelid = to_regclass('product_sales_channel_index_relation_convergence_requests')
            ) AS ordered",
        )).await.map_err(|_| super::schema_preflight_error("Product Channel convergence ordering"))?;
        if !ordered
            .and_then(|row| row.try_get::<bool>("", "ordered").ok())
            .unwrap_or(false)
        {
            bail!(
                "Product Channel convergence requires m20261117_000044_order_product_channel_convergence_commits"
            );
        }
        Ok(())
    }

    pub(super) async fn run(self, db: DatabaseConnection) -> Infallible {
        let observer = Arc::new(RemoteChannelObservation {
            db: db.clone(),
            provider: self.provider,
        });
        let resolver = ProductSalesChannelRelationResolver::new(db.clone(), observer.clone());
        let store = ProductSalesChannelIndexRelationConvergenceStore::new(db);
        tracing::info!(
            tenants = self.tenants.len(),
            "Product Channel convergence worker started"
        );
        let mut ticks = tokio::time::interval(self.poll);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            for tenants in self.tenants.chunks(CONCURRENT_TENANTS) {
                let mut batch = tokio::task::JoinSet::new();
                for &tenant in tenants {
                    let (observer, resolver, store) =
                        (observer.clone(), resolver.clone(), store.clone());
                    batch.spawn(async move {
                        if let Err(error) = run_tenant(&observer, &resolver, &store, tenant).await {
                            tracing::warn!(tenant_id = %tenant, %error, "Product Channel convergence failed");
                        }
                    });
                }
                while let Some(result) = batch.join_next().await {
                    if let Err(error) = result {
                        tracing::error!(%error, "Product Channel convergence task failed");
                    }
                }
            }
        }
    }
}

async fn run_tenant(
    observer: &RemoteChannelObservation,
    resolver: &ProductSalesChannelRelationResolver,
    store: &ProductSalesChannelIndexRelationConvergenceStore,
    tenant: Uuid,
) -> Result<()> {
    let snapshot = observer
        .read_channel(tenant, ChannelIndexIdentitySelector::GenerationOnly)
        .await?;
    let outcome = tokio::time::timeout(
        STORE_TIMEOUT,
        store.claim(tenant, snapshot.generation, LEASE),
    )
    .await
    .context("Product Channel convergence claim timed out")??;
    let ProductSalesChannelIndexRelationConvergenceClaimOutcome::Claimed(claim) = outcome else {
        return Ok(());
    };
    let (marker, error) = match tokio::time::timeout(
        WORK_TIMEOUT,
        resolver.execute_claim(store, &claim, PAGE_SIZE),
    )
    .await
    {
        Ok(Ok(())) => return Ok(()),
        Ok(Err(error)) => (
            "product_sales_channel_relation_convergence_retryable",
            anyhow!(error),
        ),
        Err(error) => (
            "product_sales_channel_relation_convergence_timeout",
            anyhow!(error),
        ),
    };
    match tokio::time::timeout(STORE_TIMEOUT, store.retry(&claim, RETRY_DELAY, marker)).await {
        Ok(Ok(())) | Ok(Err(ProductSalesChannelIndexRelationConvergenceError::LeaseLost)) => {}
        Ok(Err(retry_error)) => {
            tracing::warn!(%retry_error, tenant_id = %tenant, "Product could not release the convergence lease")
        }
        Err(_) => {
            tracing::warn!(tenant_id = %tenant, "Product convergence lease release timed out")
        }
    }
    Err(error)
}

struct RemoteChannelObservation {
    db: DatabaseConnection,
    provider: GrpcChannelIndexIdentity,
}

impl RemoteChannelObservation {
    async fn read_channel(
        &self,
        tenant_id: Uuid,
        selector: ChannelIndexIdentitySelector,
    ) -> Result<ChannelIndexIdentitySnapshot, PortError> {
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::service("product.channel-convergence"),
            "und",
            Uuid::new_v4().to_string(),
        )
        .with_claim(CHANNEL_INDEX_IDENTITY_READ_CLAIM)
        .with_deadline(RPC_TIMEOUT);
        tokio::time::timeout(
            RPC_TIMEOUT,
            self.provider.read_index_identity(context, selector),
        )
        .await
        .map_err(|_| {
            PortError::timeout(
                "channel.index_identity_timeout",
                "Channel identity read timed out",
            )
        })?
    }
}

#[async_trait]
impl ProductSalesChannelObservationSource for RemoteChannelObservation {
    async fn observe(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> Result<ProductSalesChannelIndexRelationObservation, ProductSalesChannelRelationResolverError>
    {
        // Release the local read before calling Channel. The owner writer compares the revision.
        let (visibility, revision) =
            load_product_visibility(&self.db, tenant_id, product_id).await?;
        let snapshot = self
            .read_channel(tenant_id, channel_identity_selector(&visibility))
            .await;
        bind_channel_observation(tenant_id, revision, visibility, snapshot)
    }
}
