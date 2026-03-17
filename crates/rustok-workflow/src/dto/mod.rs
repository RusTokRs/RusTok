use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{ExecutionStatus, OnError, StepExecutionStatus, StepType, WorkflowStatus};

// ── Workflow DTOs ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkflowInput {
    pub name: String,
    pub description: Option<String>,
    pub trigger_config: serde_json::Value,
    /// Optional unique webhook slug for this workflow
    pub webhook_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateWorkflowInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<WorkflowStatus>,
    pub trigger_config: Option<serde_json::Value>,
    /// Set to Some("") to clear the webhook slug, or Some("slug") to set it
    pub webhook_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: WorkflowStatus,
    pub trigger_config: serde_json::Value,
    pub webhook_slug: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub failure_count: i32,
    pub auto_disabled_at: Option<DateTime<Utc>>,
    pub steps: Vec<WorkflowStepResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSummary {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub status: WorkflowStatus,
    pub webhook_slug: Option<String>,
    pub failure_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Version DTOs ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVersionSummary {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub version: i32,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVersionDetail {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub version: i32,
    pub snapshot: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

// ── WorkflowStep DTOs ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkflowStepInput {
    pub position: i32,
    pub step_type: StepType,
    pub config: serde_json::Value,
    pub on_error: OnError,
    pub timeout_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWorkflowStepInput {
    pub position: Option<i32>,
    pub step_type: Option<StepType>,
    pub config: Option<serde_json::Value>,
    pub on_error: Option<OnError>,
    pub timeout_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStepResponse {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub position: i32,
    pub step_type: StepType,
    pub config: serde_json::Value,
    pub on_error: OnError,
    pub timeout_ms: Option<i64>,
}

// ── Execution DTOs ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionResponse {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub tenant_id: Uuid,
    pub trigger_event_id: Option<Uuid>,
    pub status: ExecutionStatus,
    pub context: serde_json::Value,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub step_executions: Vec<WorkflowStepExecutionResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStepExecutionResponse {
    pub id: Uuid,
    pub execution_id: Uuid,
    pub step_id: Uuid,
    pub status: StepExecutionStatus,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

// ── Trigger config helpers ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerConfig {
    /// Triggered by a matching DomainEvent. Supports wildcard suffix: "blog.*"
    Event { event_type: String },
    /// Triggered on a cron schedule (6-field: sec min hour dom month dow)
    Cron { expression: String },
    /// Triggered manually via API or admin UI
    Manual,
    /// Triggered by an incoming webhook (Phase 4)
    Webhook { path: String },
    /// Triggered by an Alloy script calling workflow.trigger()
    Alloy { workflow_id: String },
}
