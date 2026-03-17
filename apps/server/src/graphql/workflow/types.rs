use async_graphql::{Enum, InputObject, SimpleObject};
use serde_json::Value;
use uuid::Uuid;

use rustok_workflow::entities::{ExecutionStatus, OnError, StepExecutionStatus, StepType, WorkflowStatus};
use rustok_workflow::{
    WorkflowExecutionResponse, WorkflowResponse, WorkflowStepExecutionResponse,
    WorkflowStepResponse, WorkflowSummary,
};
use rustok_workflow::{WorkflowVersionDetail, WorkflowVersionSummary};
use rustok_workflow::templates::WorkflowTemplate;

// ── Enum mirrors ───────────────────────────────────────────────────────────────

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlWorkflowStatus {
    Draft,
    Active,
    Paused,
    Archived,
}

impl From<WorkflowStatus> for GqlWorkflowStatus {
    fn from(s: WorkflowStatus) -> Self {
        match s {
            WorkflowStatus::Draft => Self::Draft,
            WorkflowStatus::Active => Self::Active,
            WorkflowStatus::Paused => Self::Paused,
            WorkflowStatus::Archived => Self::Archived,
        }
    }
}

impl From<GqlWorkflowStatus> for WorkflowStatus {
    fn from(s: GqlWorkflowStatus) -> Self {
        match s {
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
    fn from(t: StepType) -> Self {
        match t {
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
    fn from(t: GqlStepType) -> Self {
        match t {
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
    fn from(o: OnError) -> Self {
        match o {
            OnError::Stop => Self::Stop,
            OnError::Skip => Self::Skip,
            OnError::Retry => Self::Retry,
        }
    }
}

impl From<GqlOnError> for OnError {
    fn from(o: GqlOnError) -> Self {
        match o {
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
    fn from(s: ExecutionStatus) -> Self {
        match s {
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
    fn from(s: StepExecutionStatus) -> Self {
        match s {
            StepExecutionStatus::Pending => Self::Pending,
            StepExecutionStatus::Running => Self::Running,
            StepExecutionStatus::Completed => Self::Completed,
            StepExecutionStatus::Failed => Self::Failed,
            StepExecutionStatus::Skipped => Self::Skipped,
        }
    }
}

// ── Output types ───────────────────────────────────────────────────────────────

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
    fn from(w: WorkflowSummary) -> Self {
        Self {
            id: w.id,
            tenant_id: w.tenant_id,
            name: w.name,
            status: w.status.into(),
            webhook_slug: w.webhook_slug,
            failure_count: w.failure_count,
            created_at: w.created_at.to_rfc3339(),
            updated_at: w.updated_at.to_rfc3339(),
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
    fn from(s: WorkflowStepResponse) -> Self {
        Self {
            id: s.id,
            workflow_id: s.workflow_id,
            position: s.position,
            step_type: s.step_type.into(),
            config: s.config,
            on_error: s.on_error.into(),
            timeout_ms: s.timeout_ms,
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
    fn from(w: WorkflowResponse) -> Self {
        Self {
            id: w.id,
            tenant_id: w.tenant_id,
            name: w.name,
            description: w.description,
            status: w.status.into(),
            trigger_config: w.trigger_config,
            webhook_slug: w.webhook_slug,
            created_by: w.created_by,
            created_at: w.created_at.to_rfc3339(),
            updated_at: w.updated_at.to_rfc3339(),
            failure_count: w.failure_count,
            auto_disabled_at: w.auto_disabled_at.map(|d| d.to_rfc3339()),
            steps: w.steps.into_iter().map(Into::into).collect(),
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
    fn from(s: WorkflowStepExecutionResponse) -> Self {
        Self {
            id: s.id,
            execution_id: s.execution_id,
            step_id: s.step_id,
            status: s.status.into(),
            input: s.input,
            output: s.output,
            error: s.error,
            started_at: s.started_at.to_rfc3339(),
            completed_at: s.completed_at.map(|d| d.to_rfc3339()),
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
    fn from(e: WorkflowExecutionResponse) -> Self {
        Self {
            id: e.id,
            workflow_id: e.workflow_id,
            tenant_id: e.tenant_id,
            trigger_event_id: e.trigger_event_id,
            status: e.status.into(),
            context: e.context,
            error: e.error,
            started_at: e.started_at.to_rfc3339(),
            completed_at: e.completed_at.map(|d| d.to_rfc3339()),
            step_executions: e.step_executions.into_iter().map(Into::into).collect(),
        }
    }
}

// ── Input types ────────────────────────────────────────────────────────────────

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

// ── Phase 4: Version types ──────────────────────────────────────────────────────

#[derive(SimpleObject)]
pub struct GqlWorkflowVersionSummary {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub version: i32,
    pub created_by: Option<Uuid>,
    pub created_at: String,
}

impl From<WorkflowVersionSummary> for GqlWorkflowVersionSummary {
    fn from(v: WorkflowVersionSummary) -> Self {
        Self {
            id: v.id,
            workflow_id: v.workflow_id,
            version: v.version,
            created_by: v.created_by,
            created_at: v.created_at.to_rfc3339(),
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
    fn from(v: WorkflowVersionDetail) -> Self {
        Self {
            id: v.id,
            workflow_id: v.workflow_id,
            version: v.version,
            snapshot: v.snapshot,
            created_by: v.created_by,
            created_at: v.created_at.to_rfc3339(),
        }
    }
}

// ── Phase 4: Template types ─────────────────────────────────────────────────────

#[derive(SimpleObject)]
pub struct GqlWorkflowTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub trigger_config: Value,
}

impl From<&WorkflowTemplate> for GqlWorkflowTemplate {
    fn from(t: &WorkflowTemplate) -> Self {
        Self {
            id: t.id.to_string(),
            name: t.name.to_string(),
            description: t.description.to_string(),
            category: t.category.to_string(),
            trigger_config: t.trigger_config.clone(),
        }
    }
}
