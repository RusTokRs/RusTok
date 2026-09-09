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
    pub rewrite_status: String,
    pub current_milestone: String,
    pub engine_type: String,
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
    pub owner_module: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IndexInboxMetricsSnapshot {
    pub total_messages: u64,
    pub pending_messages: u64,
    pub processing_messages: u64,
    pub completed_messages: u64,
    pub failed_messages: u64,
    pub dead_letter_messages: u64,
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
    pub catalog_status: String,
    pub admission_rules_count: usize,
    pub link_availability_rules_count: usize,
    pub partition_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexOperationsSnapshot {
    pub replay_runner: String,
    pub reconciliation_mode: String,
    pub drift_repair: String,
    pub registered_sources: Vec<IndexSourceDescriptorSnapshot>,
    pub inbox_metrics: IndexInboxMetricsSnapshot,
    pub job_metrics: IndexJobMetricsSnapshot,
    pub query_diagnostics: IndexQueryDiagnosticsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexSourceDescriptorSnapshot {
    pub name: String,
    pub entity: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TriggerReplayInput {
    pub schema_module: String,
    pub schema_entity: String,
    pub schema_version: u32,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayActionResult {
    pub success: bool,
    pub job_id: Option<String>,
    pub status: String,
    pub pages_processed: u64,
    pub mutations_applied: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CancelJobInput {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CancelActionResult {
    pub success: bool,
    pub job_id: String,
    pub outcome: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetryJobInput {
    pub job_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetryActionResult {
    pub success: bool,
    pub job_id: String,
    pub outcome: String,
    pub retry_epoch: Option<u32>,
    pub message: String,
}
