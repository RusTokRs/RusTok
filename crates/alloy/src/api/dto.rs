use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    context::ExecutionPhase,
    execution_log::ExecutionLogEntry,
    model::{Script, ScriptId, ScriptStatus, ScriptTrigger},
};

// ============ Requests ============

#[derive(Debug, Deserialize)]
pub struct CreateScriptRequest {
    pub name: String,
    pub description: Option<String>,
    pub code: String,
    pub trigger: ScriptTrigger,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub run_as_system: bool,
    pub tenant_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateScriptRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub code: Option<String>,
    pub trigger: Option<ScriptTrigger>,
    pub status: Option<ScriptStatus>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct RunScriptRequest {
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
    pub entity: Option<EntityInput>,
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
    pub fn offset(&self) -> u64 {
        (self.page.saturating_sub(1) as u64) * (self.per_page as u64)
    }

    pub fn limit(&self) -> u64 {
        self.per_page.min(100) as u64
    }
}

#[derive(Debug, Deserialize)]
pub struct ListExecutionLogQuery {
    #[serde(default)]
    pub script_id: Option<Uuid>,
    #[serde(default = "default_execution_log_limit")]
    pub limit: u64,
}

fn default_execution_log_limit() -> u64 {
    50
}

impl ListExecutionLogQuery {
    pub fn normalized_limit(&self) -> u64 {
        self.limit.clamp(1, 100)
    }
}

// ============ Responses ============

#[derive(Debug, Serialize)]
pub struct ScriptResponse {
    pub id: ScriptId,
    pub name: String,
    pub description: Option<String>,
    pub code: String,
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
            code: s.code,
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
pub struct RunScriptResponse {
    pub execution_id: String,
    pub success: bool,
    pub duration_ms: i64,
    pub error: Option<String>,
    pub changes: Option<HashMap<String, serde_json::Value>>,
    pub return_value: serde_json::Value,
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
            ((total as f64) / (per_page as f64)).ceil() as u32
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

#[derive(Debug, Serialize)]
pub struct ExecutionLogEntryResponse {
    pub id: Uuid,
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

impl From<ExecutionLogEntry> for ExecutionLogEntryResponse {
    fn from(entry: ExecutionLogEntry) -> Self {
        Self {
            id: entry.id,
            script_id: entry.script_id,
            script_name: entry.script_name,
            phase: execution_phase_label(entry.phase).to_string(),
            outcome: entry.outcome,
            duration_ms: entry.duration_ms,
            error: entry.error,
            user_id: entry.user_id,
            tenant_id: entry.tenant_id,
            created_at: entry.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListExecutionLogResponse {
    pub executions: Vec<ExecutionLogEntryResponse>,
    pub limit: u64,
}

fn execution_phase_label(phase: ExecutionPhase) -> &'static str {
    match phase {
        ExecutionPhase::Before => "before",
        ExecutionPhase::After => "after",
        ExecutionPhase::OnCommit => "on_commit",
        ExecutionPhase::Manual => "manual",
        ExecutionPhase::Scheduled => "scheduled",
    }
}
