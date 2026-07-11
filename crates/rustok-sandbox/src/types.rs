use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{SandboxError, SandboxPolicy};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxExecutorKind {
    Rhai,
    WasmComponent,
    Sidecar,
}

impl fmt::Display for SandboxExecutorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Rhai => "rhai",
            Self::WasmComponent => "wasm_component",
            Self::Sidecar => "sidecar",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SandboxSubject {
    AlloyDraft {
        draft_id: Uuid,
        revision: u64,
    },
    ModuleArtifact {
        slug: String,
        version: String,
        digest: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPhase {
    Validate,
    Test,
    Manual,
    BeforeHook,
    AfterHook,
    Scheduled,
    Http,
    Event,
    Lifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxContext {
    pub execution_id: Uuid,
    pub phase: ExecutionPhase,
    pub timestamp: DateTime<Utc>,
    pub tenant_id: Option<Uuid>,
    pub actor_id: Option<String>,
    pub trace_id: Option<String>,
}

impl SandboxContext {
    pub fn new(phase: ExecutionPhase) -> Self {
        Self {
            execution_id: Uuid::new_v4(),
            phase,
            timestamp: Utc::now(),
            tenant_id: None,
            actor_id: None,
            trace_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxPayload {
    pub executor: SandboxExecutorKind,
    pub media_type: String,
    pub digest: String,
    pub entrypoint: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxRequest {
    pub subject: SandboxSubject,
    pub context: SandboxContext,
    pub payload: SandboxPayload,
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub policy: SandboxPolicy,
}

impl SandboxRequest {
    pub fn validate(&self) -> Result<(), SandboxError> {
        if self.payload.media_type.trim().is_empty() {
            return Err(SandboxError::InvalidRequest(
                "payload media_type must not be empty".to_string(),
            ));
        }
        if self.payload.digest.trim().is_empty() {
            return Err(SandboxError::InvalidRequest(
                "payload digest must not be empty".to_string(),
            ));
        }
        if self.payload.entrypoint.trim().is_empty() {
            return Err(SandboxError::InvalidRequest(
                "payload entrypoint must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub duration_ms: u64,
    pub instructions_consumed: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
    pub output_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxOutcome {
    pub execution_id: Uuid,
    #[serde(default)]
    pub output: Value,
    #[serde(default)]
    pub metrics: ExecutionMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Started,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub execution_id: Uuid,
    pub subject: SandboxSubject,
    pub executor: SandboxExecutorKind,
    pub status: ExecutionStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub metrics: Option<ExecutionMetrics>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

