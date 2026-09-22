use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{
    ModuleWorkError, ModuleWorkHandler, ModuleWorkItem, ModuleWorkOutcome, ModuleWorkSource,
};
use rustok_core::ModuleRuntimeExtensions;
use rustok_product::{
    ProductSalesChannelIndexRelationConvergenceClaim,
    ProductSalesChannelIndexRelationConvergenceClaimOutcome,
    ProductSalesChannelIndexRelationConvergenceError,
    ProductSalesChannelIndexRelationConvergenceStore,
    ProductSalesChannelIndexRelationConvergenceWork,
};
use rustok_runtime::{
    HostRuntimeContext, ModuleWorkRegistration, ModuleWorkRegistrations, ModuleWorkScheduler,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::channel_relation_resolver::{
    MAX_PRODUCT_SALES_CHANNEL_RELATION_RESOLVE_PAGE, ProductSalesChannelRelationResolver,
    ProductSalesChannelRelationResolverError,
};

pub(crate) const PRODUCT_SALES_CHANNEL_RELATION_CONVERGENCE_WORKER: &str =
    "product_sales_channel_relation_convergence";
const WORK_ITEM_CONTRACT: &str = "product_sales_channel_relation_convergence_v1";
const CLAIM_FAILED_CODE: &str = "product.sales_channel_relation_convergence.claim_failed";
const INVALID_WORK_ITEM_CODE: &str = "product.sales_channel_relation_convergence.invalid_work_item";
const EXECUTION_FAILED_CODE: &str = "product.sales_channel_relation_convergence.execution_failed";
const RETRY_MARKER: &str = "product_sales_channel_relation_convergence_retryable";
const REJECTED_MARKER: &str = "product_sales_channel_relation_convergence_rejected";
const CANCELLED_MARKER: &str = "product_sales_channel_relation_convergence_cancelled";
const RETRY_DELAY: Duration = Duration::from_secs(5);
const REJECTED_RETRY_DELAY: Duration = Duration::from_secs(60);
const LEASE_DURATION: Duration = Duration::from_secs(300);

const DUE_TENANT_SQL: &str = r#"
SELECT
    state.tenant_id,
    COALESCE(channel_generation.generation, 0)::bigint AS current_channel_identity_generation
FROM product_sales_channel_index_relation_convergence_state state
LEFT JOIN channel_index_identity_generations channel_generation
  ON channel_generation.tenant_id = state.tenant_id
WHERE state.available_at <= CURRENT_TIMESTAMP
  AND (state.lease_token IS NULL OR state.lease_expires_at <= CURRENT_TIMESTAMP)
  AND (
      state.sweep_generation IS NOT NULL
      OR state.channel_identity_generation IS NULL
      OR state.channel_identity_generation < COALESCE(channel_generation.generation, 0)
      OR EXISTS (
          SELECT 1
          FROM product_sales_channel_index_relation_convergence_requests request
          WHERE request.tenant_id = state.tenant_id
            AND request.sequence_no > state.visibility_cursor
      )
  )
ORDER BY state.available_at, state.updated_at, state.tenant_id
LIMIT 1
"#;

#[derive(Clone)]
struct ProductSalesChannelRelationConvergenceRegistrationMarker;

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    if !extensions.contains::<rustok_product::ProductRuntimeSelected>()
        || !extensions.contains::<rustok_channel::ChannelRuntimeSelected>()
    {
        return Ok(());
    }
    if extensions.contains::<ProductSalesChannelRelationConvergenceRegistrationMarker>() {
        return Err(rustok_core::Error::Validation(
            "Product-SalesChannel relation convergence worker is already registered".to_owned(),
        ));
    }
    extensions
        .get_or_insert_with::<ModuleWorkRegistrations, _>(Default::default)
        .register(Arc::new(ProductSalesChannelRelationConvergenceRegistration));
    extensions.insert(ProductSalesChannelRelationConvergenceRegistrationMarker);
    Ok(())
}

#[derive(Clone)]
struct ProductSalesChannelRelationConvergenceRegistration;

#[async_trait]
impl ModuleWorkRegistration for ProductSalesChannelRelationConvergenceRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        if host.db_clone().get_database_backend() != DbBackend::Postgres {
            return Ok(());
        }
        ProductSalesChannelRelationConvergenceAdapter::new(host.db_clone())
            .register_with(scheduler)
            .await
    }
}

