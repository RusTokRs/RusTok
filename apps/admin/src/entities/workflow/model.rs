use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkflowStatus {
    Draft,
    Active,
    Paused,
    Archived,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for WorkflowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "DRAFT"),
            Self::Active => write!(f, "ACTIVE"),
            Self::Paused => write!(f, "PAUSED"),
            Self::Archived => write!(f, "ARCHIVED"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StepType {
    Action,
    Condition,
    Delay,
    AlloyScript,
    EmitEvent,
    Http,
    Notify,
    Transform,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for StepType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Action => "ACTION",
            Self::Condition => "CONDITION",
            Self::Delay => "DELAY",
            Self::AlloyScript => "ALLOY_SCRIPT",
            Self::EmitEvent => "EMIT_EVENT",
            Self::Http => "HTTP",
            Self::Notify => "NOTIFY",
            Self::Transform => "TRANSFORM",
            Self::Unknown => "UNKNOWN",
        };
        write!(f, "{}", s)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OnError {
    Stop,
    Skip,
    Retry,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for OnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stop => write!(f, "STOP"),
            Self::Skip => write!(f, "SKIP"),
            Self::Retry => write!(f, "RETRY"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionStatus {
    Running,
    Completed,
    Failed,
    TimedOut,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "RUNNING"),
            Self::Completed => write!(f, "COMPLETED"),
            Self::Failed => write!(f, "FAILED"),
            Self::TimedOut => write!(f, "TIMED_OUT"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkflowSummary {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub name: String,
    pub status: WorkflowStatus,
    #[serde(rename = "failureCount")]
    pub failure_count: i32,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkflowStep {
    pub id: String,
    #[serde(rename = "workflowId")]
    pub workflow_id: String,
    pub position: i32,
    #[serde(rename = "stepType")]
    pub step_type: StepType,
    pub config: serde_json::Value,
    #[serde(rename = "onError")]
    pub on_error: OnError,
    #[serde(rename = "timeoutMs")]
    pub timeout_ms: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkflowDetail {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: WorkflowStatus,
    #[serde(rename = "triggerConfig")]
    pub trigger_config: serde_json::Value,
    #[serde(rename = "createdBy")]
    pub created_by: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "failureCount")]
    pub failure_count: i32,
    #[serde(rename = "autoDisabledAt")]
    pub auto_disabled_at: Option<String>,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkflowExecution {
    pub id: String,
    #[serde(rename = "workflowId")]
    pub workflow_id: String,
    pub status: ExecutionStatus,
    pub error: Option<String>,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "completedAt")]
    pub completed_at: Option<String>,
    #[serde(rename = "stepExecutions")]
    pub step_executions: Vec<StepExecution>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StepExecution {
    pub id: String,
    #[serde(rename = "stepId")]
    pub step_id: String,
    pub status: String,
    pub error: Option<String>,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "completedAt")]
    pub completed_at: Option<String>,
}
