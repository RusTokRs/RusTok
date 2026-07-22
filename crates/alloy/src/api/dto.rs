use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    context::ExecutionPhase,
    execution_log::ExecutionLogEntry,
    model::{
        AlloyWorkspace, ReviewDecision, ReviewStatus, Script, ScriptId, ScriptStatus,
        ScriptTrigger, TestRun, TestRunStatus,
    },
};

// ============ Requests ============

#[derive(Debug, Deserialize)]
pub struct CreateScriptRequest {
    pub name: String,
    pub description: Option<String>,
    pub workspace: AlloyWorkspace,
    pub trigger: ScriptTrigger,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub run_as_system: bool,
    pub tenant_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateScriptRequest {
    pub expected_version: u32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub workspace: Option<AlloyWorkspace>,
    pub trigger: Option<ScriptTrigger>,
    pub status: Option<ScriptStatus>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct RunScriptRequest {
    pub expected_version: u32,
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
    pub entity: Option<EntityInput>,
}

#[derive(Debug, Deserialize)]
pub struct ScriptRevisionRequest {
    pub expected_version: u32,
}

#[derive(Debug, Deserialize)]
pub struct ReviewScriptRequest {
    pub expected_version: u32,
    pub status: ReviewStatus,
    pub policy_revision: String,
    pub reason: Option<String>,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct RunWorkspaceTestRequest {
    pub expected_version: u32,
    pub test_path: String,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct StageReleaseRequest {
    pub expected_version: u32,
    pub publish_request_id: String,
    pub artifact_digest: String,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct EntityInput {
    pub id: String,
    pub entity_type: String,
    pub data: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ListScriptsQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
    #[serde(default)]
    pub status: Option<String>,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

impl ListScriptsQuery {
    pub fn status_filter(&self) -> Result<Option<ScriptStatus>, String> {
        self.status
            .as_deref()
            .map(|status| {
                ScriptStatus::parse(status)
                    .ok_or_else(|| format!("Unsupported script status filter: {status}"))
            })
            .transpose()
    }

    pub fn offset(&self) -> u64 {
        (self.normalized_page().saturating_sub(1) as u64) * self.limit()
    }

    pub fn normalized_page(&self) -> u32 {
        self.page.max(1)
    }

    pub fn limit(&self) -> u64 {
        self.per_page.clamp(1, 100) as u64
    }

    pub fn normalized_per_page(&self) -> u32 {
        self.limit() as u32
    }
}

#[derive(Debug, Deserialize)]
pub struct ListExecutionLogQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_execution_log_per_page")]
    pub per_page: u32,
}

fn default_execution_log_per_page() -> u32 {
    50
}

impl ListExecutionLogQuery {
    pub fn offset(&self) -> u64 {
        (self.normalized_page().saturating_sub(1) as u64) * self.limit()
    }

    pub fn normalized_page(&self) -> u32 {
        self.page.max(1)
    }

    pub fn limit(&self) -> u64 {
        self.per_page.clamp(1, 100) as u64
    }

    pub fn normalized_per_page(&self) -> u32 {
        self.limit() as u32
    }
}

// ============ Responses ============

#[derive(Debug, Serialize)]
pub struct ScriptResponse {
    pub id: ScriptId,
    pub name: String,
    pub description: Option<String>,
    pub workspace: AlloyWorkspace,
    pub trigger: ScriptTrigger,
    pub status: ScriptStatus,
    pub version: u32,
    pub error_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Script> for ScriptResponse {
    fn from(s: Script) -> Self {
        Self {
            id: s.id,
            name: s.name,
            description: s.description,
            workspace: s.workspace,
            trigger: s.trigger,
            status: s.status,
            version: s.version,
            error_count: s.error_count,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ReviewDecisionResponse {
    pub id: Uuid,
    pub script_id: ScriptId,
    pub revision: u32,
    pub source_digest: String,
    pub status: ReviewStatus,
    pub policy_revision: String,
    pub actor_id: String,
    pub reason: Option<String>,
    pub idempotency_key: Uuid,
    pub created_at: String,
}

impl From<ReviewDecision> for ReviewDecisionResponse {
    fn from(decision: ReviewDecision) -> Self {
        Self {
            id: decision.id,
            script_id: decision.script_id,
            revision: decision.revision,
            source_digest: decision.source_digest,
            status: decision.status,
            policy_revision: decision.policy_revision,
            actor_id: decision.actor_id,
            reason: decision.reason,
            idempotency_key: decision.idempotency_key,
            created_at: decision.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TestRunResponse {
    pub id: Uuid,
    pub script_id: ScriptId,
    pub revision: u32,
    pub source_digest: String,
    pub test_path: String,
    pub actor_id: String,
    pub idempotency_key: Uuid,
    pub status: TestRunStatus,
    pub passed: Option<bool>,
    pub error: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

impl From<TestRun> for TestRunResponse {
    fn from(run: TestRun) -> Self {
        Self {
            id: run.id,
            script_id: run.script_id,
            revision: run.revision,
            source_digest: run.source_digest,
            test_path: run.test_path,
            actor_id: run.actor_id,
            idempotency_key: run.idempotency_key,
            status: run.status,
            passed: run.passed,
            error: run.error,
            created_at: run.created_at.to_rfc3339(),
            completed_at: run.completed_at.map(|time| time.to_rfc3339()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StageReleaseResponse {
    pub staging_id: String,
    pub created: bool,
}

#[derive(Debug, Serialize)]
pub struct RunScriptResponse {
    pub execution_id: String,
    pub success: bool,
    pub duration_ms: i64,
    pub error: Option<String>,
    pub changes: Option<HashMap<String, serde_json::Value>>,
    pub return_value: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ExecutionLogResponse {
    pub id: String,
    pub script_id: ScriptId,
    pub script_name: String,
    pub phase: String,
    pub outcome: String,
    pub duration_ms: i64,
    pub error: Option<String>,
    pub user_id: Option<String>,
    pub tenant_id: Option<Uuid>,
    pub created_at: String,
}

impl From<ExecutionLogEntry> for ExecutionLogResponse {
    fn from(entry: ExecutionLogEntry) -> Self {
        Self {
            id: entry.id.to_string(),
            script_id: entry.script_id,
            script_name: entry.script_name,
            phase: execution_phase_name(entry.phase).to_string(),
            outcome: entry.outcome,
            duration_ms: entry.duration_ms,
            error: entry.error,
            user_id: entry.user_id,
            tenant_id: entry.tenant_id,
            created_at: entry.created_at.to_rfc3339(),
        }
    }
}

fn execution_phase_name(phase: ExecutionPhase) -> &'static str {
    match phase {
        ExecutionPhase::Before => "before",
        ExecutionPhase::After => "after",
        ExecutionPhase::OnCommit => "on_commit",
        ExecutionPhase::Manual => "manual",
        ExecutionPhase::Scheduled => "scheduled",
    }
}

#[derive(Debug, Serialize)]
pub struct ListExecutionLogResponse {
    pub executions: Vec<ExecutionLogResponse>,
    pub total: usize,
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
}

impl ListExecutionLogResponse {
    pub fn new(
        executions: Vec<ExecutionLogResponse>,
        total: usize,
        page: u32,
        per_page: u32,
    ) -> Self {
        let total_pages = if per_page > 0 {
            ((total as u64).div_ceil(per_page as u64)).min(u32::MAX as u64) as u32
        } else {
            0
        };
        Self {
            executions,
            total,
            page,
            per_page,
            total_pages,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListScriptsResponse {
    pub scripts: Vec<ScriptResponse>,
    pub total: usize,
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
}

impl ListScriptsResponse {
    pub fn new(scripts: Vec<ScriptResponse>, total: usize, page: u32, per_page: u32) -> Self {
        let total_pages = if per_page > 0 {
            ((total as u64).div_ceil(per_page as u64)).min(u32::MAX as u64) as u32
        } else {
            0
        };
        Self {
            scripts,
            total,
            page,
            per_page,
            total_pages,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct SchedulerStatusResponse {
    pub running: bool,
    pub jobs: Vec<ScheduledJobInfo>,
}

#[derive(Debug, Serialize)]
pub struct ScheduledJobInfo {
    pub script_id: ScriptId,
    pub script_name: String,
    pub cron_expression: String,
    pub next_run: String,
    pub last_run: Option<String>,
    pub running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn execution_entry(phase: ExecutionPhase) -> ExecutionLogEntry {
        ExecutionLogEntry {
            id: Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap(),
            script_id: Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap(),
            script_name: "canonical_transport_mapping".to_string(),
            phase,
            outcome: "success".to_string(),
            duration_ms: 123,
            error: Some("operator-visible error".to_string()),
            user_id: Some("operator-7".to_string()),
            tenant_id: Some(Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap()),
            created_at: Utc.with_ymd_and_hms(2026, 6, 19, 12, 0, 0).unwrap(),
        }
    }

    #[test]
    fn execution_log_response_preserves_canonical_transport_fields() {
        let entry = execution_entry(ExecutionPhase::OnCommit);
        let response = ExecutionLogResponse::from(entry);

        assert_eq!(response.id, "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        assert_eq!(
            response.script_id,
            Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap()
        );
        assert_eq!(response.script_name, "canonical_transport_mapping");
        assert_eq!(response.phase, "on_commit");
        assert_eq!(response.outcome, "success");
        assert_eq!(response.duration_ms, 123);
        assert_eq!(response.error.as_deref(), Some("operator-visible error"));
        assert_eq!(response.user_id.as_deref(), Some("operator-7"));
        assert_eq!(
            response.tenant_id,
            Some(Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap())
        );
        assert_eq!(response.created_at, "2026-06-19T12:00:00+00:00");
    }

    #[test]
    fn execution_phase_names_match_rest_contract() {
        let phases = [
            (ExecutionPhase::Before, "before"),
            (ExecutionPhase::After, "after"),
            (ExecutionPhase::OnCommit, "on_commit"),
            (ExecutionPhase::Manual, "manual"),
            (ExecutionPhase::Scheduled, "scheduled"),
        ];

        for (phase, expected) in phases {
            assert_eq!(
                ExecutionLogResponse::from(execution_entry(phase)).phase,
                expected
            );
        }
    }

    #[test]
    fn execution_log_response_reports_exact_total_pages() {
        let response = ListExecutionLogResponse::new(Vec::new(), 101, 3, 50);

        assert_eq!(response.total, 101);
        assert_eq!(response.page, 3);
        assert_eq!(response.per_page, 50);
        assert_eq!(response.total_pages, 3);
    }

    #[test]
    fn list_scripts_query_clamps_limit_before_offset() {
        let query = ListScriptsQuery {
            page: 3,
            per_page: 250,
            status: None,
        };

        assert_eq!(query.normalized_page(), 3);
        assert_eq!(query.limit(), 100);
        assert_eq!(query.normalized_per_page(), 100);
        assert_eq!(query.offset(), 200);

        let zero_page = ListScriptsQuery {
            page: 0,
            per_page: 20,
            status: None,
        };
        assert_eq!(zero_page.normalized_page(), 1);
        assert_eq!(zero_page.offset(), 0);
    }

    #[test]
    fn list_scripts_query_rejects_unknown_status_filter() {
        let valid = ListScriptsQuery {
            page: 1,
            per_page: 20,
            status: Some("active".to_string()),
        };
        assert_eq!(valid.status_filter(), Ok(Some(ScriptStatus::Active)));

        let invalid = ListScriptsQuery {
            page: 1,
            per_page: 20,
            status: Some("retired".to_string()),
        };

        assert_eq!(
            invalid.status_filter(),
            Err("Unsupported script status filter: retired".to_string())
        );
    }

    #[test]
    fn execution_log_query_defaults_and_clamps_to_operator_contract() {
        let query = ListExecutionLogQuery {
            page: 2,
            per_page: 0,
        };

        assert_eq!(query.normalized_page(), 2);
        assert_eq!(query.limit(), 1);
        assert_eq!(query.normalized_per_page(), 1);
        assert_eq!(query.offset(), 1);

        let zero_page = ListExecutionLogQuery {
            page: 0,
            per_page: 50,
        };
        assert_eq!(zero_page.normalized_page(), 1);
        assert_eq!(zero_page.offset(), 0);

        let oversized = ListExecutionLogQuery {
            page: 2,
            per_page: 500,
        };
        assert_eq!(oversized.limit(), 100);
        assert_eq!(oversized.offset(), 100);
    }

    #[test]
    fn update_request_requires_an_expected_version() {
        assert!(serde_json::from_str::<UpdateScriptRequest>(r#"{}"#).is_err());
        let request = serde_json::from_str::<UpdateScriptRequest>(r#"{"expected_version": 3}"#)
            .expect("expected version should deserialize");
        assert_eq!(request.expected_version, 3);
    }

    #[test]
    fn run_request_requires_an_expected_version() {
        assert!(serde_json::from_str::<RunScriptRequest>(r#"{}"#).is_err());
        let request = serde_json::from_str::<RunScriptRequest>(r#"{"expected_version": 3}"#)
            .expect("expected version should deserialize");
        assert_eq!(request.expected_version, 3);
    }

    #[test]
    fn lifecycle_request_requires_an_expected_version() {
        assert!(serde_json::from_str::<ScriptRevisionRequest>(r#"{}"#).is_err());
        let request = serde_json::from_str::<ScriptRevisionRequest>(r#"{"expected_version": 3}"#)
            .expect("expected version should deserialize");
        assert_eq!(request.expected_version, 3);
    }
}
