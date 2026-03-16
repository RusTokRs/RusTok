use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryFilter, QueryOrder,
    Set,
};
use uuid::Uuid;

use crate::dto::{
    CreateWorkflowInput, CreateWorkflowStepInput, UpdateWorkflowInput, UpdateWorkflowStepInput,
    WorkflowResponse, WorkflowStepResponse, WorkflowSummary,
};
use crate::entities::{
    workflow, workflow_step, WorkflowActiveModel, WorkflowEntity, WorkflowStatus,
    WorkflowStepActiveModel, WorkflowStepEntity,
};
use crate::error::{WorkflowError, WorkflowResult};

pub struct WorkflowService {
    db: DatabaseConnection,
}

impl WorkflowService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    // ── Workflows ──────────────────────────────────────────────────────────────

    pub async fn create(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: CreateWorkflowInput,
    ) -> WorkflowResult<Uuid> {
        let now = Utc::now().fixed_offset();
        let id = Uuid::new_v4();

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
        };
        model.insert(&self.db).await?;

        Ok(id)
    }

    pub async fn get(&self, tenant_id: Uuid, id: Uuid) -> WorkflowResult<WorkflowResponse> {
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
            created_by: workflow.created_by,
            created_at: workflow.created_at.into(),
            updated_at: workflow.updated_at.into(),
            steps: steps.into_iter().map(step_to_response).collect(),
        })
    }

    pub async fn list(&self, tenant_id: Uuid) -> WorkflowResult<Vec<WorkflowSummary>> {
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
                created_at: w.created_at.into(),
                updated_at: w.updated_at.into(),
            })
            .collect())
    }

    pub async fn update(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateWorkflowInput,
    ) -> WorkflowResult<()> {
        let existing = WorkflowEntity::find_by_id(id)
            .filter(workflow::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(WorkflowError::NotFound(id))?;

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
        model.updated_at = Set(Utc::now().fixed_offset());
        model.update(&self.db).await?;

        Ok(())
    }

    pub async fn delete(&self, tenant_id: Uuid, id: Uuid) -> WorkflowResult<()> {
        let existing = WorkflowEntity::find_by_id(id)
            .filter(workflow::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(WorkflowError::NotFound(id))?;

        let model: WorkflowActiveModel = existing.into();
        model.delete(&self.db).await?;

        Ok(())
    }

    // ── Steps ──────────────────────────────────────────────────────────────────

    pub async fn add_step(
        &self,
        tenant_id: Uuid,
        workflow_id: Uuid,
        input: CreateWorkflowStepInput,
    ) -> WorkflowResult<Uuid> {
        // Verify workflow belongs to tenant
        WorkflowEntity::find_by_id(workflow_id)
            .filter(workflow::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(WorkflowError::NotFound(workflow_id))?;

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
        model.insert(&self.db).await?;

        Ok(step_id)
    }

    pub async fn update_step(
        &self,
        tenant_id: Uuid,
        workflow_id: Uuid,
        step_id: Uuid,
        input: UpdateWorkflowStepInput,
    ) -> WorkflowResult<()> {
        // Verify ownership
        WorkflowEntity::find_by_id(workflow_id)
            .filter(workflow::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(WorkflowError::NotFound(workflow_id))?;

        let existing = WorkflowStepEntity::find_by_id(step_id)
            .filter(workflow_step::Column::WorkflowId.eq(workflow_id))
            .one(&self.db)
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
        model.update(&self.db).await?;

        Ok(())
    }

    pub async fn delete_step(
        &self,
        tenant_id: Uuid,
        workflow_id: Uuid,
        step_id: Uuid,
    ) -> WorkflowResult<()> {
        // Verify ownership
        WorkflowEntity::find_by_id(workflow_id)
            .filter(workflow::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(WorkflowError::NotFound(workflow_id))?;

        let existing = WorkflowStepEntity::find_by_id(step_id)
            .filter(workflow_step::Column::WorkflowId.eq(workflow_id))
            .one(&self.db)
            .await?
            .ok_or(WorkflowError::StepNotFound(step_id))?;

        let model: WorkflowStepActiveModel = existing.into();
        model.delete(&self.db).await?;

        Ok(())
    }

    /// Load ordered steps for a workflow (used by the engine).
    pub async fn load_steps(
        &self,
        workflow_id: Uuid,
    ) -> WorkflowResult<Vec<crate::entities::WorkflowStep>> {
        let steps = WorkflowStepEntity::find()
            .filter(workflow_step::Column::WorkflowId.eq(workflow_id))
            .order_by(workflow_step::Column::Position, Order::Asc)
            .all(&self.db)
            .await?;
        Ok(steps)
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
