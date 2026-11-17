use std::{collections::HashSet, convert::Infallible, future::Future, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use rustok_product::services::{
    CanonicalProductIndexRefreshEventFactory, ProductIndexRefreshRelayError,
    ProductIndexRefreshRelayStep, ProductIndexRefreshRelayStepOutcome,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use uuid::Uuid;

const TENANTS_ENV: &str = "RUSTOK_PRODUCT_INDEX_REFRESH_TENANTS";
const POLL_MS_ENV: &str = "RUSTOK_PRODUCT_INDEX_REFRESH_POLL_MS";
const MAX_TENANTS: usize = 128;
const CONCURRENT_TENANTS: usize = 4;
const STEPS_PER_STREAM: usize = 8;
const STREAM_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct IndexRefresh {
    tenants: Vec<Uuid>,
    interval: Duration,
}

impl IndexRefresh {
    pub(super) fn from_environment() -> Result<Option<Self>> {
        let raw = match std::env::var(TENANTS_ENV) {
            Ok(raw) => raw,
            Err(std::env::VarError::NotPresent) => {
                if std::env::var_os(POLL_MS_ENV).is_some() {
                    bail!("{POLL_MS_ENV} requires {TENANTS_ENV}");
                }
                return Ok(None);
            }
            Err(error) => return Err(error).context(TENANTS_ENV),
        };
        let mut tenants = Vec::new();
        let mut seen = HashSet::new();
        for entry in raw.split(',') {
            let tenant = Uuid::parse_str(entry.trim())
                .ok()
                .filter(|id| !id.is_nil() && seen.insert(*id))
                .filter(|_| tenants.len() < MAX_TENANTS)
                .ok_or_else(|| {
                    anyhow!("{TENANTS_ENV} requires 1 to {MAX_TENANTS} distinct non-nil UUIDs")
                })?;
            tenants.push(tenant);
        }
        let ms = super::optional_u64(POLL_MS_ENV, 1000)?;
        if !(100..=60_000).contains(&ms) {
            bail!("{POLL_MS_ENV} must be in 100..=60000");
        }
        Ok(Some(Self {
            tenants,
            interval: Duration::from_millis(ms),
        }))
    }

    pub(super) async fn verify_schema(&self, db: &DatabaseConnection) -> Result<()> {
        db.execute_unprepared(
            "SELECT tenant_id, stream_kind, last_sequence_no FROM product_index_refresh_relay_cursors LIMIT 0",
        )
        .await
        .map_err(|_| super::schema_preflight_error("product_index_refresh_relay_cursors"))?;
        let ordered = db
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Postgres,
                r#"
            SELECT count(*) = 2 AS ordered
            FROM pg_trigger
            WHERE NOT tgisinternal AND tgenabled IN ('O', 'A')
              AND tgname = 'trg_product_index_refresh_sequence'
              AND tgrelid IN (to_regclass('product_index_locale_refresh_ledger'),
                              to_regclass('product_variant_index_refresh_ledger'))
        "#,
            ))
            .await
            .map_err(|_| anyhow!("Product Index refresh ledger preflight failed"))?
            .and_then(|row| row.try_get::<bool>("", "ordered").ok())
            .unwrap_or(false);
        if !ordered {
            bail!(
                "Product Index refresh publisher requires ordered ledger inserts; apply m20261116_000043_order_product_index_refresh_ledger_commits"
            );
        }
        Ok(())
    }

    pub(super) async fn run(self, db: DatabaseConnection) -> Infallible {
        tracing::info!(
            tenants = self.tenants.len(),
            "Product Index refresh publisher started"
        );
        let mut ticks = tokio::time::interval(self.interval);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            for tenants in self.tenants.chunks(CONCURRENT_TENANTS) {
                let mut batch = tokio::task::JoinSet::new();
                for &tenant in tenants {
                    let step = ProductIndexRefreshRelayStep::new(
                        db.clone(),
                        CanonicalProductIndexRefreshEventFactory,
                    );
                    batch.spawn(async move {
                        tokio::join!(
                            drain(tenant, "locale", || step.publish_next_locale(tenant)),
                            drain(tenant, "variant", || step.publish_next_variant(tenant)),
                        );
                    });
                }
                while let Some(result) = batch.join_next().await {
                    if let Err(error) = result {
                        tracing::error!(%error, "Product Index refresh publisher task failed");
                    }
                }
            }
        }
    }
}

async fn drain<F>(tenant: Uuid, stream: &str, mut publish: impl FnMut() -> F)
where
    F: Future<Output = Result<ProductIndexRefreshRelayStepOutcome, ProductIndexRefreshRelayError>>,
{
    let batch = async {
        for _ in 0..STEPS_PER_STREAM {
            match publish().await? {
                ProductIndexRefreshRelayStepOutcome::Published { .. }
                | ProductIndexRefreshRelayStepOutcome::CursorAdvanced { .. } => {}
                ProductIndexRefreshRelayStepOutcome::Idle { .. } => break,
            }
        }
        Ok::<_, ProductIndexRefreshRelayError>(())
    };
    match tokio::time::timeout(STREAM_TIMEOUT, batch).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            tracing::warn!(tenant_id = %tenant, stream, %error, "Product Index refresh publication failed")
        }
        Err(_) => {
            tracing::warn!(tenant_id = %tenant, stream, "Product Index refresh publication timed out")
        }
    }
}
