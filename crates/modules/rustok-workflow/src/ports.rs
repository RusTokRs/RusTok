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
        WorkflowError::WebhookSignatureMissing => PortError::new(
            PortErrorKind::Forbidden,
            "workflow.webhook_signature_missing",
            "webhook signature is missing",
            false,
        ),
        WorkflowError::WebhookSignatureInvalid => PortError::new(
            PortErrorKind::Forbidden,
            "workflow.webhook_signature_invalid",
            "webhook signature is invalid",
            false,
        ),
        WorkflowError::WebhookSecretNotConfigured => PortError::new(
            PortErrorKind::Unavailable,
            "workflow.webhook_secret_not_configured",
            "webhook secret is not configured",
            false,
        ),
        WorkflowError::VersionNotFound(workflow_id, version) => PortError::new(
            PortErrorKind::NotFound,
            "workflow.version_not_found",
            format!("workflow version not found: workflow_id={workflow_id}, version={version}"),
            false,
        ),
        WorkflowError::TemplateNotFound(template_id) => PortError::new(
            PortErrorKind::NotFound,
            "workflow.template_not_found",
            format!("workflow template not found: {template_id}"),
            false,
        ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;

    fn sample_context(tenant_id: &str) -> PortContext {
        PortContext::new(tenant_id, PortActor::system(), "en", "corr-test")
    }

    #[test]
    fn test_workflow_tenant_id_valid() {
        let tenant_id = Uuid::new_v4();
        let context = sample_context(&tenant_id.to_string());
        let parsed = workflow_tenant_id(&context).expect("valid tenant UUID");
        assert_eq!(parsed, tenant_id);
    }

    #[test]
    fn test_workflow_tenant_id_invalid() {
        let context = sample_context("invalid-uuid");
        let err = workflow_tenant_id(&context).unwrap_err();
        assert_eq!(err.code, "workflow.tenant_id_invalid");
    }

    #[test]
    fn test_workflow_error_mappings() {
        let id = Uuid::new_v4();
        let err = workflow_error_to_port_error(WorkflowError::NotFound(id));
        assert_eq!(err.kind, PortErrorKind::NotFound);
        assert_eq!(err.code, "workflow.not_found");

        let err = workflow_error_to_port_error(WorkflowError::VersionNotFound(id, 2));
        assert_eq!(err.kind, PortErrorKind::NotFound);
        assert_eq!(err.code, "workflow.version_not_found");

        let err = workflow_error_to_port_error(WorkflowError::TemplateNotFound("crm".into()));
        assert_eq!(err.kind, PortErrorKind::NotFound);
        assert_eq!(err.code, "workflow.template_not_found");

        let err = workflow_error_to_port_error(WorkflowError::NotActive("draft".into()));
        assert_eq!(err.kind, PortErrorKind::Conflict);

        let err = workflow_error_to_port_error(WorkflowError::WebhookSignatureMissing);
        assert_eq!(err.kind, PortErrorKind::Forbidden);

        let err = workflow_error_to_port_error(WorkflowError::WebhookSecretNotConfigured);
        assert_eq!(err.kind, PortErrorKind::Unavailable);
    }
}

