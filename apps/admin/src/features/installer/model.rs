use chrono::{DateTime, Utc};
use rustok_installer::{InstallPlan, PreflightReport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallStatusResponse {
    pub status: String,
    pub initialized: bool,
    pub completed: bool,
    pub session_id: Option<Uuid>,
    pub tenant_id: Option<Uuid>,
    pub lock_owner: Option<String>,
    pub lock_expires_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallPreflightResponse {
    pub passed: bool,
    pub report: PreflightReport,
    pub redacted_plan: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct InstallApplyRequest {
    pub plan: InstallPlan,
    pub lock_owner: Option<String>,
    pub lock_ttl_secs: Option<i64>,
    pub pg_admin_url: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallJobState {
    Running,
    Succeeded,
    Failed,
}

impl InstallJobState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallApplyJobResponse {
    pub job_id: Uuid,
    pub status: InstallJobState,
    pub submitted_at: DateTime<Utc>,
    pub status_url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallJobStatusResponse {
    pub job_id: Uuid,
    pub status: InstallJobState,
    pub submitted_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub session_id: Option<Uuid>,
    pub tenant_id: Option<Uuid>,
    pub output: Option<Value>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallReceiptsResponse {
    pub session_id: Uuid,
    pub receipts: Vec<InstallReceiptRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallReceiptRow {
    pub id: Uuid,
    pub session_id: Uuid,
    pub step: String,
    pub outcome: String,
    pub input_checksum: String,
    pub diagnostics: Value,
    pub installer_version: String,
    pub created_at: DateTime<Utc>,
}
