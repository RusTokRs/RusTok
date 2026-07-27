use chrono::{DateTime, Utc};
use rhai::Dynamic;
use std::collections::HashMap;
use uuid::Uuid;

use crate::context::ExecutionPhase;
use crate::error::ScriptError;
use crate::model::ScriptId;
use crate::sandbox_request::AlloyExecutionEvidence;

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub script_id: ScriptId,
    pub script_name: String,
    pub execution_id: Uuid,
    pub phase: ExecutionPhase,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub evidence: Option<AlloyExecutionEvidence>,
    pub outcome: ExecutionOutcome,
}

#[derive(Debug, Clone)]
pub enum ExecutionOutcome {
    Success {
        return_value: Option<Dynamic>,
        entity_changes: HashMap<String, Dynamic>,
    },
    Aborted {
        reason: String,
    },
    Failed {
        error: ScriptError,
    },
}

impl ExecutionResult {
    pub fn is_success(&self) -> bool {
        matches!(self.outcome, ExecutionOutcome::Success { .. })
    }

    pub fn is_aborted(&self) -> bool {
        matches!(self.outcome, ExecutionOutcome::Aborted { .. })
    }

    pub fn duration_ms(&self) -> i64 {
        (self.finished_at - self.started_at).num_milliseconds()
    }
}

#[derive(Debug)]
pub struct PhaseResult {
    pub phase: ExecutionPhase,
    pub results: Vec<ExecutionResult>,
    pub entity_changes: HashMap<String, Dynamic>,
}

impl PhaseResult {
    pub fn new(phase: ExecutionPhase) -> Self {
        Self {
            phase,
            results: Vec::new(),
            entity_changes: HashMap::new(),
        }
    }

    pub fn has_abort(&self) -> Option<&str> {
        for result in &self.results {
            if let ExecutionOutcome::Aborted { reason } = &result.outcome {
                return Some(reason);
            }
        }
        None
    }

    pub fn has_errors(&self) -> bool {
        self.results
            .iter()
            .any(|result| matches!(result.outcome, ExecutionOutcome::Failed { .. }))
    }

    pub fn merge_changes(&mut self) {
        for result in &self.results {
            if let ExecutionOutcome::Success { entity_changes, .. } = &result.outcome {
                for (key, value) in entity_changes {
                    self.entity_changes.insert(key.clone(), value.clone());
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum HookOutcome {
    Continue { changes: HashMap<String, Dynamic> },
    Rejected { reason: String },
    Error { error: ScriptError },
}
