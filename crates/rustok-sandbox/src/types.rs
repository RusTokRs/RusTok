use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{SandboxError, SandboxPolicy};

/// Cooperative, request-scoped cancellation handle for one sandbox execution.
#[derive(Clone, Debug, Default)]
pub struct SandboxCancellation(Arc<AtomicBool>);

impl SandboxCancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

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
        /// Exact durable installation selected by the module owner. This is
        /// host-controlled execution identity, not artifact-supplied input.
        installation_id: Uuid,
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
    /// Immutable ABI selected by the admitted draft or artifact descriptor.
    pub runtime_abi: String,
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
        if matches!(
            &self.subject,
            SandboxSubject::ModuleArtifact {
                installation_id,
                ..
            } if installation_id.is_nil()
        ) {
            return Err(SandboxError::InvalidRequest(
                "module artifact installation_id must not be nil".to_string(),
            ));
        }
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
        if self.payload.runtime_abi.trim().is_empty() {
            return Err(SandboxError::InvalidRequest(
                "payload runtime_abi must not be empty".to_string(),
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
    /// Time spent in request validation, admission, and required start evidence
    /// before the executor begins. Current admission is fail-fast, so this is
    /// normally near zero until a queued admission policy is introduced.
    pub queue_time_ms: u64,
    /// Elapsed wall-clock time spent inside the selected executor.
    pub duration_ms: u64,
    pub instructions_consumed: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
    pub output_bytes: Option<u64>,
    /// Calls admitted by the shared capability budget, including a later
    /// policy-denied broker call but excluding malformed or rate-rejected input.
    pub capability_calls: u32,
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
    /// Redacted request identity needed by durable owner observers. Payload,
    /// policy grants, input, output, headers, credentials, and error text are
    /// deliberately absent.
    pub context: SandboxContext,
    pub executor: SandboxExecutorKind,
    pub status: ExecutionStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub metrics: Option<ExecutionMetrics>,
    pub error_code: Option<String>,
}
