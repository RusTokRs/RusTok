use std::{convert::Infallible, time::Duration};

use anyhow::{Context, Result, bail};
use flex_ports::grpc::{GrpcFlexAttachedSchemaProvider, parse_snapshot_tenants};
use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext};
use rustok_product::services::flex_schema_projection::{
    ProductFlexSchemaApplyOutcome, ProductFlexSchemaProjection,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

const ENDPOINT_ENV: &str = "RUSTOK_PRODUCT_FLEX_SCHEMA_GRPC_ENDPOINT";
const TOKEN_ENV: &str = "RUSTOK_FLEX_PRODUCT_SCHEMA_GRPC_BEARER_TOKEN";
const TENANTS_ENV: &str = "RUSTOK_PRODUCT_FLEX_SCHEMA_TENANTS";
const PERIOD_ENV: &str = "RUSTOK_PRODUCT_FLEX_SCHEMA_REFRESH_SECONDS";
const ALLOW_LOOPBACK_ENV: &str = "RUSTOK_PRODUCT_FLEX_SCHEMA_ALLOW_INSECURE_LOOPBACK";
const CA_CERT_ENV: &str = "RUSTOK_PRODUCT_FLEX_SCHEMA_CA_CERT_PATH";
const REFRESH_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct FlexSchemaRefresh {
    provider: GrpcFlexAttachedSchemaProvider,
    tenants: Vec<Uuid>,
    period: Duration,
}

impl FlexSchemaRefresh {
    pub(super) fn from_environment(projected: bool) -> Result<Option<Self>> {
        let endpoint = super::optional_env(ENDPOINT_ENV);
        if let Some(endpoint) = endpoint {
            if !projected {
                bail!(
                    "{ENDPOINT_ENV} requires {}=true",
                    super::FLEX_SCHEMA_PROJECTION_ENV
                );
            }
            let tenant_config = super::required_env(TENANTS_ENV)?;
            let tenants = parse_snapshot_tenants(&tenant_config).context(TENANTS_ENV)?;
            let period = super::optional_u64(PERIOD_ENV, 30)?;
            if !(1..=3600).contains(&period) {
                bail!("{PERIOD_ENV} must be in 1..=3600");
            }
            let token = super::required_secret_env(TOKEN_ENV)?;
            let ca_pem = std::env::var_os(CA_CERT_ENV)
                .map(std::fs::read)
                .transpose()
                .context(CA_CERT_ENV)?;
            let provider = GrpcFlexAttachedSchemaProvider::with_trusted_ca(
                &endpoint,
                &token,
                super::optional_bool(ALLOW_LOOPBACK_ENV, false)?,
                ca_pem.as_deref(),
            )
            .context("Flex schema gRPC configuration is invalid")?;
            Ok(Some(Self {
                provider,
                tenants,
                period: Duration::from_secs(period),
            }))
        } else {
            for setting in [TENANTS_ENV, PERIOD_ENV, ALLOW_LOOPBACK_ENV, CA_CERT_ENV] {
                if std::env::var_os(setting).is_some() {
                    bail!("{setting} requires {ENDPOINT_ENV}");
                }
            }
            Ok(None)
        }
    }

    pub(super) async fn run(self, db: DatabaseConnection) -> Infallible {
        let store = ProductFlexSchemaProjection::new(db);
        let mut ticks = tokio::time::interval(self.period);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            // Bound concurrent connections and keep at most one refresh per tenant in flight.
            for tenants in self.tenants.chunks(8) {
                let mut batch = tokio::task::JoinSet::new();
                for &tenant in tenants {
                    let provider = self.provider.clone();
                    let store = store.clone();
                    batch.spawn(async move {
                        let context = PortContext::new(tenant.to_string(), PortActor::service("product.flex-schema-sync"), PLATFORM_FALLBACK_LOCALE, Uuid::new_v4().to_string()).with_deadline(REFRESH_TIMEOUT);
                        let outcome = tokio::time::timeout(REFRESH_TIMEOUT, store.refresh(context, &provider)).await;
                        match outcome {
                            Ok(Ok(ProductFlexSchemaApplyOutcome::Advanced)) => tracing::info!(tenant_id = %tenant, "Product Flex schema projection advanced"),
                            Ok(Ok(ProductFlexSchemaApplyOutcome::Replay)) => {}
                            Ok(Ok(ProductFlexSchemaApplyOutcome::Stale)) => tracing::warn!(tenant_id = %tenant, "Flex served an older revision than the Product projection"),
                            Ok(Err(error)) => tracing::warn!(tenant_id = %tenant, code = %error.code, retryable = error.retryable, "Product Flex schema refresh failed"),
                            Err(_) => tracing::warn!(tenant_id = %tenant, "Product Flex schema refresh timed out"),
                        }
                    });
                }
                while let Some(result) = batch.join_next().await {
                    if let Err(error) = result {
                        tracing::error!(%error, "Product Flex schema refresh task failed");
                    }
                }
            }
        }
    }
}
