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
    /// Latest host-side HTTP installer job projection, if one exists.
    pub job_id: Option<Uuid>,
    pub job_status: Option<InstallJobState>,
}

impl InstallStatusResponse {
    pub fn blocks_apply(&self) -> bool {
        self.completed || self.job_status.is_some_and(InstallJobState::blocks_apply)
    }
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

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallJobState {
    Running,
    Succeeded,
    Failed,
    RecoveryRequired,
    Unknown,
}

impl<'de> Deserialize<'de> for InstallJobState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "recovery_required" => Self::RecoveryRequired,
            _ => Self::Unknown,
        })
    }
}

impl InstallJobState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::RecoveryRequired => "recovery_required",
            Self::Unknown => "unknown",
        }
    }

    /// Unresolved jobs must keep Apply disabled. Unknown future states fail
    /// closed because the server may have added another blocking state.
    pub fn blocks_apply(self) -> bool {
        matches!(self, Self::Running | Self::RecoveryRequired | Self::Unknown)
    }

    pub fn should_poll(self) -> bool {
        matches!(self, Self::Running)
    }

    pub fn should_load_receipts(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::RecoveryRequired)
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_required_status_projection_deserializes_and_blocks_apply() {
        let job_id = Uuid::new_v4();
        let response: InstallStatusResponse = serde_json::from_value(serde_json::json!({
            "status": "recovery_required",
            "initialized": true,
            "completed": false,
            "session_id": job_id,
            "tenant_id": null,
            "lock_owner": null,
            "lock_expires_at": null,
            "completed_at": null,
            "job_id": job_id,
            "job_status": "recovery_required"
        }))
        .expect("server status projection should deserialize");

        assert_eq!(response.job_id, Some(job_id));
        assert_eq!(response.job_status, Some(InstallJobState::RecoveryRequired));
        assert!(response.blocks_apply());
    }

    #[test]
    fn installer_job_state_policy_distinguishes_running_and_terminal_states() {
        assert!(InstallJobState::Running.blocks_apply());
        assert!(InstallJobState::Running.should_poll());
        assert!(!InstallJobState::Running.should_load_receipts());

        assert!(InstallJobState::RecoveryRequired.blocks_apply());
        assert!(!InstallJobState::RecoveryRequired.should_poll());
        assert!(InstallJobState::RecoveryRequired.should_load_receipts());

        for state in [InstallJobState::Succeeded, InstallJobState::Failed] {
            assert!(!state.blocks_apply());
            assert!(!state.should_poll());
            assert!(state.should_load_receipts());
        }
    }

    #[test]
    fn unknown_server_job_states_fail_closed() {
        let state: InstallJobState =
            serde_json::from_str("\"future_server_state\"").expect("unknown states are retained");
        assert_eq!(state, InstallJobState::Unknown);
        assert!(state.blocks_apply());
        assert!(!state.should_poll());
    }
}
