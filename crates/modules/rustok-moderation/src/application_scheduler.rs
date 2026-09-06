use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{
    ModuleWorkError, ModuleWorkHandler, ModuleWorkItem, ModuleWorkOutcome, ModuleWorkSource,
};
use rustok_moderation_api::ModerationSubjectAdapterRegistry;
use rustok_runtime::{HostRuntimeContext, ModuleWorkRegistration, ModuleWorkScheduler};
use sea_orm::{ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::domain::ModerationApplicationOperationStatus;
use crate::entities::moderation_application_operation;
use crate::service::ModerationService;

pub(crate) const MODERATION_APPLICATION_WORKER: &str = "moderation_decision_application";
const MODERATION_APPLICATION_LEASE_OWNER: &str = "module-work:moderation-decision-application";

/// Moderation-owned adapter from durable decision application operations to the generic
/// host module-work scheduler.
///
/// Candidate discovery is intentionally read-only. The canonical durable claim remains the
/// CAS in `ModerationService::dispatch_application_operation_once`, so multiple hosts may
/// discover the same row without creating a second lease or dispatch path.
#[derive(Clone)]
pub(crate) struct ModerationApplicationWorkAdapter {
    service: ModerationService,
    registry: Arc<ModerationSubjectAdapterRegistry>,
}

impl ModerationApplicationWorkAdapter {
    pub(crate) fn new(
        database: DatabaseConnection,
        registry: Arc<ModerationSubjectAdapterRegistry>,
    ) -> Self {
        Self {
            service: ModerationService::new(database),
            registry,
        }
    }

    pub(crate) async fn register_with(
        self,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let adapter = Arc::new(self);
        scheduler.register(adapter.clone(), adapter).await
    }

    async fn next_due_candidate(
        &self,
    ) -> Result<Option<moderation_application_operation::Model>, ModuleWorkError> {
        let now = Utc::now().fixed_offset();
        moderation_application_operation::Entity::find()
            .filter(discovery_due_condition(now))
            .order_by_asc(moderation_application_operation::Column::NextAttemptAt)
            .order_by_asc(moderation_application_operation::Column::CreatedAt)
            .one(self.service.database())
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))
    }

    fn work_item(operation: moderation_application_operation::Model) -> ModuleWorkItem {
        ModuleWorkItem {
            id: operation.decision_id,
            tenant_id: operation.tenant_id,
            worker_slug: MODERATION_APPLICATION_WORKER.to_string(),
            // This token belongs only to the generic scheduler envelope. The authoritative
            // Moderation operation lease is created later by the one-attempt CAS dispatcher.
            lease_token: Uuid::new_v4().to_string(),
            payload: serde_json::json!({
                "decision_id": operation.decision_id,
            }),
        }
    }

    fn decision_id(item: &ModuleWorkItem) -> Result<Uuid, ModuleWorkError> {
        let value = item
            .payload
            .get("decision_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ModuleWorkError::Handler(
                    "moderation application work item has no decision_id".to_string(),
                )
            })?;
        let decision_id = Uuid::parse_str(value).map_err(|_| {
            ModuleWorkError::Handler(
                "moderation application work item decision_id is invalid".to_string(),
            )
        })?;
        if decision_id != item.id {
            return Err(ModuleWorkError::Handler(
                "moderation application work item identity mismatch".to_string(),
            ));
        }
        Ok(decision_id)
    }
}

pub(crate) struct ModerationApplicationWorkRegistration;

#[async_trait]
impl ModuleWorkRegistration for ModerationApplicationWorkRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let registry = host
            .shared_get::<Arc<ModerationSubjectAdapterRegistry>>()
            .ok_or_else(|| {
                ModuleWorkError::Handler(
                    "moderation subject adapter registry is not materialized".to_string(),
                )
            })?;
        ModerationApplicationWorkAdapter::new(host.db_clone(), registry)
            .register_with(scheduler)
            .await
    }
}

#[async_trait]
impl ModuleWorkSource for ModerationApplicationWorkAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != MODERATION_APPLICATION_WORKER {
            return Ok(None);
        }
        Ok(self.next_due_candidate().await?.map(Self::work_item))
    }

    async fn complete(
        &self,
        _item: &ModuleWorkItem,
        _outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        // The Moderation application operation is the durable source of truth. The one-attempt
        // dispatcher already records applied/retryable/rejected/operator-review state with its
        // own lease CAS. The generic scheduler must not write a second completion state.
        Ok(())
    }
}

#[async_trait]
impl ModuleWorkHandler for ModerationApplicationWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        MODERATION_APPLICATION_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        if item.worker_slug != MODERATION_APPLICATION_WORKER {
            return Err(ModuleWorkError::Handler(
                "wrong moderation application worker slug".to_string(),
            ));
        }
        let decision_id = Self::decision_id(&item)?;
        self.service
            .dispatch_application_operation_once(
                self.registry.as_ref(),
                item.tenant_id,
                decision_id,
                MODERATION_APPLICATION_LEASE_OWNER,
            )
            .await
            .map_err(|error| ModuleWorkError::Handler(error.to_string()))?;

        // `Some` means this host claimed and finalized/scheduled the durable operation. `None`
        // means another host won the CAS or the row stopped being due. Both are complete from
        // the generic scheduler envelope's perspective.
        Ok(ModuleWorkOutcome::Completed)
    }
}

/// Read-only discovery hint for the shared scheduler. This mirrors the owner due predicate so
/// the scheduler avoids needless calls, but it is not authoritative: the dispatcher repeats
/// the canonical CAS predicate before any domain adapter is invoked.
fn discovery_due_condition(now: chrono::DateTime<chrono::FixedOffset>) -> Condition {
    Condition::any()
        .add(
            Condition::all()
                .add(moderation_application_operation::Column::Status.is_in([
                    ModerationApplicationOperationStatus::Pending.as_str(),
                    ModerationApplicationOperationStatus::Retryable.as_str(),
                ]))
                .add(moderation_application_operation::Column::NextAttemptAt.lte(now)),
        )
        .add(
            Condition::all()
                .add(
                    moderation_application_operation::Column::Status
                        .eq(ModerationApplicationOperationStatus::Applying.as_str()),
                )
                .add(moderation_application_operation::Column::LeaseExpiresAt.lte(now)),
        )
}
