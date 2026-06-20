use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use uuid::Uuid;

use crate::{WorkflowError, WorkflowResponse, WorkflowService, WorkflowSummary};

/// Transport-neutral owner boundary for workflow read projections.
#[async_trait]
pub trait WorkflowReadPort: Send + Sync {
    async fn list_workflows(&self, context: PortContext)
    -> Result<Vec<WorkflowSummary>, PortError>;

    async fn get_workflow(
        &self,
        context: PortContext,
        workflow_id: Uuid,
    ) -> Result<WorkflowResponse, PortError>;
}

#[async_trait]
impl WorkflowReadPort for WorkflowService {
    async fn list_workflows(
        &self,
        context: PortContext,
    ) -> Result<Vec<WorkflowSummary>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = workflow_tenant_id(&context)?;
        self.list(tenant_id)
            .await
            .map_err(workflow_error_to_port_error)
    }

    async fn get_workflow(
        &self,
        context: PortContext,
        workflow_id: Uuid,
    ) -> Result<WorkflowResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = workflow_tenant_id(&context)?;
        self.get(tenant_id, workflow_id)
            .await
            .map_err(workflow_error_to_port_error)
    }
}

fn workflow_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "workflow.tenant_id_invalid",
            "workflow read port requires a UUID tenant id",
        )
    })
}

fn workflow_error_to_port_error(error: WorkflowError) -> PortError {
    match error {
        WorkflowError::NotFound(id) => PortError::new(
            PortErrorKind::NotFound,
            "workflow.not_found",
            format!("workflow not found: {id}"),
            false,
        ),
        WorkflowError::StepNotFound(id) => PortError::new(
            PortErrorKind::NotFound,
            "workflow.step_not_found",
            format!("workflow step not found: {id}"),
            false,
        ),
        WorkflowError::ExecutionNotFound(id) => PortError::new(
            PortErrorKind::NotFound,
            "workflow.execution_not_found",
            format!("workflow execution not found: {id}"),
            false,
        ),
        WorkflowError::NotActive(status) => PortError::new(
            PortErrorKind::Conflict,
            "workflow.not_active",
            format!("workflow is not active: {status}"),
            false,
        ),
        WorkflowError::StepFailed(message)
        | WorkflowError::UnknownStepType(message)
        | WorkflowError::InvalidTriggerConfig(message)
        | WorkflowError::InvalidStepConfig(message) => {
            PortError::validation("workflow.validation", message)
        }
        WorkflowError::Database(message) => PortError::unavailable(
            "workflow.database_unavailable",
            format!("workflow database error: {message}"),
        ),
        WorkflowError::Serialization(message) => PortError::new(
            PortErrorKind::InvariantViolation,
            "workflow.serialization_failed",
            format!("workflow serialization error: {message}"),
            false,
        ),
    }
}
