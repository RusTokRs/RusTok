use async_graphql::{Enum, InputObject, SimpleObject};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::{ExecutionStatus, OnError, StepExecutionStatus, StepType, WorkflowStatus};
use crate::templates::WorkflowTemplate;
use crate::{
    WorkflowExecutionResponse, WorkflowResponse, WorkflowStepExecutionResponse,
    WorkflowStepResponse, WorkflowSummary, WorkflowVersionDetail, WorkflowVersionSummary,
};

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlWorkflowStatus {
    Draft,
    Active,
    Paused,
    Archived,
}

impl From<WorkflowStatus> for GqlWorkflowStatus {
    fn from(status: WorkflowStatus) -> Self {
        match status {
            WorkflowStatus::Draft => Self::Draft,
            WorkflowStatus::Active => Self::Active,
            WorkflowStatus::Paused => Self::Paused,
            WorkflowStatus::Archived => Self::Archived,
        }
    }
}

impl From<GqlWorkflowStatus> for WorkflowStatus {
    fn from(status: GqlWorkflowStatus) -> Self {
        match status {
            GqlWorkflowStatus::Draft => Self::Draft,
            GqlWorkflowStatus::Active => Self::Active,
            GqlWorkflowStatus::Paused => Self::Paused,
            GqlWorkflowStatus::Archived => Self::Archived,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlStepType {
    Action,
    Condition,
    Delay,
    AlloyScript,
    EmitEvent,
    Http,
    Notify,
    Transform,
}

impl From<StepType> for GqlStepType {
    fn from(step_type: StepType) -> Self {
        match step_type {
            StepType::Action => Self::Action,
            StepType::Condition => Self::Condition,
            StepType::Delay => Self::Delay,
            StepType::AlloyScript => Self::AlloyScript,
            StepType::EmitEvent => Self::EmitEvent,
            StepType::Http => Self::Http,
            StepType::Notify => Self::Notify,
            StepType::Transform => Self::Transform,
        }
    }
}

impl From<GqlStepType> for StepType {
    fn from(step_type: GqlStepType) -> Self {
        match step_type {
            GqlStepType::Action => Self::Action,
            GqlStepType::Condition => Self::Condition,
            GqlStepType::Delay => Self::Delay,
            GqlStepType::AlloyScript => Self::AlloyScript,
            GqlStepType::EmitEvent => Self::EmitEvent,
            GqlStepType::Http => Self::Http,
            GqlStepType::Notify => Self::Notify,
            GqlStepType::Transform => Self::Transform,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlOnError {
    Stop,
    Skip,
    Retry,
}

impl From<OnError> for GqlOnError {
    fn from(on_error: OnError) -> Self {
        match on_error {
            OnError::Stop => Self::Stop,
            OnError::Skip => Self::Skip,
            OnError::Retry => Self::Retry,
        }
    }
}

impl From<GqlOnError> for OnError {
    fn from(on_error: GqlOnError) -> Self {
        match on_error {
            GqlOnError::Stop => Self::Stop,
            GqlOnError::Skip => Self::Skip,
            GqlOnError::Retry => Self::Retry,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlExecutionStatus {
    Running,
    Completed,
    Failed,
    TimedOut,
}

impl From<ExecutionStatus> for GqlExecutionStatus {
    fn from(status: ExecutionStatus) -> Self {
        match status {
            ExecutionStatus::Running => Self::Running,
            ExecutionStatus::Completed => Self::Completed,
            ExecutionStatus::Failed => Self::Failed,
            ExecutionStatus::TimedOut => Self::TimedOut,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlStepExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl From<StepExecutionStatus> for GqlStepExecutionStatus {
    fn from(status: StepExecutionStatus) -> Self {
        match status {
            StepExecutionStatus::Pending => Self::Pending,
            StepExecutionStatus::Running => Self::Running,
            StepExecutionStatus::Completed => Self::Completed,
            StepExecutionStatus::Failed => Self::Failed,
            StepExecutionStatus::Skipped => Self::Skipped,
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlWorkflowSummary {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub status: GqlWorkflowStatus,
    pub webhook_slug: Option<String>,
    pub failure_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<WorkflowSummary> for GqlWorkflowSummary {
    fn from(workflow: WorkflowSummary) -> Self {
        Self {
            id: workflow.id,
            tenant_id: workflow.tenant_id,
            name: workflow.name,
            status: workflow.status.into(),
            webhook_slug: workflow.webhook_slug,
            failure_count: workflow.failure_count,
            created_at: workflow.created_at.to_rfc3339(),
            updated_at: workflow.updated_at.to_rfc3339(),
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlWorkflowStep {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub position: i32,
    pub step_type: GqlStepType,
    pub config: Value,
    pub on_error: GqlOnError,
    pub timeout_ms: Option<i64>,
}

impl From<WorkflowStepResponse> for GqlWorkflowStep {
    fn from(step: WorkflowStepResponse) -> Self {
        Self {
            id: step.id,
            workflow_id: step.workflow_id,
            position: step.position,
            step_type: step.step_type.into(),
            config: step.config,
            on_error: step.on_error.into(),
            timeout_ms: step.timeout_ms,
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlWorkflow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: GqlWorkflowStatus,
    pub trigger_config: Value,
    pub webhook_slug: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
    pub failure_count: i32,
    pub auto_disabled_at: Option<String>,
    pub steps: Vec<GqlWorkflowStep>,
}

impl From<WorkflowResponse> for GqlWorkflow {
    fn from(workflow: WorkflowResponse) -> Self {
        Self {
            id: workflow.id,
            tenant_id: workflow.tenant_id,
            name: workflow.name,
            description: workflow.description,
            status: workflow.status.into(),
            trigger_config: workflow.trigger_config,
            webhook_slug: workflow.webhook_slug,
            created_by: workflow.created_by,
            created_at: workflow.created_at.to_rfc3339(),
            updated_at: workflow.updated_at.to_rfc3339(),
            failure_count: workflow.failure_count,
            auto_disabled_at: workflow.auto_disabled_at.map(|value| value.to_rfc3339()),
            steps: workflow.steps.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlStepExecution {
    pub id: Uuid,
    pub execution_id: Uuid,
    pub step_id: Uuid,
    pub status: GqlStepExecutionStatus,
    pub input: Value,
    pub output: Value,
    pub error: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

impl From<WorkflowStepExecutionResponse> for GqlStepExecution {
    fn from(step: WorkflowStepExecutionResponse) -> Self {
        Self {
            id: step.id,
            execution_id: step.execution_id,
            step_id: step.step_id,
            status: step.status.into(),
            input: step.input,
            output: step.output,
            error: step.error,
            started_at: step.started_at.to_rfc3339(),
            completed_at: step.completed_at.map(|value| value.to_rfc3339()),
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlWorkflowExecution {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub tenant_id: Uuid,
    pub trigger_event_id: Option<Uuid>,
    pub status: GqlExecutionStatus,
    pub context: Value,
    pub error: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub step_executions: Vec<GqlStepExecution>,
}

impl From<WorkflowExecutionResponse> for GqlWorkflowExecution {
    fn from(execution: WorkflowExecutionResponse) -> Self {
        Self {
            id: execution.id,
            workflow_id: execution.workflow_id,
            tenant_id: execution.tenant_id,
            trigger_event_id: execution.trigger_event_id,
            status: execution.status.into(),
            context: execution.context,
            error: execution.error,
            started_at: execution.started_at.to_rfc3339(),
            completed_at: execution.completed_at.map(|value| value.to_rfc3339()),
            step_executions: execution
                .step_executions
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

#[derive(InputObject)]
pub struct GqlCreateWorkflowInput {
    pub name: String,
    pub description: Option<String>,
    pub trigger_config: Value,
    pub webhook_slug: Option<String>,
}

#[derive(InputObject)]
pub struct GqlUpdateWorkflowInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<GqlWorkflowStatus>,
    pub trigger_config: Option<Value>,
    pub webhook_slug: Option<String>,
}

#[derive(InputObject)]
pub struct GqlCreateStepInput {
    pub position: i32,
    pub step_type: GqlStepType,
    pub config: Value,
    pub on_error: GqlOnError,
    pub timeout_ms: Option<i64>,
}

#[derive(InputObject)]
pub struct GqlUpdateStepInput {
    pub position: Option<i32>,
    pub step_type: Option<GqlStepType>,
    pub config: Option<Value>,
    pub on_error: Option<GqlOnError>,
    pub timeout_ms: Option<i64>,
}

#[derive(SimpleObject)]
pub struct GqlWorkflowVersionSummary {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub version: i32,
    pub created_by: Option<Uuid>,
    pub created_at: String,
}

impl From<WorkflowVersionSummary> for GqlWorkflowVersionSummary {
    fn from(version: WorkflowVersionSummary) -> Self {
        Self {
            id: version.id,
            workflow_id: version.workflow_id,
            version: version.version,
            created_by: version.created_by,
            created_at: version.created_at.to_rfc3339(),
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlWorkflowVersionDetail {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub version: i32,
    pub snapshot: Value,
    pub created_by: Option<Uuid>,
    pub created_at: String,
}

impl From<WorkflowVersionDetail> for GqlWorkflowVersionDetail {
    fn from(version: WorkflowVersionDetail) -> Self {
        Self {
            id: version.id,
            workflow_id: version.workflow_id,
            version: version.version,
            snapshot: version.snapshot,
            created_by: version.created_by,
            created_at: version.created_at.to_rfc3339(),
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlWorkflowTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub trigger_config: Value,
}

impl From<&WorkflowTemplate> for GqlWorkflowTemplate {
    fn from(template: &WorkflowTemplate) -> Self {
        Self {
            id: template.id.to_string(),
            name: template.name.to_string(),
            description: template.description.to_string(),
            category: template.category.to_string(),
            trigger_config: template.trigger_config.clone(),
        }
    }
}