#[derive(Clone)]
struct ProductSalesChannelRelationConvergenceAdapter {
    db: DatabaseConnection,
    resolver: ProductSalesChannelRelationResolver,
    store: ProductSalesChannelIndexRelationConvergenceStore,
}

impl ProductSalesChannelRelationConvergenceAdapter {
    fn new(db: DatabaseConnection) -> Self {
        Self {
            resolver: ProductSalesChannelRelationResolver::new(db.clone()),
            store: ProductSalesChannelIndexRelationConvergenceStore::new(db.clone()),
            db,
        }
    }

    async fn register_with(self, scheduler: &ModuleWorkScheduler) -> Result<(), ModuleWorkError> {
        let adapter = Arc::new(self);
        scheduler.register(adapter.clone(), adapter).await
    }

    async fn discover_due_tenant(&self) -> Result<Option<(Uuid, u64)>, ModuleWorkError> {
        let row = self
            .db
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                DUE_TENANT_SQL.to_owned(),
            ))
            .await
            .map_err(|_| ModuleWorkError::Source(CLAIM_FAILED_CODE.to_owned()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let tenant_id = row
            .try_get::<Uuid>("", "tenant_id")
            .map_err(|_| ModuleWorkError::Source(CLAIM_FAILED_CODE.to_owned()))?;
        let generation = row
            .try_get::<i64>("", "current_channel_identity_generation")
            .map_err(|_| ModuleWorkError::Source(CLAIM_FAILED_CODE.to_owned()))?;
        if tenant_id.is_nil() || generation < 0 {
            return Err(ModuleWorkError::Source(CLAIM_FAILED_CODE.to_owned()));
        }
        let generation = u64::try_from(generation)
            .map_err(|_| ModuleWorkError::Source(CLAIM_FAILED_CODE.to_owned()))?;
        Ok(Some((tenant_id, generation)))
    }

    async fn reconcile_sweep_page(
        &self,
        tenant_id: Uuid,
        after_product_id: Option<Uuid>,
    ) -> Result<Option<Uuid>, ProductSalesChannelRelationResolverError> {
        self.resolver
            .reconcile_tenant_page(
                tenant_id,
                after_product_id,
                MAX_PRODUCT_SALES_CHANNEL_RELATION_RESOLVE_PAGE,
            )
            .await
    }

    fn work_item(
        claim: &ProductSalesChannelIndexRelationConvergenceClaim,
    ) -> Result<ModuleWorkItem, ModuleWorkError> {
        let payload = ConvergenceWorkPayload::from_claim(claim)?;
        Ok(ModuleWorkItem {
            id: claim.lease_token(),
            tenant_id: claim.tenant_id(),
            worker_slug: PRODUCT_SALES_CHANNEL_RELATION_CONVERGENCE_WORKER.to_owned(),
            lease_token: claim.lease_token().to_string(),
            payload: serde_json::to_value(payload)
                .map_err(|_| ModuleWorkError::Source(CLAIM_FAILED_CODE.to_owned()))?,
        })
    }

    fn decode_item(
        item: &ModuleWorkItem,
    ) -> Result<ProductSalesChannelIndexRelationConvergenceClaim, ModuleWorkError> {
        if item.worker_slug != PRODUCT_SALES_CHANNEL_RELATION_CONVERGENCE_WORKER
            || item.id.is_nil()
            || item.tenant_id.is_nil()
        {
            return Err(invalid_work_item());
        }
        let lease_token = Uuid::parse_str(&item.lease_token).map_err(|_| invalid_work_item())?;
        if lease_token.is_nil() || lease_token != item.id {
            return Err(invalid_work_item());
        }
        let payload: ConvergenceWorkPayload =
            serde_json::from_value(item.payload.clone()).map_err(|_| invalid_work_item())?;
        if payload.contract != WORK_ITEM_CONTRACT {
            return Err(invalid_work_item());
        }
        let work = payload.work.into_owner_work()?;
        ProductSalesChannelIndexRelationConvergenceClaim::restore(item.tenant_id, lease_token, work)
            .map_err(|_| invalid_work_item())
    }

    async fn release_after_outcome(
        &self,
        item: &ModuleWorkItem,
        outcome: &ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        let claim = Self::decode_item(item)?;
        let (delay, marker) = match outcome {
            ModuleWorkOutcome::Completed => return Ok(()),
            ModuleWorkOutcome::Retryable { .. } => (RETRY_DELAY, RETRY_MARKER),
            ModuleWorkOutcome::Rejected { .. } => (REJECTED_RETRY_DELAY, REJECTED_MARKER),
            ModuleWorkOutcome::Cancelled => (RETRY_DELAY, CANCELLED_MARKER),
        };
        match self.store.retry(&claim, delay, marker).await {
            Ok(()) | Err(ProductSalesChannelIndexRelationConvergenceError::LeaseLost) => Ok(()),
            Err(_) => Err(ModuleWorkError::Source(CLAIM_FAILED_CODE.to_owned())),
        }
    }

    async fn complete_visibility(
        &self,
        claim: &ProductSalesChannelIndexRelationConvergenceClaim,
    ) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        self.store
            .complete_visibility(claim)
            .await
            .map_err(|_| ModuleWorkError::Handler(EXECUTION_FAILED_CODE.to_owned()))?;
        Ok(ModuleWorkOutcome::Completed)
    }
}

