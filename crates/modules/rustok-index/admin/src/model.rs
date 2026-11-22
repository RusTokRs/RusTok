use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexAdminBootstrap {
    pub tenant: IndexTenantSnapshot,
    pub module: IndexModuleSnapshot,
    pub storage: IndexStorageSnapshot,
    pub schemas: Vec<IndexSchemaSnapshot>,
    pub operations: IndexOperationsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexTenantSnapshot {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub default_locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexModuleSnapshot {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexStorageSnapshot {
    pub backend: String,
    pub layout: String,
    pub tables: Vec<IndexTableSnapshot>,
    pub partition_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexTableSnapshot {
    pub name: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexSchemaSnapshot {
    pub module: String,
    pub entity: String,
    pub version: u32,
    pub fingerprint: String,
    pub field_count: usize,
    pub link_count: usize,
    pub owner_module: Option<String>,
    pub readiness: SchemaReadiness,
    pub source_selected: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SchemaReadiness {
    Ready,
    Missing,
    Inactive,
    Unselected,
    ContractMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IndexInboxMetricsSnapshot {
    pub total_messages: u64,
    pub pending_messages: u64,
    pub processing_messages: u64,
    pub applied_messages: u64,
    pub rejected_messages: u64,
    pub oldest_pending_age_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IndexJobMetricsSnapshot {
    pub total_jobs: u64,
    pub pending_jobs: u64,
    pub running_jobs: u64,
    pub succeeded_jobs: u64,
    pub failed_jobs: u64,
    pub cancelled_jobs: u64,
    pub retry_recovery_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IndexQueryDiagnosticsSnapshot {
    pub admission_rules_count: usize,
    pub link_availability_rules_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexOperationsSnapshot {
    pub replay_available: bool,
    pub can_manage: bool,
    pub registered_sources: Option<Vec<IndexSourceDescriptorSnapshot>>,
    pub inbox_metrics: IndexInboxMetricsSnapshot,
    pub job_metrics: IndexJobMetricsSnapshot,
    pub query_diagnostics: Option<IndexQueryDiagnosticsSnapshot>,
    pub recent_jobs: Vec<IndexJobSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexSourceDescriptorSnapshot {
    pub name: String,
    pub entity: String,
    pub owner_module: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexJobSnapshot {
    pub job_id: String,
    pub kind: String,
    pub state: String,
    pub schema: String,
    pub locale: Option<String>,
    pub failure_count: u32,
    pub cancel_requested: bool,
    pub available_at: Option<String>,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TriggerReplayInput {
    pub schema_module: String,
    pub schema_entity: String,
    pub schema_version: u32,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayActionResult {
    pub job_id: Option<String>,
    pub status: ReplayActionStatus,
    pub retry_after_seconds: Option<u64>,
    pub pages_processed: u64,
    pub mutations_applied: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReplayActionStatus {
    Busy,
    AlreadyComplete,
    Complete,
    Cancelled,
    Yielded,
    RetryScheduled,
    FailedPermanent,
    FailedExhausted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CancelJobInput {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CancelActionResult {
    pub job_id: String,
    pub outcome: CancelActionStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CancelActionStatus {
    Requested,
    Cancelled,
    AlreadySucceeded,
    AlreadyFailed,
    AlreadyCancelled,
    NotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RetryJobInput {
    pub job_id: String,
    pub kind: RetryJobKind,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetryJobKind {
    Replay,
    Reconciliation,
}

impl RetryJobKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Replay => "replay",
            Self::Reconciliation => "reconciliation",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetryActionResult {
    pub job_id: String,
    pub outcome: RetryActionStatus,
    pub retry_epoch: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RetryActionStatus {
    Requeued,
    NotFound,
    NotFailed,
    ScopeUnavailable,
}
