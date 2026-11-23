use chrono::Utc;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect, Set, Statement,
    TransactionTrait,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::dto::{
    CreateWorkflowInput, CreateWorkflowStepInput, UpdateWorkflowInput, UpdateWorkflowStepInput,
    WorkflowExecutionResponse, WorkflowResponse, WorkflowStepExecutionResponse,
    WorkflowStepResponse, WorkflowSummary, WorkflowVersionDetail, WorkflowVersionSummary,
};
use crate::entities::{
    WorkflowActiveModel, WorkflowEntity, WorkflowExecutionEntity, WorkflowStatus,
    WorkflowStepActiveModel, WorkflowStepEntity, WorkflowStepExecutionEntity,
    WorkflowVersionActiveModel, WorkflowVersionEntity, workflow, workflow_execution, workflow_step,
    workflow_step_execution, workflow_version, workflow_cron_firing, ExecutionStatus,
};
use crate::error::{WorkflowError, WorkflowResult};
use tracing::instrument;

pub struct WorkflowService {
    db: DatabaseConnection,
}

impl WorkflowService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    // ── Workflows ──────────────────────────────────────────────────────────────

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, workflow_id))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: CreateWorkflowInput,
    ) -> WorkflowResult<Uuid> {
        if tenant_id.is_nil() {
            return Err(WorkflowError::InvalidTriggerConfig(
                "tenant_id must not be nil".to_string(),
            ));
        }
        if input.name.trim().is_empty() || input.name.len() > 255 {
            return Err(WorkflowError::InvalidTriggerConfig(
                "workflow name must contain 1 to 255 characters".to_string(),
            ));
        }
        let (webhook_slug, webhook_secret) =
            normalize_webhook_configuration(input.webhook_slug, input.webhook_secret)?;

        let now = Utc::now().fixed_offset();
        let id = Uuid::new_v4();
        tracing::Span::current().record("workflow_id", tracing::field::display(id));

        let model = WorkflowActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            name: Set(input.name),
            description: Set(input.description),
            status: Set(WorkflowStatus::Draft),
            trigger_config: Set(input.trigger_config),
            created_by: Set(actor_id),
            created_at: Set(now),
            updated_at: Set(now),
            failure_count: Set(0),
            auto_disabled_at: Set(None),
            webhook_slug: Set(webhook_slug),
            webhook_secret: Set(webhook_secret),
        };
        model.insert(&self.db).await?;

        Ok(id)
    }

    pub async fn get(&self, tenant_id: Uuid, id: Uuid) -> WorkflowResult<WorkflowResponse> {
        if tenant_id.is_nil() || id.is_nil() {
            return Err(WorkflowError::NotFound(id));
        }
        let workflow = WorkflowEntity::find_by_id(id)
            .filter(workflow::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(WorkflowError::NotFound(id))?;

        let steps = WorkflowStepEntity::find()
            .filter(workflow_step::Column::WorkflowId.eq(id))
            .order_by(workflow_step::Column::Position, Order::Asc)
            .all(&self.db)
            .await?;

        Ok(WorkflowResponse {
            id: workflow.id,
            tenant_id: workflow.tenant_id,
            name: workflow.name,
            description: workflow.description,
            status: workflow.status,
            trigger_config: workflow.trigger_config,
            webhook_slug: workflow.webhook_slug,
            created_by: workflow.created_by,
            created_at: workflow.created_at.into(),
            updated_at: workflow.updated_at.into(),
            failure_count: workflow.failure_count,
            auto_disabled_at: workflow.auto_disabled_at.map(Into::into),
            steps: steps.into_iter().map(step_to_response).collect(),
        })
    }

    pub async fn list(&self, tenant_id: Uuid) -> WorkflowResult<Vec<WorkflowSummary>> {
        if tenant_id.is_nil() {
            return Ok(Vec::new());
        }
        let workflows = WorkflowEntity::find()
            .filter(workflow::Column::TenantId.eq(tenant_id))
            .order_by(workflow::Column::CreatedAt, Order::Desc)
            .all(&self.db)
            .await?;

        Ok(workflows
            .into_iter()
            .map(|w| WorkflowSummary {
                id: w.id,
                tenant_id: w.tenant_id,
                name: w.name,
                status: w.status,
                webhook_slug: w.webhook_slug,
                failure_count: w.failure_count,
                created_at: w.created_at.into(),
                updated_at: w.updated_at.into(),
            })
            .collect())
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, id = %id))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        actor_id: Option<Uuid>,
        input: UpdateWorkflowInput,
    ) -> WorkflowResult<()> {
        if tenant_id.is_nil() || id.is_nil() {
            return Err(WorkflowError::NotFound(id));
        }
        if let Some(ref name) = input.name
            && (name.trim().is_empty() || name.len() > 255)
        {
            return Err(WorkflowError::InvalidTriggerConfig(
                "workflow name must contain 1 to 255 characters".to_string(),
            ));
        }
        let transaction = self.db.begin().await?;
        let existing = lock_workflow_for_update(&transaction, tenant_id, id).await?;

        let next_webhook_slug = input
            .webhook_slug
            .clone()
            .or_else(|| existing.webhook_slug.clone());
        let next_webhook_secret = match input.webhook_secret.clone() {
            Some(secret) if secret.is_empty() => None,
            Some(secret) => Some(secret),
            None => existing.webhook_secret.clone(),
        };
        let (next_webhook_slug, next_webhook_secret) =
            normalize_webhook_configuration(next_webhook_slug, next_webhook_secret)?;
        // Drafts may be incomplete, but activating or modifying an active
        // cron must fail at the owner write boundary instead of leaving an
        // apparently active workflow that the scheduler silently skips.
        if input.status.as_ref().unwrap_or(&existing.status) == &WorkflowStatus::Active {
            validate_active_cron_trigger(
                input.trigger_config.as_ref().unwrap_or(&existing.trigger_config),
            )?;
        }

        // Save version snapshot before applying the update while the workflow row is locked.
        self.save_version_internal_on(&transaction, id, actor_id, &existing)
            .await?;

        let mut model: WorkflowActiveModel = existing.into();
        if let Some(name) = input.name {
            model.name = Set(name);
        }
        if let Some(description) = input.description {
            model.description = Set(Some(description));
        }
        if let Some(status) = input.status {
            model.status = Set(status);
        }
        if let Some(trigger_config) = input.trigger_config {
            model.trigger_config = Set(trigger_config);
        }
        model.webhook_slug = Set(next_webhook_slug);
        model.webhook_secret = Set(next_webhook_secret);
        model.updated_at = Set(Utc::now().fixed_offset());
        model.update(&transaction).await?;
        transaction.commit().await?;

        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, id = %id))]
    pub async fn delete(&self, tenant_id: Uuid, id: Uuid) -> WorkflowResult<()> {
        if tenant_id.is_nil() || id.is_nil() {
            return Err(WorkflowError::NotFound(id));
        }
        let transaction = self.db.begin().await?;
        let existing = lock_workflow_for_update(&transaction, tenant_id, id).await?;

        // A hard delete would cascade to the only execution and recovery
        // evidence, even while external effects may still be running.
        // The same parent-row lock also serializes with the durable cron
        // reservation transaction before it starts issuing effects.
        let unresolved_execution = WorkflowExecutionEntity::find()
            .filter(workflow_execution::Column::WorkflowId.eq(id))
            .filter(workflow_execution::Column::TenantId.eq(tenant_id))
            .filter(workflow_execution::Column::Status.is_in([
                ExecutionStatus::Running.to_string(),
                ExecutionStatus::ReconciliationRequired.to_string(),
            ]))
            .one(&transaction).await?.is_some();
        let unresolved_firing = workflow_cron_firing::Entity::find()
            .filter(workflow_cron_firing::Column::WorkflowId.eq(id))
            .filter(workflow_cron_firing::Column::TenantId.eq(tenant_id))
            .filter(workflow_cron_firing::Column::Status.is_in([
                workflow_cron_firing::PENDING,
                workflow_cron_firing::RUNNING,
                workflow_cron_firing::RECONCILIATION_REQUIRED,
            ]))
            .one(&transaction).await?.is_some();
        if unresolved_execution || unresolved_firing {
            return Err(WorkflowError::WorkflowDeletionBlocked(id));
        }

        let model: WorkflowActiveModel = existing.into();
        model.delete(&transaction).await?;
        transaction.commit().await?;

        Ok(())
    }

    // ── Steps ──────────────────────────────────────────────────────────────────

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, workflow_id = %workflow_id))]
    pub async fn add_step(
        &self,
        tenant_id: Uuid,
        workflow_id: Uuid,
        input: CreateWorkflowStepInput,
    ) -> WorkflowResult<Uuid> {
        if tenant_id.is_nil() || workflow_id.is_nil() {
            return Err(WorkflowError::NotFound(workflow_id));
        }
        super::engine::validate_step_timeout_ms(input.timeout_ms)?;
        let transaction = self.db.begin().await?;
        // Lock the parent workflow so concurrent workflow updates/restores cannot
        // interleave with step creation.
        let parent = lock_workflow_for_update(&transaction, tenant_id, workflow_id).await?;

        let step_id = Uuid::new_v4();
        let model = WorkflowStepActiveModel {
            id: Set(step_id),
            workflow_id: Set(workflow_id),
            position: Set(input.position),
            step_type: Set(input.step_type),
            config: Set(input.config),
            on_error: Set(input.on_error),
            timeout_ms: Set(input.timeout_ms),
        };
        model.insert(&transaction).await?;
        touch_workflow_after_step_mutation(&transaction, parent).await?;
        transaction.commit().await?;

        Ok(step_id)
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, workflow_id = %workflow_id, step_id = %step_id))]
    pub async fn update_step(
        &self,
        tenant_id: Uuid,
        workflow_id: Uuid,
        step_id: Uuid,
        input: UpdateWorkflowStepInput,
    ) -> WorkflowResult<()> {
        if tenant_id.is_nil() || workflow_id.is_nil() || step_id.is_nil() {
            return Err(WorkflowError::StepNotFound(step_id));
        }
        super::engine::validate_step_timeout_ms(input.timeout_ms)?;
        let transaction = self.db.begin().await?;
        let parent = lock_workflow_for_update(&transaction, tenant_id, workflow_id).await?;

        let existing = WorkflowStepEntity::find_by_id(step_id)
            .filter(workflow_step::Column::WorkflowId.eq(workflow_id))
            .one(&transaction)
            .await?
            .ok_or(WorkflowError::StepNotFound(step_id))?;

        let mut model: WorkflowStepActiveModel = existing.into();
        if let Some(pos) = input.position {
            model.position = Set(pos);
        }
        if let Some(step_type) = input.step_type {
            model.step_type = Set(step_type);
        }
        if let Some(config) = input.config {
            model.config = Set(config);
        }
        if let Some(on_error) = input.on_error {
            model.on_error = Set(on_error);
        }
        if let Some(timeout_ms) = input.timeout_ms {
            model.timeout_ms = Set(Some(timeout_ms));
        }
        model.update(&transaction).await?;
        touch_workflow_after_step_mutation(&transaction, parent).await?;
        transaction.commit().await?;

        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, workflow_id = %workflow_id, step_id = %step_id))]
    pub async fn delete_step(
        &self,
        tenant_id: Uuid,
        workflow_id: Uuid,
        step_id: Uuid,
    ) -> WorkflowResult<()> {
        if tenant_id.is_nil() || workflow_id.is_nil() || step_id.is_nil() {
            return Err(WorkflowError::StepNotFound(step_id));
        }
        let transaction = self.db.begin().await?;
        let parent = lock_workflow_for_update(&transaction, tenant_id, workflow_id).await?;

        let existing = WorkflowStepEntity::find_by_id(step_id)
            .filter(workflow_step::Column::WorkflowId.eq(workflow_id))
            .one(&transaction)
            .await?
            .ok_or(WorkflowError::StepNotFound(step_id))?;

        let model: WorkflowStepActiveModel = existing.into();
        model.delete(&transaction).await?;
        touch_workflow_after_step_mutation(&transaction, parent).await?;
        transaction.commit().await?;

        Ok(())
    }

    /// Load ordered steps for a workflow (used by the engine).
    pub async fn load_steps(
        &self,
        workflow_id: Uuid,
    ) -> WorkflowResult<Vec<crate::entities::WorkflowStep>> {
        if workflow_id.is_nil() {
            return Ok(Vec::new());
        }
        let steps = WorkflowStepEntity::find()
            .filter(workflow_step::Column::WorkflowId.eq(workflow_id))
            .order_by(workflow_step::Column::Position, Order::Asc)
            .all(&self.db)
            .await?;
        Ok(steps)
    }

    /// Manually trigger a workflow execution.
    /// Requires the workflow to be Active or explicitly bypassed via `force`.
    pub async fn trigger_manual(
        &self,
        tenant_id: Uuid,
        workflow_id: Uuid,
        actor_id: Option<Uuid>,
        payload: serde_json::Value,
        force: bool,
    ) -> WorkflowResult<Uuid> {
        if tenant_id.is_nil() || workflow_id.is_nil() {
            return Err(WorkflowError::NotFound(workflow_id));
        }
        let workflow = WorkflowEntity::find_by_id(workflow_id)
            .filter(workflow::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(WorkflowError::NotFound(workflow_id))?;

        if !force && workflow.status != WorkflowStatus::Active {
            return Err(WorkflowError::NotActive(workflow.status.to_string()));
        }

        let steps = self.load_steps(workflow_id).await?;

        let initial_context = serde_json::json!({
            "trigger": { "type": "manual", "actor_id": actor_id },
            "payload": payload
        });

        let engine = crate::services::WorkflowEngine::new(self.db.clone());
        let execution_id = engine
            .execute(workflow_id, tenant_id, None, steps, initial_context)
            .await?;

        Ok(execution_id)
    }

    /// Increment failure counter and auto-disable after threshold.
    pub async fn record_failure(
        &self,
        tenant_id: Uuid,
        workflow_id: Uuid,
        auto_disable_threshold: u32,
    ) -> WorkflowResult<()> {
        if tenant_id.is_nil() || workflow_id.is_nil() {
            return Err(WorkflowError::NotFound(workflow_id));
        }
        // A zero threshold cannot represent a positive number of consecutive
        // failures. Converting a large u32 to i32 with `as` used to wrap.
        let threshold = i32::try_from(auto_disable_threshold).ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| WorkflowError::InvalidTriggerConfig(
                "auto_disable_threshold must be between 1 and i32::MAX".into(),
            ))?;

        // The failure counter and an optional transition to paused must be
        // serialized with manual updates, restores and concurrent failures.
        // A failed/paused workflow must not be re-paused based on a stale read,
        // nor should it overwrite a newer operator status transition.
        let transaction = self.db.begin().await?;
        let existing = lock_workflow_for_update(&transaction, tenant_id, workflow_id).await?;
        if existing.status != WorkflowStatus::Active {
            transaction.commit().await?;
            return Ok(());
        }
        let next_failure_count = existing.failure_count.checked_add(1)
            .ok_or_else(|| WorkflowError::InvalidTriggerConfig(
                "workflow consecutive failure counter exhausted".into(),
            ))?;
        let should_disable = next_failure_count >= threshold;
        let mut model: WorkflowActiveModel = existing.into();
        model.failure_count = Set(next_failure_count);
        model.updated_at = Set(Utc::now().fixed_offset());
        if should_disable {
            model.status = Set(WorkflowStatus::Paused);
            model.auto_disabled_at = Set(Some(Utc::now().fixed_offset()));
        }
        model.update(&transaction).await?;
        transaction.commit().await?;

        if should_disable {
            tracing::warn!(
                workflow_id = %workflow_id,
                threshold,
                "Workflow auto-disabled after consecutive failures"
            );
        }
        Ok(())
    }

    // ── Execution queries ──────────────────────────────────────────────────────

    /// List executions for a workflow (most recent first, limit 50).
    pub async fn list_executions(
        &self,
        tenant_id: Uuid,
        workflow_id: Uuid,
    ) -> WorkflowResult<Vec<WorkflowExecutionResponse>> {
        if tenant_id.is_nil() || workflow_id.is_nil() {
            return Ok(Vec::new());
        }
        let executions: Vec<crate::entities::WorkflowExecution> = WorkflowExecutionEntity::find()
            .filter(workflow_execution::Column::WorkflowId.eq(workflow_id))
            .filter(workflow_execution::Column::TenantId.eq(tenant_id))
            .order_by(workflow_execution::Column::StartedAt, Order::Desc)
            .limit(50)
            .all(&self.db)
            .await?;

        if executions.is_empty() {
            return Ok(Vec::new());
        }

        // One bounded batch for the step logs, not one query per execution.
        // The execution IDs come only from the tenant-scoped parent query, so
        // unrelated tenants cannot inject their step histories into this view.
        let execution_ids: Vec<Uuid> = executions.iter().map(|item| item.id).collect();
        let step_rows = WorkflowStepExecutionEntity::find()
            .filter(workflow_step_execution::Column::ExecutionId.is_in(execution_ids))
            .order_by(workflow_step_execution::Column::StartedAt, Order::Asc)
            .order_by(workflow_step_execution::Column::Id, Order::Asc)
            .all(&self.db)
            .await?;
        let mut by_execution: std::collections::HashMap<
            Uuid, Vec<crate::entities::WorkflowStepExecution>
        > = std::collections::HashMap::new();
        for row in step_rows {
            by_execution.entry(row.execution_id).or_default().push(row);
        }
        Ok(executions.into_iter()
            .map(|execution| {
                let steps = by_execution.remove(&execution.id).unwrap_or_default();
                execution_to_response(execution, steps)
            })
            .collect())
    }

    /// Get a single execution by id (tenant-scoped).
    pub async fn get_execution(
        &self,
        tenant_id: Uuid,
        execution_id: Uuid,
    ) -> WorkflowResult<WorkflowExecutionResponse> {
        if tenant_id.is_nil() || execution_id.is_nil() {
            return Err(WorkflowError::ExecutionNotFound(execution_id));
        }
        let exec = WorkflowExecutionEntity::find_by_id(execution_id)
            .filter(workflow_execution::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(WorkflowError::ExecutionNotFound(execution_id))?;

        let step_execs = WorkflowStepExecutionEntity::find()
            .filter(workflow_step_execution::Column::ExecutionId.eq(exec.id))
            .order_by(workflow_step_execution::Column::StartedAt, Order::Asc)
            .all(&self.db)
            .await?;

        Ok(execution_to_response(exec, step_execs))
    }

    // ── Webhook trigger ────────────────────────────────────────────────────────

    /// Trigger all active workflows with a matching webhook slug.
    /// Returns a list of spawned execution IDs.
    pub async fn trigger_by_webhook(
        &self,
        tenant_id: Uuid,
        webhook_slug: &str,
        body: &[u8],
        signature: Option<&str>,
    ) -> WorkflowResult<Vec<Uuid>> {
        if tenant_id.is_nil() || webhook_slug.trim().is_empty() {
            return Ok(vec![]);
        }
        let matching = WorkflowEntity::find()
            .filter(workflow::Column::TenantId.eq(tenant_id))
            .filter(workflow::Column::Status.eq(WorkflowStatus::Active.to_string()))
            .filter(workflow::Column::WebhookSlug.eq(webhook_slug))
            .all(&self.db)
            .await?;

        if matching.is_empty() {
            return Ok(vec![]);
        }

        let signature = signature.ok_or(WorkflowError::WebhookSignatureMissing)?;
        for workflow in &matching {
            let Some(secret) = workflow.webhook_secret.as_deref() else {
                return Err(WorkflowError::WebhookSecretNotConfigured);
            };
            if !verify_webhook_signature(secret, body, signature) {
                return Err(WorkflowError::WebhookSignatureInvalid);
            }
        }

        let payload: serde_json::Value = serde_json::from_slice(body).unwrap_or_else(|_| {
            serde_json::Value::String(String::from_utf8_lossy(body).into_owned())
        });
        let engine = std::sync::Arc::new(crate::services::WorkflowEngine::new(self.db.clone()));
        let initial_context = serde_json::json!({
            "webhook": { "slug": webhook_slug, "payload": payload }
        });

        let mut execution_ids = Vec::new();

        for wf in matching {
            let wf_id = wf.id;
            let steps = self.load_steps(wf_id).await?;
            let ctx = initial_context.clone();
            let engine = engine.clone();

            let execution_id = engine.execute(wf_id, tenant_id, None, steps, ctx).await?;
            execution_ids.push(execution_id);
        }

        Ok(execution_ids)
    }

    // ── Versioning ─────────────────────────────────────────────────────────────

    /// Save a version snapshot of the current workflow state.
    async fn save_version_internal_on<C>(
        &self,
        conn: &C,
        workflow_id: Uuid,
        actor_id: Option<Uuid>,
        wf: &crate::entities::Workflow,
    ) -> WorkflowResult<i32>
    where
        C: ConnectionTrait,
    {
        // Get next version number while the parent workflow row is already locked
        // by the caller. This serializes version allocation for this workflow.
        let max_version: Option<i32> = WorkflowVersionEntity::find()
            .filter(workflow_version::Column::WorkflowId.eq(workflow_id))
            .order_by(workflow_version::Column::Version, Order::Desc)
            .limit(1)
            .one(conn)
            .await?
            .map(|v| v.version);

        let version = max_version.unwrap_or(0) + 1;

        let steps = WorkflowStepEntity::find()
            .filter(workflow_step::Column::WorkflowId.eq(workflow_id))
            .order_by(workflow_step::Column::Position, Order::Asc)
            .all(conn)
            .await?;

        let snapshot = serde_json::json!({
            "id": wf.id,
            "name": wf.name,
            "description": wf.description,
            "status": wf.status,
            "trigger_config": wf.trigger_config,
            "webhook_slug": wf.webhook_slug,
            "steps": steps.iter().map(|s| serde_json::json!({
                "id": s.id,
                "position": s.position,
                "step_type": s.step_type,
                "config": s.config,
                "on_error": s.on_error,
                "timeout_ms": s.timeout_ms,
            })).collect::<Vec<_>>(),
        });

        let ver = WorkflowVersionActiveModel {
            id: Set(Uuid::new_v4()),
            workflow_id: Set(workflow_id),
            version: Set(version),
            snapshot: Set(snapshot),
            created_by: Set(actor_id),
            created_at: Set(Utc::now().fixed_offset()),
        };
        ver.insert(conn).await?;

        let old_versions = WorkflowVersionEntity::find()
            .filter(workflow_version::Column::WorkflowId.eq(workflow_id))
            .order_by(workflow_version::Column::Version, Order::Desc)
            .offset(20)
            .all(conn)
            .await?;

        for old in old_versions {
            let am: WorkflowVersionActiveModel = old.into();
            am.delete(conn).await?;
        }

        Ok(version)
    }

    /// List all saved versions for a workflow (newest first).
    pub async fn list_versions(
        &self,
        tenant_id: Uuid,
        workflow_id: Uuid,
    ) -> WorkflowResult<Vec<WorkflowVersionSummary>> {
        if tenant_id.is_nil() || workflow_id.is_nil() {
            return Err(WorkflowError::NotFound(workflow_id));
        }
        // Verify ownership
        WorkflowEntity::find_by_id(workflow_id)
            .filter(workflow::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(WorkflowError::NotFound(workflow_id))?;

        let versions = WorkflowVersionEntity::find()
            .filter(workflow_version::Column::WorkflowId.eq(workflow_id))
            .order_by(workflow_version::Column::Version, Order::Desc)
            .all(&self.db)
            .await?;

        Ok(versions
            .into_iter()
            .map(|v| WorkflowVersionSummary {
                id: v.id,
                workflow_id: v.workflow_id,
                version: v.version,
                created_by: v.created_by,
                created_at: v.created_at.into(),
            })
            .collect())
    }

    /// Get a specific version detail (includes full snapshot).
    pub async fn get_version(
        &self,
        tenant_id: Uuid,
        workflow_id: Uuid,
        version: i32,
    ) -> WorkflowResult<WorkflowVersionDetail> {
        if tenant_id.is_nil() || workflow_id.is_nil() {
            return Err(WorkflowError::NotFound(workflow_id));
        }
        // Verify ownership
        WorkflowEntity::find_by_id(workflow_id)
            .filter(workflow::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(WorkflowError::NotFound(workflow_id))?;

        let ver = WorkflowVersionEntity::find()
            .filter(workflow_version::Column::WorkflowId.eq(workflow_id))
            .filter(workflow_version::Column::Version.eq(version))
            .one(&self.db)
            .await?
            .ok_or_else(|| WorkflowError::VersionNotFound(workflow_id, version))?;

        Ok(WorkflowVersionDetail {
            id: ver.id,
            workflow_id: ver.workflow_id,
            version: ver.version,
            snapshot: ver.snapshot,
            created_by: ver.created_by,
            created_at: ver.created_at.into(),
        })
    }

    /// Restore a workflow to a previously saved version.
    /// Saves a new version (the current state) before overwriting.
    pub async fn restore_version(
        &self,
        tenant_id: Uuid,
        workflow_id: Uuid,
        version: i32,
        actor_id: Option<Uuid>,
    ) -> WorkflowResult<()> {
        if tenant_id.is_nil() || workflow_id.is_nil() {
            return Err(WorkflowError::NotFound(workflow_id));
        }
        let transaction = self.db.begin().await?;
        let existing = lock_workflow_for_update(&transaction, tenant_id, workflow_id).await?;

        let ver = WorkflowVersionEntity::find()
            .filter(workflow_version::Column::WorkflowId.eq(workflow_id))
            .filter(workflow_version::Column::Version.eq(version))
            .one(&transaction)
            .await?
            .ok_or_else(|| WorkflowError::VersionNotFound(workflow_id, version))?;

        let snapshot = ver.snapshot.clone();
        if existing.status == WorkflowStatus::Active {
            validate_active_cron_trigger(
                snapshot.get("trigger_config").unwrap_or(&existing.trigger_config),
            )?;
        }

        // Save current state as a new version before restoring.
        self.save_version_internal_on(&transaction, workflow_id, actor_id, &existing)
            .await?;

        // Apply snapshot
        let mut model: WorkflowActiveModel = existing.into();
        if let Some(name) = snapshot.get("name").and_then(|v| v.as_str()) {
            model.name = Set(name.to_string());
        }
        model.description = Set(snapshot
            .get("description")
            .and_then(|v| v.as_str())
            .map(str::to_string));
        if let Some(tc) = snapshot.get("trigger_config").cloned() {
            model.trigger_config = Set(tc);
        }
        if let Some(slug) = snapshot.get("webhook_slug") {
            model.webhook_slug = Set(slug
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_string));
        }
        model.updated_at = Set(Utc::now().fixed_offset());
        model.update(&transaction).await?;

        // Restore steps: delete all current steps and re-insert from snapshot
        WorkflowStepEntity::delete_many()
            .filter(workflow_step::Column::WorkflowId.eq(workflow_id))
            .exec(&transaction)
            .await?;

        if let Some(steps) = snapshot.get("steps").and_then(|v| v.as_array()) {
            for step in steps {
                let step_id = step
                    .get("id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<Uuid>().ok())
                    .unwrap_or_else(Uuid::new_v4);

                let position = step.get("position").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let step_type: crate::entities::StepType = step
                    .get("step_type")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or(crate::entities::StepType::Action);
                let config = step
                    .get("config")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                let on_error: crate::entities::OnError = step
                    .get("on_error")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or(crate::entities::OnError::Stop);
                let timeout_ms = step.get("timeout_ms").and_then(|v| v.as_i64());
                super::engine::validate_step_timeout_ms(timeout_ms)?;

                let am = WorkflowStepActiveModel {
                    id: Set(step_id),
                    workflow_id: Set(workflow_id),
                    position: Set(position),
                    step_type: Set(step_type),
                    config: Set(config),
                    on_error: Set(on_error),
                    timeout_ms: Set(timeout_ms),
                };
                am.insert(&transaction).await?;
            }
        }

        transaction.commit().await?;
        Ok(())
    }

    // ── Template-based creation ────────────────────────────────────────────────

    /// Create a workflow from a built-in marketplace template.
    #[instrument(skip(self), fields(tenant_id = %tenant_id, template_id = %template_id))]
    pub async fn create_from_template(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        template_id: &str,
        name: String,
    ) -> WorkflowResult<Uuid> {
        if tenant_id.is_nil() {
            return Err(WorkflowError::InvalidTriggerConfig(
                "tenant_id must not be nil".to_string(),
            ));
        }
        let template_id = template_id.trim();
        if template_id.is_empty() || template_id.len() > 128 {
            return Err(WorkflowError::TemplateNotFound(template_id.to_string()));
        }
        let name = name.trim().to_string();
        if name.is_empty() || name.len() > 255 {
            return Err(WorkflowError::InvalidTriggerConfig(
                "workflow name must contain 1 to 255 characters".to_string(),
            ));
        }
        let template = crate::templates::BUILTIN_TEMPLATES
            .iter()
            .find(|t| t.id == template_id)
            .ok_or_else(|| WorkflowError::TemplateNotFound(template_id.to_string()))?;

        let workflow_id = self
            .create(
                tenant_id,
                actor_id,
                CreateWorkflowInput {
                    name,
                    description: Some(template.description.to_string()),
                    trigger_config: template.trigger_config.clone(),
                    webhook_slug: None,
                    webhook_secret: None,
                },
            )
            .await?;

        for (i, step) in template.steps.iter().enumerate() {
            self.add_step(
                tenant_id,
                workflow_id,
                crate::dto::CreateWorkflowStepInput {
                    position: i as i32,
                    step_type: step.step_type.clone(),
                    config: step.config.clone(),
                    on_error: step.on_error.clone(),
                    timeout_ms: step.timeout_ms,
                },
            )
            .await?;
        }

        Ok(workflow_id)
    }

    pub async fn reset_failure_count(&self, workflow_id: Uuid) -> WorkflowResult<()> {
        if workflow_id.is_nil() {
            return Ok(());
        }
        use sea_orm::sea_query::Expr;

        workflow::Entity::update_many()
            .col_expr(workflow::Column::FailureCount, Expr::value(0i32))
            .filter(workflow::Column::Id.eq(workflow_id))
            .exec(&self.db)
            .await?;

        Ok(())
    }
}

fn normalize_webhook_configuration(
    webhook_slug: Option<String>,
    webhook_secret: Option<String>,
) -> WorkflowResult<(Option<String>, Option<String>)> {
    let webhook_slug = webhook_slug.filter(|value| !value.is_empty());
    let webhook_secret = webhook_secret.filter(|value| !value.is_empty());

    match (webhook_slug, webhook_secret) {
        (None, None) => Ok((None, None)),
        (Some(slug), Some(secret)) => {
            if secret.len() < 32 || secret.len() > 128 {
                return Err(WorkflowError::InvalidTriggerConfig(
                    "webhook_secret must contain 32..=128 bytes".to_string(),
                ));
            }
            Ok((Some(slug), Some(secret)))
        }
        (Some(_), None) => Err(WorkflowError::InvalidTriggerConfig(
            "webhook_secret is required when webhook_slug is configured".to_string(),
        )),
        (None, Some(_)) => Err(WorkflowError::InvalidTriggerConfig(
            "webhook_secret requires webhook_slug".to_string(),
        )),
    }
}

fn verify_webhook_signature(secret: &str, body: &[u8], signature: &str) -> bool {
    let supplied = signature.strip_prefix("sha256=").unwrap_or(signature);
    let Ok(supplied) = decode_hex_signature(supplied) else {
        return false;
    };

    let mut key_block = [0_u8; 64];
    let secret_bytes = secret.as_bytes();
    if secret_bytes.len() > key_block.len() {
        let digest = Sha256::digest(secret_bytes);
        key_block[..digest.len()].copy_from_slice(&digest);
    } else {
        key_block[..secret_bytes.len()].copy_from_slice(secret_bytes);
    }

    let mut inner = Sha256::new();
    let ipad = [0x36_u8; 64];
    for (index, byte) in key_block.iter().enumerate() {
        inner.update([*byte ^ ipad[index]]);
    }
    inner.update(body);
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    let opad = [0x5c_u8; 64];
    for (index, byte) in key_block.iter().enumerate() {
        outer.update([*byte ^ opad[index]]);
    }
    outer.update(inner_digest);
    let expected = outer.finalize();

    if supplied.len() != expected.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (actual, expected) in supplied.iter().zip(expected.iter()) {
        difference |= actual ^ expected;
    }
    difference == 0
}

fn decode_hex_signature(value: &str) -> Result<Vec<u8>, ()> {
    if value.len() != 64 {
        return Err(());
    }
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(32);
    for &[high_byte, low_byte] in bytes.as_chunks::<2>().0 {
        let high = hex_nibble(high_byte).ok_or(())?;
        let low = hex_nibble(low_byte).ok_or(())?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Only active schedules must be immediately runnable. Invalid drafts
/// remain editable, but entering the active state requires a bounded cron
/// expression accepted by the exact scheduler parser.
fn validate_active_cron_trigger(config: &serde_json::Value) -> WorkflowResult<()> {
    if config.get("type").and_then(|value| value.as_str()) == Some("cron") {
        let expression = config.get("expression")
            .and_then(|value| value.as_str())
            .ok_or_else(|| WorkflowError::InvalidTriggerConfig(
                "an active cron requires a string expression".to_owned(),
            ))?;
        if expression.is_empty() || expression.len() > 512 {
            return Err(WorkflowError::InvalidTriggerConfig(
                "cron expression must contain 1 to 512 bytes".to_owned(),
            ));
        }
        expression.parse::<cron::Schedule>().map_err(|_| {
            WorkflowError::InvalidTriggerConfig(
                "cron expression is not accepted by the scheduler".to_owned(),
            )
        })?;
    }
    Ok(())
}

/// Step edits must advance the parent revision in the SAME transaction;
/// otherwise cron firings queued under an old step definition may be
/// executed against a newly edited step graph without being invalidated.
async fn touch_workflow_after_step_mutation(
    transaction: &sea_orm::DatabaseTransaction,
    parent: crate::entities::Workflow,
) -> WorkflowResult<()> {
    let mut model: WorkflowActiveModel = parent.into();
    model.updated_at = Set(Utc::now().fixed_offset());
    model.update(transaction).await?;
    Ok(())
}

pub(crate) async fn lock_workflow_for_update(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    workflow_id: Uuid,
) -> WorkflowResult<crate::entities::Workflow> {
    if tenant_id.is_nil() || workflow_id.is_nil() {
        return Err(WorkflowError::NotFound(workflow_id));
    }
    let query =
        WorkflowEntity::find_by_id(workflow_id).filter(workflow::Column::TenantId.eq(tenant_id));

    let workflow = match txn.get_database_backend() {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => {
            query.lock_exclusive().one(txn).await?
        }
        DatabaseBackend::Sqlite => {
            let statement = Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE workflows SET updated_at = updated_at WHERE tenant_id = ?1 AND id = ?2",
                [tenant_id.into(), workflow_id.into()],
            );
            txn.execute_raw(statement).await?;
            query.one(txn).await?
        }
        _ => query.one(txn).await?,
    };

    workflow.ok_or(WorkflowError::NotFound(workflow_id))
}

fn execution_to_response(
    exec: crate::entities::WorkflowExecution,
    step_execs: Vec<crate::entities::WorkflowStepExecution>,
) -> WorkflowExecutionResponse {
    use chrono::DateTime;
    WorkflowExecutionResponse {
        id: exec.id,
        workflow_id: exec.workflow_id,
        tenant_id: exec.tenant_id,
        trigger_event_id: exec.trigger_event_id,
        status: exec.status,
        context: exec.context,
        error: exec.error,
        started_at: DateTime::from(exec.started_at),
        completed_at: exec.completed_at.map(DateTime::from),
        step_executions: step_execs
            .into_iter()
            .map(step_execution_to_response)
            .collect(),
    }
}

fn step_execution_to_response(
    s: crate::entities::WorkflowStepExecution,
) -> WorkflowStepExecutionResponse {
    use chrono::DateTime;
    WorkflowStepExecutionResponse {
        id: s.id,
        execution_id: s.execution_id,
        step_id: s.step_id,
        status: s.status,
        input: s.input,
        output: s.output,
        error: s.error,
        started_at: DateTime::from(s.started_at),
        completed_at: s.completed_at.map(DateTime::from),
    }
}

fn step_to_response(s: crate::entities::WorkflowStep) -> WorkflowStepResponse {
    WorkflowStepResponse {
        id: s.id,
        workflow_id: s.workflow_id,
        position: s.position,
        step_type: s.step_type,
        config: s.config,
        on_error: s.on_error,
        timeout_ms: s.timeout_ms,
    }
}

#[cfg(test)]
mod cron_revision_tests {
    use super::*;
    use chrono::Duration as ChronoDuration;
    use sea_orm::Database;
    use sea_orm_migration::SchemaManager;
    use crate::entities::{OnError, StepType};

    #[tokio::test]
    async fn execution_history_batches_steps_without_cross_tenant_leakage() {
        use crate::entities::{
            workflow_execution, workflow_step, workflow_step_execution,
            ExecutionStatus, StepExecutionStatus,
        };
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for migration in crate::migrations::migrations() {
            migration.up(&SchemaManager::new(&db)).await.unwrap();
        }
        let now = Utc::now().fixed_offset();
        let tenant_id = Uuid::new_v4();
        let other_tenant = Uuid::new_v4();
        let own_workflow = Uuid::new_v4();
        let other_workflow = Uuid::new_v4();
        let service = WorkflowService::new(db.clone());
        for (tenant, workflow_id) in [
            (tenant_id, own_workflow),
            (other_tenant, other_workflow),
        ] {
            workflow::ActiveModel {
                id: Set(workflow_id),
                tenant_id: Set(tenant),
                name: Set(format!("workflow-{workflow_id}")),
                description: Set(None),
                status: Set(WorkflowStatus::Active),
                trigger_config: Set(serde_json::json!({"type":"manual"})),
                created_by: Set(None),
                created_at: Set(now),
                updated_at: Set(now),
                failure_count: Set(0),
                auto_disabled_at: Set(None),
                webhook_slug: Set(None),
                webhook_secret: Set(None),
            }.insert(&db).await.unwrap();

            let step_id = Uuid::new_v4();
            workflow_step::ActiveModel {
                id: Set(step_id),
                workflow_id: Set(workflow_id),
                position: Set(0),
                step_type: Set(StepType::Action),
                config: Set(serde_json::json!({})),
                on_error: Set(OnError::Stop),
                timeout_ms: Set(None),
            }.insert(&db).await.unwrap();

            for n in 0..2 {
                let execution_id = Uuid::new_v4();
                workflow_execution::ActiveModel {
                    id: Set(execution_id),
                    workflow_id: Set(workflow_id),
                    tenant_id: Set(tenant),
                    trigger_event_id: Set(None),
                    status: Set(ExecutionStatus::Completed),
                    context: Set(serde_json::Value::Null),
                    error: Set(None),
                    started_at: Set(now),
                    completed_at: Set(Some(now)),
                }.insert(&db).await.unwrap();

                workflow_step_execution::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    execution_id: Set(execution_id),
                    step_id: Set(step_id),
                    status: Set(StepExecutionStatus::Completed),
                    input: Set(serde_json::json!({"order": n})),
                    output: Set(serde_json::json!({"tenant":tenant.to_string()})),
                    error: Set(None),
                    started_at: Set(now),
                    completed_at: Set(Some(now)),
                }.insert(&db).await.unwrap();
            }
        }

        let rows = service.list_executions(tenant_id, own_workflow).await.unwrap();
        assert_eq!(rows.len(), 2);
        for row in rows {
            assert_eq!(row.tenant_id, tenant_id);
            assert_eq!(row.step_executions.len(), 1);
            assert_eq!(
                row.step_executions[0].output["tenant"].as_str(),
                Some(tenant_id.to_string().as_str()),
            );
        }
        assert!(service.list_executions(other_tenant, own_workflow)
            .await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn failure_tracking_is_tenant_scoped_atomic_and_does_not_repause() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for migration in crate::migrations::migrations() {
            migration.up(&SchemaManager::new(&db)).await.unwrap();
        }
        let owner = Uuid::new_v4();
        let other_tenant = Uuid::new_v4();
        let workflow_id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();
        workflow::ActiveModel {
            id: Set(workflow_id),
            tenant_id: Set(owner),
            name: Set("failure threshold".into()),
            description: Set(None),
            status: Set(WorkflowStatus::Active),
            trigger_config: Set(serde_json::json!({"type":"manual"})),
            created_by: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            failure_count: Set(0),
            auto_disabled_at: Set(None),
            webhook_slug: Set(None),
            webhook_secret: Set(None),
        }.insert(&db).await.unwrap();

        let service = WorkflowService::new(db.clone());
        assert!(matches!(
            service.record_failure(other_tenant, workflow_id, 2).await,
            Err(WorkflowError::NotFound(_))
        ));
        assert!(matches!(
            service.record_failure(owner, workflow_id, 0).await,
            Err(WorkflowError::InvalidTriggerConfig(_))
        ));
        assert!(matches!(
            service.record_failure(owner, workflow_id, u32::MAX).await,
            Err(WorkflowError::InvalidTriggerConfig(_))
        ));
        service.record_failure(owner, workflow_id, 2).await.unwrap();
        let once = WorkflowEntity::find_by_id(workflow_id)
            .one(&db).await.unwrap().unwrap();
        assert_eq!(once.failure_count, 1);
        assert_eq!(once.status, WorkflowStatus::Active);
        assert!(once.auto_disabled_at.is_none());

        service.record_failure(owner, workflow_id, 2).await.unwrap();
        let paused = WorkflowEntity::find_by_id(workflow_id)
            .one(&db).await.unwrap().unwrap();
        assert_eq!(paused.failure_count, 2);
        assert_eq!(paused.status, WorkflowStatus::Paused);
        assert!(paused.auto_disabled_at.is_some());

        // An overdue event cannot increment or override an operator-paused
        // workflow after the terminal transition has occurred.
        service.record_failure(owner, workflow_id, 2).await.unwrap();
        let after = WorkflowEntity::find_by_id(workflow_id)
            .one(&db).await.unwrap().unwrap();
        assert_eq!(after.failure_count, 2);
        assert_eq!(after.auto_disabled_at, paused.auto_disabled_at);
    }

    #[tokio::test]
    async fn active_cron_requires_valid_expression_and_failed_edits_are_atomic() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for migration in crate::migrations::migrations() {
            migration.up(&SchemaManager::new(&db)).await.unwrap();
        }
        let service = WorkflowService::new(db.clone());
        let tenant_id = Uuid::new_v4();
        let workflow_id = service.create(
            tenant_id, None,
            CreateWorkflowInput {
                name: "Draft schedule".to_owned(),
                description: None,
                trigger_config: serde_json::json!({
                    "type": "cron",
                    "expression": "not a cron"
                }),
                webhook_slug: None,
                webhook_secret: None,
            },
        ).await.unwrap();

        assert!(matches!(
            service.update(
                tenant_id, workflow_id, None,
                UpdateWorkflowInput {
                    status: Some(WorkflowStatus::Active),
                    ..Default::default()
                },
            ).await,
            Err(WorkflowError::InvalidTriggerConfig(_)),
        ));
        let record = WorkflowEntity::find_by_id(workflow_id)
            .one(&db).await.unwrap().unwrap();
        assert_eq!(record.status, WorkflowStatus::Draft);

        let valid = serde_json::json!({
            "type": "cron", "expression": "0 * * * * *"
        });
        service.update(
            tenant_id, workflow_id, None,
            UpdateWorkflowInput {
                status: Some(WorkflowStatus::Active),
                trigger_config: Some(valid.clone()),
                ..Default::default()
            },
        ).await.unwrap();

        assert!(matches!(
            service.update(
                tenant_id, workflow_id, None,
                UpdateWorkflowInput {
                    trigger_config: Some(serde_json::json!({
                        "type": "cron", "expression": ""
                    })),
                    ..Default::default()
                },
            ).await,
            Err(WorkflowError::InvalidTriggerConfig(_)),
        ));
        let record = WorkflowEntity::find_by_id(workflow_id)
            .one(&db).await.unwrap().unwrap();
        assert_eq!(record.status, WorkflowStatus::Active);
        assert_eq!(record.trigger_config, valid,
            "invalid active edits may not partially overwrite the trigger");
    }

    #[tokio::test]
    async fn deletion_preserves_running_and_pending_execution_evidence() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for migration in crate::migrations::migrations() {
            migration.up(&SchemaManager::new(&db)).await.unwrap();
        }
        let tenant_id = Uuid::new_v4();
        let workflow_id = Uuid::new_v4();
        let now = Utc::now();
        workflow::ActiveModel {
            id: Set(workflow_id),
            tenant_id: Set(tenant_id),
            name: Set("Do not delete unfinished work".into()),
            description: Set(None),
            status: Set(WorkflowStatus::Active),
            trigger_config: Set(serde_json::json!({
                "type":"cron","expression":"0 * * * * *"
            })),
            created_by: Set(None),
            created_at: Set((now - ChronoDuration::minutes(2)).fixed_offset()),
            updated_at: Set((now - ChronoDuration::minutes(2)).fixed_offset()),
            failure_count: Set(0),
            auto_disabled_at: Set(None),
            webhook_slug: Set(None),
            webhook_secret: Set(None),
        }.insert(&db).await.unwrap();
        let execution_id = Uuid::new_v4();
        workflow_execution::ActiveModel {
            id: Set(execution_id),
            workflow_id: Set(workflow_id),
            tenant_id: Set(tenant_id),
            trigger_event_id: Set(None),
            status: Set(ExecutionStatus::Running),
            context: Set(serde_json::json!({})),
            error: Set(None),
            started_at: Set(now.fixed_offset()),
            completed_at: Set(None),
        }.insert(&db).await.unwrap();
        let wf = WorkflowEntity::find_by_id(workflow_id)
            .one(&db).await.unwrap().unwrap();
        crate::services::cron_durable::enqueue_due_slots(
            &db, &wf, "0 * * * * *",
            &"0 * * * * *".parse().unwrap(), now,
        ).await.unwrap();

        let service = WorkflowService::new(db.clone());
        assert!(matches!(
            service.delete(tenant_id, workflow_id).await,
            Err(WorkflowError::WorkflowDeletionBlocked(id)) if id == workflow_id
        ));
        assert!(WorkflowEntity::find_by_id(workflow_id)
            .one(&db).await.unwrap().is_some());

        workflow_execution::Entity::update_many()
            .col_expr(
                workflow_execution::Column::Status,
                Expr::value(ExecutionStatus::Completed.to_string()),
            )
            .filter(workflow_execution::Column::Id.eq(execution_id))
            .exec(&db).await.unwrap();
        assert!(matches!(
            service.delete(tenant_id, workflow_id).await,
            Err(WorkflowError::WorkflowDeletionBlocked(_)),
        ), "queued cron slots independently block destructive deletion");

        workflow_cron_firing::Entity::update_many()
            .col_expr(
                workflow_cron_firing::Column::Status,
                Expr::value(workflow_cron_firing::SKIPPED),
            )
            .filter(workflow_cron_firing::Column::WorkflowId.eq(workflow_id))
            .exec(&db).await.unwrap();
        service.delete(tenant_id, workflow_id).await.unwrap();
        assert!(WorkflowEntity::find_by_id(workflow_id)
            .one(&db).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn mutating_step_graph_advances_owning_workflow_revision() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for migration in crate::migrations::migrations() {
            migration.up(&SchemaManager::new(&db)).await.unwrap();
        }
        let tenant_id = Uuid::new_v4();
        let workflow_id = Uuid::new_v4();
        let baseline = Utc::now() - ChronoDuration::days(2);
        workflow::ActiveModel {
            id: Set(workflow_id),
            tenant_id: Set(tenant_id),
            name: Set("Versioned schedule".to_owned()),
            description: Set(None),
            status: Set(WorkflowStatus::Active),
            trigger_config: Set(serde_json::json!({
                "type":"cron","expression":"0 * * * * *"
            })),
            created_by: Set(None),
            created_at: Set(baseline.fixed_offset()),
            updated_at: Set(baseline.fixed_offset()),
            failure_count: Set(0),
            auto_disabled_at: Set(None),
            webhook_slug: Set(None),
            webhook_secret: Set(None),
        }.insert(&db).await.unwrap();

        let before = WorkflowEntity::find_by_id(workflow_id)
            .one(&db).await.unwrap().unwrap();
        let before_revision = crate::services::cron_durable::schedule_revision(
            &before, "0 * * * * *",
        );
        WorkflowService::new(db.clone()).add_step(
            tenant_id, workflow_id,
            CreateWorkflowStepInput {
                position: 0,
                step_type: StepType::Action,
                config: serde_json::json!({"action":"noop"}),
                on_error: OnError::Stop,
                timeout_ms: None,
            }
        ).await.unwrap();
        let after = WorkflowEntity::find_by_id(workflow_id)
            .one(&db).await.unwrap().unwrap();
        assert!(after.updated_at > before.updated_at);
        assert_ne!(before_revision, crate::services::cron_durable::schedule_revision(
            &after, "0 * * * * *",
        ));
    }
}