#[async_trait]
impl ModuleWorkSource for ProductSalesChannelRelationConvergenceAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != PRODUCT_SALES_CHANNEL_RELATION_CONVERGENCE_WORKER {
            return Ok(None);
        }
        let Some((tenant_id, current_generation)) = self.discover_due_tenant().await? else {
            return Ok(None);
        };
        match self
            .store
            .claim(tenant_id, current_generation, LEASE_DURATION)
            .await
            .map_err(|_| ModuleWorkError::Source(CLAIM_FAILED_CODE.to_owned()))?
        {
            ProductSalesChannelIndexRelationConvergenceClaimOutcome::Idle
            | ProductSalesChannelIndexRelationConvergenceClaimOutcome::Busy => Ok(None),
            ProductSalesChannelIndexRelationConvergenceClaimOutcome::Claimed(claim) => {
                Self::work_item(&claim).map(Some)
            }
        }
    }

    async fn complete(
        &self,
        item: &ModuleWorkItem,
        outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        self.release_after_outcome(item, &outcome).await
    }
}

#[async_trait]
impl ModuleWorkHandler for ProductSalesChannelRelationConvergenceAdapter {
    fn worker_slug(&self) -> &'static str {
        PRODUCT_SALES_CHANNEL_RELATION_CONVERGENCE_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        let claim = Self::decode_item(&item)?;
        match claim.work().clone() {
            ProductSalesChannelIndexRelationConvergenceWork::VisibilityRequest {
                product_id,
                ..
            } => match self
                .resolver
                .reconcile_product(claim.tenant_id(), product_id)
                .await
            {
                Ok(_) | Err(ProductSalesChannelRelationResolverError::ProductNotFound) => {
                    self.complete_visibility(&claim).await
                }
                Err(error) if owner_rejected(&error) => {
                    // Owner rejection must not head-of-line block valid Products later in the same tenant.
                    self.complete_visibility(&claim).await
                }
                Err(error) => Ok(classify_resolver_error(error)),
            },
            ProductSalesChannelIndexRelationConvergenceWork::ChannelSweep {
                after_product_id,
                ..
            } => match self
                .reconcile_sweep_page(claim.tenant_id(), after_product_id)
                .await
            {
                Ok(next_product_id) => {
                    self.store
                        .complete_sweep_page(&claim, next_product_id)
                        .await
                        .map_err(|_| ModuleWorkError::Handler(EXECUTION_FAILED_CODE.to_owned()))?;
                    Ok(ModuleWorkOutcome::Completed)
                }
                Err(error) => Ok(classify_resolver_error(error)),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConvergenceWorkPayload {
    contract: String,
    work: ConvergencePayloadWork,
}

impl ConvergenceWorkPayload {
    fn from_claim(
        claim: &ProductSalesChannelIndexRelationConvergenceClaim,
    ) -> Result<Self, ModuleWorkError> {
        Ok(Self {
            contract: WORK_ITEM_CONTRACT.to_owned(),
            work: ConvergencePayloadWork::from_owner_work(claim.work()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ConvergencePayloadWork {
    VisibilityRequest {
        sequence_no: i64,
        product_id: Uuid,
        product_source_version: u64,
    },
    ChannelSweep {
        generation: u64,
        after_product_id: Option<Uuid>,
    },
}

impl ConvergencePayloadWork {
    fn from_owner_work(work: &ProductSalesChannelIndexRelationConvergenceWork) -> Self {
        match work {
            ProductSalesChannelIndexRelationConvergenceWork::VisibilityRequest {
                sequence_no,
                product_id,
                product_source_version,
            } => Self::VisibilityRequest {
                sequence_no: *sequence_no,
                product_id: *product_id,
                product_source_version: *product_source_version,
            },
            ProductSalesChannelIndexRelationConvergenceWork::ChannelSweep {
                generation,
                after_product_id,
            } => Self::ChannelSweep {
                generation: *generation,
                after_product_id: *after_product_id,
            },
        }
    }

    fn into_owner_work(
        self,
    ) -> Result<ProductSalesChannelIndexRelationConvergenceWork, ModuleWorkError> {
        let work = match self {
            Self::VisibilityRequest {
                sequence_no,
                product_id,
                product_source_version,
            } => ProductSalesChannelIndexRelationConvergenceWork::VisibilityRequest {
                sequence_no,
                product_id,
                product_source_version,
            },
            Self::ChannelSweep {
                generation,
                after_product_id,
            } => ProductSalesChannelIndexRelationConvergenceWork::ChannelSweep {
                generation,
                after_product_id,
            },
        };
        Ok(work)
    }
}

fn owner_rejected(error: &ProductSalesChannelRelationResolverError) -> bool {
    matches!(
        error,
        ProductSalesChannelRelationResolverError::InvalidProductVisibility
            | ProductSalesChannelRelationResolverError::TooManyVisibilitySlugs
            | ProductSalesChannelRelationResolverError::TooManyResolvedChannels
    )
}

fn classify_resolver_error(error: ProductSalesChannelRelationResolverError) -> ModuleWorkOutcome {
    match error {
        ProductSalesChannelRelationResolverError::InvalidTenant
        | ProductSalesChannelRelationResolverError::InvalidProduct
        | ProductSalesChannelRelationResolverError::InvalidCursor
        | ProductSalesChannelRelationResolverError::InvalidPage
        | ProductSalesChannelRelationResolverError::InvalidProductVisibility
        | ProductSalesChannelRelationResolverError::TooManyVisibilitySlugs
        | ProductSalesChannelRelationResolverError::TooManyResolvedChannels => {
            ModuleWorkOutcome::Rejected {
                message: REJECTED_MARKER.to_owned(),
            }
        }
        ProductSalesChannelRelationResolverError::ProductNotFound
        | ProductSalesChannelRelationResolverError::ConcurrentChange
        | ProductSalesChannelRelationResolverError::Unavailable
        | ProductSalesChannelRelationResolverError::RelationOwner(_)
        | ProductSalesChannelRelationResolverError::FreshnessOwner(_) => {
            ModuleWorkOutcome::Retryable {
                message: RETRY_MARKER.to_owned(),
            }
        }
    }
}

fn invalid_work_item() -> ModuleWorkError {
    ModuleWorkError::Handler(INVALID_WORK_ITEM_CODE.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_query_is_bounded_and_durable_state_driven() {
        for marker in [
            "product_sales_channel_index_relation_convergence_state",
            "channel_index_identity_generations",
            "product_sales_channel_index_relation_convergence_requests",
            "state.sweep_generation IS NOT NULL",
            "state.channel_identity_generation IS NULL",
            "request.sequence_no > state.visibility_cursor",
            "LIMIT 1",
        ] {
            assert!(DUE_TENANT_SQL.contains(marker), "missing {marker}");
        }
        for forbidden in ["UPDATE ", "INSERT ", "DELETE "] {
            assert!(!DUE_TENANT_SQL.contains(forbidden));
        }
    }

    #[test]
    fn convergence_worker_is_bounded() {
        assert_eq!(MAX_PRODUCT_SALES_CHANNEL_RELATION_RESOLVE_PAGE, 64);
        assert_eq!(LEASE_DURATION, Duration::from_secs(300));
        assert_eq!(RETRY_DELAY, Duration::from_secs(5));
    }

    #[test]
    fn owner_rejection_isolated_from_retryable_storage_failures() {
        assert!(owner_rejected(
            &ProductSalesChannelRelationResolverError::InvalidProductVisibility
        ));
        assert!(!owner_rejected(
            &ProductSalesChannelRelationResolverError::ConcurrentChange
        ));
        assert!(!owner_rejected(
            &ProductSalesChannelRelationResolverError::Unavailable
        ));
    }
}
