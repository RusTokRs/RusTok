use async_trait::async_trait;
use chrono::{Duration, Utc};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use std::collections::BTreeSet;
use std::sync::Arc;
use uuid::Uuid;

use rustok_api::{
    ModuleWorkError, ModuleWorkHandler, ModuleWorkItem, ModuleWorkOutcome, ModuleWorkSource,
};
use rustok_runtime::{HostRuntimeContext, ModuleWorkRegistration, ModuleWorkScheduler};

use crate::entities::ai_agent_workflow_stages;
use crate::{AiHostRuntime, AiManagementService, AiOperatorContext};

pub const AGENT_WORKFLOW_STAGE_WORKER: &str = "ai_agent_workflow_stage";

/// AI-owned adapter between persisted agent stages and the generic work scheduler.
/// It never exposes an AI table or task type to the platform scheduler.
#[derive(Clone)]
pub struct AiAgentWorkflowWorkAdapter {
    runtime: AiHostRuntime,
    lease_duration: Duration,
}

impl AiAgentWorkflowWorkAdapter {
    pub fn new(runtime: AiHostRuntime, lease_duration: Duration) -> Self {
        Self {
            runtime,
            lease_duration,
        }
    }

    fn worker_operator(tenant_id: Uuid) -> AiOperatorContext {
        AiOperatorContext {
            tenant_id,
            user_id: Uuid::nil(),
            permissions: Vec::new(),
            role_slugs: Vec::new(),
            preferred_locale: None,
        }
    }

    /// Publishes the AI workflow queue through the generic scheduler. The host
    /// only composes this method; it never receives AI tables or task types.
    pub async fn register_with(
        self,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let adapter = Arc::new(self);
        scheduler.register(adapter.clone(), adapter).await
    }

    fn work_ids(item: &ModuleWorkItem) -> Result<(Uuid, Uuid), ModuleWorkError> {
        let stage_id = item
            .payload
            .get("stage_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ModuleWorkError::Handler("work item has no stage_id".to_string()))
            .and_then(|value| {
                Uuid::parse_str(value).map_err(|_| {
                    ModuleWorkError::Handler("work item stage_id is invalid".to_string())
                })
            })?;
        let lease_token = Uuid::parse_str(&item.lease_token).map_err(|_| {
            ModuleWorkError::Handler("work item lease token is invalid".to_string())
        })?;
        Ok((stage_id, lease_token))
    }

    /// Recovers abandoned leases before claiming new work. Recovery remains
    /// tenant-scoped at the service boundary; this adapter only discovers the
    /// affected tenants from its own durable source.
    async fn recover_expired_leases(&self) -> Result<(), ModuleWorkError> {
        let now = Utc::now();
        let tenant_ids = ai_agent_workflow_stages::Entity::find()
            .filter(ai_agent_workflow_stages::Column::Status.eq("running"))
            .filter(ai_agent_workflow_stages::Column::LeaseExpiresAt.lt(now.clone()))
            .select_only()
            .column(ai_agent_workflow_stages::Column::TenantId)
            .into_tuple::<Uuid>()
            .all(self.runtime.db())
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))?
            .into_iter()
            .collect::<BTreeSet<_>>();
        for tenant_id in tenant_ids {
            AiManagementService::requeue_expired_agent_stage_leases(
                self.runtime.db(),
                tenant_id,
                now.clone(),
            )
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))?;
        }
        Ok(())
    }
}

pub(crate) struct AiAgentWorkflowWorkRegistration;

#[async_trait]
impl ModuleWorkRegistration for AiAgentWorkflowWorkRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let runtime =
            crate::ai_host_runtime_from_context(host).map_err(ModuleWorkError::Handler)?;
        AiAgentWorkflowWorkAdapter::new(runtime, Duration::minutes(5))
            .register_with(scheduler)
            .await
    }
}

#[async_trait]
impl ModuleWorkSource for AiAgentWorkflowWorkAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != AGENT_WORKFLOW_STAGE_WORKER {
            return Ok(None);
        }
        self.recover_expired_leases().await?;
        let Some(stage) = ai_agent_workflow_stages::Entity::find()
            .filter(ai_agent_workflow_stages::Column::Status.eq("ready"))
            .order_by_asc(ai_agent_workflow_stages::Column::CreatedAt)
            .one(self.runtime.db())
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))?
        else {
            return Ok(None);
        };
        let lease_token = Uuid::new_v4();
        let claimed = AiManagementService::claim_agent_workflow_stage(
            self.runtime.db(),
            stage.tenant_id,
            stage.id,
            lease_token,
            Utc::now() + self.lease_duration,
        )
        .await
        .map_err(|error| ModuleWorkError::Source(error.to_string()))?;
        if !claimed {
            return Ok(None);
        }
        Ok(Some(ModuleWorkItem {
            id: stage.id,
            tenant_id: stage.tenant_id,
            worker_slug: AGENT_WORKFLOW_STAGE_WORKER.to_string(),
            lease_token: lease_token.to_string(),
            payload: serde_json::json!({"stage_id": stage.id}),
        }))
    }

    async fn complete(
        &self,
        _item: &ModuleWorkItem,
        _outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        // The canonical AI executor finalizes the persisted stage with the same
        // lease token. A failed handler remains recoverable through lease expiry.
        Ok(())
    }
}

#[async_trait]
impl ModuleWorkHandler for AiAgentWorkflowWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        AGENT_WORKFLOW_STAGE_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        if item.worker_slug != AGENT_WORKFLOW_STAGE_WORKER {
            return Err(ModuleWorkError::Handler("wrong AI worker slug".to_string()));
        }
        let (stage_id, lease_token) = Self::work_ids(&item)?;
        AiManagementService::execute_agent_workflow_stage(
            &self.runtime,
            &Self::worker_operator(item.tenant_id),
            stage_id,
            lease_token,
        )
        .await
        .map(|_| ModuleWorkOutcome::Completed)
        .map_err(|error| ModuleWorkError::Handler(error.to_string()))
    }
}
