use chrono::{DateTime, FixedOffset};
use rustok_translation_targets::FieldKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{MachineTranslationAttemptEvidence, MachineTranslationUsage};

pub const MEMORY_SUGGESTIONS_PER_UNIT: u16 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateMachineProposalInput {
    pub item_id: Uuid,
    pub field_keys: Vec<FieldKey>,
    pub minimum_memory_similarity_basis_points: u16,
    pub tone: Option<String>,
    pub domain: Option<String>,
    pub style: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelMachineOperationInput {
    pub operation_id: Uuid,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverMachineOperationInput {
    pub operation_id: Uuid,
    pub expected_updated_at: DateTime<FixedOffset>,
    pub proposal: GenerateMachineProposalInput,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineCancellationRecord {
    pub cancellation_id: Uuid,
    pub operation_id: Uuid,
    pub status: String,
    pub provider_execution_id: Option<String>,
    pub provider_status: String,
    pub provider_error_code: Option<String>,
    pub provider_observed_at: DateTime<FixedOffset>,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineOperationStatusRecord {
    pub operation_id: Uuid,
    pub item_id: Uuid,
    pub status: String,
    pub provider_execution_id: Option<String>,
    pub provider_status: String,
    pub provider_error_code: Option<String>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineDiagnosticEvidence {
    pub code: String,
    pub blocking: bool,
    pub unit_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineProposalRecord {
    pub operation_id: Uuid,
    pub item_id: Uuid,
    pub proposal_id: Uuid,
    pub adapter_slug: String,
    pub provider_slug: String,
    pub provider_policy_digest: String,
    pub machine_request_digest: String,
    pub glossary_revision: Option<String>,
    pub glossary_digest: Option<String>,
    pub memory_digest: Option<String>,
    pub execution_id: String,
    pub execution_request_digest: String,
    pub prompt_policy_digest: String,
    pub attempts: Vec<MachineTranslationAttemptEvidence>,
    pub usage: MachineTranslationUsage,
    pub diagnostics: Vec<MachineDiagnosticEvidence>,
    pub review_required: bool,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineProposalOutcome {
    Completed(Box<MachineProposalRecord>),
    InProgress(MachineOperationStatusRecord),
}
