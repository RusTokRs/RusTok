use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranslationAdminTransportContext {
    pub token: Option<String>,
    pub tenant_slug: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationResourceIdentity {
    pub owner_slug: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub subresource_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalOrigin {
    Manual,
    Import,
    Memory,
    Ai,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalValueInput {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterchangeField {
    pub key: String,
    pub source_value: String,
    pub exact_target_value: Option<String>,
    pub proposed_value: Option<String>,
    pub source_hash: String,
    pub required: bool,
    pub max_characters: Option<u32>,
    pub protected_tokens: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterchangeItem {
    pub item_id: String,
    pub identity: TranslationResourceIdentity,
    pub source_digest: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub fields: Vec<InterchangeField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterchangeDocument {
    pub schema_version: u16,
    pub job_id: String,
    pub source_locale: String,
    pub target_locale: String,
    pub items: Vec<InterchangeItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterchangeArtifactItemOutcome {
    pub item_id: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterchangeConflictReport {
    pub total_items: u16,
    pub accepted_items: u16,
    pub conflict_items: u16,
    pub rejected_items: u16,
    pub outcomes: Vec<InterchangeArtifactItemOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterchangeArtifact {
    pub id: String,
    pub job_id: String,
    pub direction: String,
    pub status: String,
    pub content_length: u64,
    pub checksum_sha256: String,
    pub expires_at: String,
    pub processed_at: Option<String>,
    pub report: Option<InterchangeConflictReport>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterchangeArtifactContent {
    pub artifact: InterchangeArtifact,
    pub document: InterchangeDocument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    User,
    Service,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    pub kind: ActorKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryBinding {
    pub glossary_id: String,
    pub revision: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryScope {
    pub owner_slug: Option<String>,
    pub resource_kind: Option<String>,
    pub field_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlossaryTermPolicy {
    Preferred,
    Allowed,
    Forbidden,
    DoNotTranslate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlossaryMatchKind {
    Exact,
    WholeWord,
    Substring,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryVariant {
    pub value: String,
    pub policy: GlossaryTermPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryConcept {
    pub concept_key: String,
    pub source_term: String,
    pub variants: Vec<GlossaryVariant>,
    pub match_kind: GlossaryMatchKind,
    pub case_sensitive: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MemoryRetentionPolicy {
    OwnerLifecycle,
    RetainUntil,
    LegalHold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MemoryMatchKind {
    Exact,
    ContextualFuzzy,
    Fuzzy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryMatchEvidence {
    pub kind: MemoryMatchKind,
    pub source_exact: bool,
    pub context_match: bool,
    pub base_similarity_basis_points: u16,
    pub context_bonus_basis_points: u16,
    pub final_similarity_basis_points: u16,
    pub segmentation_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySuggestion {
    pub entry_id: String,
    pub source_text: String,
    pub target_text: String,
    pub source_hash: String,
    pub owner_slug: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub field_key: String,
    pub origin: String,
    pub proposal_id: String,
    pub apply_receipt_id: String,
    pub evidence: MemoryMatchEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    pub id: String,
    pub tenant_id: String,
    pub source_locale: String,
    pub target_locale: String,
    pub owner_slug: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub subresource_id: Option<String>,
    pub field_key: String,
    pub source_text: String,
    pub target_text: String,
    pub source_hash: String,
    pub target_hash: String,
    pub context_fingerprint: String,
    pub segmentation_version: String,
    pub origin: String,
    pub quality_state: String,
    pub reviewer_actor_kind: String,
    pub reviewer_actor_id: String,
    pub proposal_id: String,
    pub apply_receipt_id: String,
    pub retention_policy: MemoryRetentionPolicy,
    pub retain_until: Option<String>,
    pub tombstoned_at: Option<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryMutation {
    pub entry_id: String,
    pub revision: i64,
    pub state: String,
    pub retention_policy: MemoryRetentionPolicy,
    pub retain_until: Option<String>,
    pub tombstoned_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "input", rename_all = "snake_case")]
pub enum TranslationAdminOperation {
    ReadPolicy,
    ReadMachineOperationStatus {
        operation_id: String,
    },
    ListTargets,
    ListGlossaries {
        limit: u16,
    },
    ReadGlossary {
        glossary_id: String,
        revision: Option<i64>,
    },
    ListMemoryEntries {
        source_locale: Option<String>,
        target_locale: Option<String>,
        include_tombstoned: bool,
        limit: u16,
    },
    ReadMemoryEntry {
        entry_id: String,
    },
    LookupMemory {
        source_locale: String,
        target_locale: String,
        identity: TranslationResourceIdentity,
        field_key: String,
        source_text: String,
        minimum_similarity_basis_points: u16,
        limit: u16,
    },
    ReadJobProgress {
        job_id: String,
    },
    ReadReviewerQueue {
        job_id: String,
        assignee: Option<Actor>,
        include_unassigned: bool,
        limit: u16,
    },
    ReadReviewerWorkload {
        job_id: String,
    },
    ListWorkflowNotes {
        job_id: String,
        item_id: Option<String>,
        include_resolved: bool,
        limit: u16,
    },
    ListInterchangeArtifacts {
        job_id: Option<String>,
        include_expired: bool,
        limit: u16,
    },
    ReadInterchangeArtifact {
        artifact_id: String,
    },
    ExportJob {
        job_id: String,
        max_items: u16,
    },
    ReadProviderProgress {
        owner_slug: String,
        resource_kind: String,
        source_locale: String,
        target_locale: String,
    },
    ReadRequiredProviderProgress {
        owner_slug: String,
        resource_kind: String,
        source_locale: String,
    },
    ReplacePolicy {
        expected_revision: i64,
        required_target_locales: Vec<String>,
        idempotency_key: String,
    },
    CreateWorkflowNote {
        job_id: String,
        item_id: Option<String>,
        body: String,
        idempotency_key: String,
    },
    ResolveWorkflowNote {
        note_id: String,
        expected_revision: i64,
        idempotency_key: String,
    },
    CreateInterchangeExportArtifact {
        job_id: String,
        max_items: u16,
        expires_in_seconds: u32,
        idempotency_key: String,
    },
    StoreInterchangeImportArtifact {
        job_id: String,
        document_json: String,
        expires_in_seconds: u32,
        idempotency_key: String,
    },
    ProcessInterchangeImportArtifact {
        artifact_id: String,
        idempotency_key: String,
    },
    CreateGlossary {
        name: String,
        description: String,
        source_locale: String,
        target_locale: String,
        scope: GlossaryScope,
        idempotency_key: String,
    },
    UpdateGlossary {
        glossary_id: String,
        expected_revision: i64,
        name: String,
        description: String,
        idempotency_key: String,
    },
    ReplaceGlossaryTerms {
        glossary_id: String,
        expected_revision: i64,
        concepts: Vec<GlossaryConcept>,
        idempotency_key: String,
    },
    SetGlossaryActive {
        glossary_id: String,
        expected_revision: i64,
        is_active: bool,
        idempotency_key: String,
    },
    SetMemoryRetention {
        entry_id: String,
        expected_revision: i64,
        policy: MemoryRetentionPolicy,
        retain_until: Option<String>,
        idempotency_key: String,
    },
    TombstoneMemoryEntry {
        entry_id: String,
        expected_revision: i64,
        idempotency_key: String,
    },
    PurgeMemoryEntry {
        entry_id: String,
        expected_revision: i64,
        idempotency_key: String,
    },
    CreateJob {
        source_locale: String,
        target_locale: String,
        glossary: Option<GlossaryBinding>,
        idempotency_key: String,
    },
    AddItem {
        job_id: String,
        identity: TranslationResourceIdentity,
        idempotency_key: String,
    },
    SaveProposal {
        item_id: String,
        origin: ProposalOrigin,
        values: Vec<ProposalValueInput>,
        idempotency_key: String,
    },
    ImportItem {
        schema_version: u16,
        job_id: String,
        item_id: String,
        identity: TranslationResourceIdentity,
        source_digest: String,
        values: Vec<ProposalValueInput>,
        idempotency_key: String,
    },
    EstimateMachineTranslation {
        item_id: String,
        field_keys: Vec<String>,
        minimum_memory_similarity_basis_points: u16,
        tone: Option<String>,
        domain: Option<String>,
        style: Option<String>,
        idempotency_key: String,
    },
    GenerateMachineProposal {
        item_id: String,
        field_keys: Vec<String>,
        minimum_memory_similarity_basis_points: u16,
        tone: Option<String>,
        domain: Option<String>,
        style: Option<String>,
        idempotency_key: String,
    },
    CancelMachineOperation {
        operation_id: String,
        reason: String,
        idempotency_key: String,
    },
    RecoverMachineOperation {
        operation_id: String,
        expected_updated_at: String,
        item_id: String,
        field_keys: Vec<String>,
        minimum_memory_similarity_basis_points: u16,
        tone: Option<String>,
        domain: Option<String>,
        style: Option<String>,
        reason: String,
        idempotency_key: String,
    },
    SubmitProposal {
        item_id: String,
        proposal_id: String,
        idempotency_key: String,
    },
    ApproveProposal {
        item_id: String,
        proposal_id: String,
        idempotency_key: String,
    },
    ApplyProposal {
        item_id: String,
        proposal_id: String,
        idempotency_key: String,
    },
    AssignItem {
        item_id: String,
        expected_revision: i64,
        assignee: Actor,
        idempotency_key: String,
    },
    UnassignItem {
        item_id: String,
        expected_revision: i64,
        idempotency_key: String,
    },
    CancelJob {
        job_id: String,
        expected_revision: i64,
        reason: String,
        idempotency_key: String,
    },
    RetryItem {
        item_id: String,
        expected_revision: i64,
        reason: String,
        idempotency_key: String,
    },
    RecoverApply {
        operation_id: String,
        expected_attempt_count: i64,
        reason: String,
        idempotency_key: String,
    },
    RebuildJobProgress {
        job_id: String,
        idempotency_key: String,
    },
    SyncProviderInventory {
        owner_slug: String,
        resource_kind: String,
        limit: u16,
    },
    RebuildProviderInventory {
        owner_slug: String,
        resource_kind: String,
        source_locale: String,
        target_locale: String,
        page_size: u16,
    },
}

impl TranslationAdminOperation {
    #[cfg(any(feature = "ssr", test))]
    pub fn idempotency_key(&self) -> Option<&str> {
        match self {
            Self::ReplacePolicy {
                idempotency_key, ..
            }
            | Self::CreateGlossary {
                idempotency_key, ..
            }
            | Self::UpdateGlossary {
                idempotency_key, ..
            }
            | Self::ReplaceGlossaryTerms {
                idempotency_key, ..
            }
            | Self::SetGlossaryActive {
                idempotency_key, ..
            }
            | Self::SetMemoryRetention {
                idempotency_key, ..
            }
            | Self::TombstoneMemoryEntry {
                idempotency_key, ..
            }
            | Self::PurgeMemoryEntry {
                idempotency_key, ..
            }
            | Self::CreateJob {
                idempotency_key, ..
            }
            | Self::AddItem {
                idempotency_key, ..
            }
            | Self::SaveProposal {
                idempotency_key, ..
            }
            | Self::ImportItem {
                idempotency_key, ..
            }
            | Self::EstimateMachineTranslation {
                idempotency_key, ..
            }
            | Self::GenerateMachineProposal {
                idempotency_key, ..
            }
            | Self::CancelMachineOperation {
                idempotency_key, ..
            }
            | Self::RecoverMachineOperation {
                idempotency_key, ..
            }
            | Self::SubmitProposal {
                idempotency_key, ..
            }
            | Self::ApproveProposal {
                idempotency_key, ..
            }
            | Self::ApplyProposal {
                idempotency_key, ..
            }
            | Self::AssignItem {
                idempotency_key, ..
            }
            | Self::UnassignItem {
                idempotency_key, ..
            }
            | Self::CancelJob {
                idempotency_key, ..
            }
            | Self::RetryItem {
                idempotency_key, ..
            }
            | Self::RecoverApply {
                idempotency_key, ..
            }
            | Self::RebuildJobProgress {
                idempotency_key, ..
            }
            | Self::CreateWorkflowNote {
                idempotency_key, ..
            }
            | Self::ResolveWorkflowNote {
                idempotency_key, ..
            }
            | Self::CreateInterchangeExportArtifact {
                idempotency_key, ..
            }
            | Self::StoreInterchangeImportArtifact {
                idempotency_key, ..
            }
            | Self::ProcessInterchangeImportArtifact {
                idempotency_key, ..
            } => Some(idempotency_key),
            Self::ReadPolicy
            | Self::ReadMachineOperationStatus { .. }
            | Self::ListTargets
            | Self::ListGlossaries { .. }
            | Self::ReadGlossary { .. }
            | Self::ListMemoryEntries { .. }
            | Self::ReadMemoryEntry { .. }
            | Self::LookupMemory { .. }
            | Self::ReadJobProgress { .. }
            | Self::ReadReviewerQueue { .. }
            | Self::ReadReviewerWorkload { .. }
            | Self::ListWorkflowNotes { .. }
            | Self::ListInterchangeArtifacts { .. }
            | Self::ReadInterchangeArtifact { .. }
            | Self::ExportJob { .. }
            | Self::ReadProviderProgress { .. }
            | Self::ReadRequiredProviderProgress { .. }
            | Self::SyncProviderInventory { .. }
            | Self::RebuildProviderInventory { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", content = "value", rename_all = "snake_case")]
pub enum TranslationAdminResponse {
    Policy(TranslationPolicy),
    Targets(Vec<TranslationTarget>),
    Glossaries(Vec<GlossarySummary>),
    Glossary(Glossary),
    MemoryEntries(Vec<MemoryEntry>),
    MemoryEntry(MemoryEntry),
    MemorySuggestions(Vec<MemorySuggestion>),
    MemoryMutation(MemoryMutation),
    JobProgress(JobProgress),
    ReviewerQueue(Vec<ReviewerQueueItem>),
    ReviewerWorkloads(Vec<ReviewerWorkload>),
    WorkflowNotes(Vec<WorkflowNote>),
    WorkflowNote(WorkflowNote),
    InterchangeDocument(InterchangeDocument),
    InterchangeArtifacts(Vec<InterchangeArtifact>),
    InterchangeArtifact(InterchangeArtifact),
    InterchangeArtifactContent(InterchangeArtifactContent),
    ProviderProgress(ProviderProgress),
    RequiredProviderProgress(RequiredProviderProgress),
    Job(Job),
    Item(JobItem),
    Proposal(Proposal),
    MachineEstimate(MachineTranslationEstimate),
    MachineProposal(MachineProposal),
    MachineOperationStatus(MachineOperationStatus),
    MachineCancellation(MachineCancellation),
    Apply(ApplyResult),
    Assignment(Assignment),
    Cancellation(Cancellation),
    Retry(Retry),
    Inventory(InventoryResult),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationPolicy {
    pub tenant_id: String,
    pub required_target_locales: Vec<String>,
    pub tenant_locale_policy_revision: i64,
    pub revision: i64,
    pub freshness: String,
    pub disabled_required_target_locales: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationTarget {
    pub owner_slug: String,
    pub resource_kind: String,
    pub display_name: String,
    pub capabilities: Vec<String>,
    pub read_permission_floor: Vec<String>,
    pub apply_permission_floor: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossarySummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_locale: String,
    pub target_locale: String,
    pub scope: GlossaryScope,
    pub is_active: bool,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Glossary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_locale: String,
    pub target_locale: String,
    pub scope: GlossaryScope,
    pub is_active: bool,
    pub revision: i64,
    pub concepts: Vec<GlossaryConcept>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub job_id: String,
    pub source_digest: String,
    pub total_items: u64,
    pub assigned_items: u64,
    pub terminal_items: u64,
    pub missing_items: u64,
    pub draft_items: u64,
    pub in_review_items: u64,
    pub approved_items: u64,
    pub applying_items: u64,
    pub applied_items: u64,
    pub stale_items: u64,
    pub conflict_items: u64,
    pub blocked_items: u64,
    pub excluded_items: u64,
    pub cancelled_items: u64,
    pub required_units: u64,
    pub optional_units: u64,
    pub applied_required_units: u64,
    pub applied_optional_units: u64,
    pub approved_required_units: u64,
    pub approved_optional_units: u64,
    pub complete_resources: u64,
    pub source_characters: u64,
    pub translated_characters: u64,
    pub revision: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProgress {
    pub owner_slug: String,
    pub resource_kind: String,
    pub source_locale: String,
    pub target_locale: String,
    pub required_units: u64,
    pub exact_required_units: u64,
    pub optional_units: u64,
    pub exact_optional_units: u64,
    pub resources: u64,
    pub complete_resources: u64,
    pub owner_change_cursor: Option<String>,
    pub projected_cursor: Option<String>,
    pub checkpoint_revision: Option<i64>,
    pub checkpoint_updated_at: Option<String>,
    pub freshness: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequiredProviderProgress {
    pub owner_slug: String,
    pub resource_kind: String,
    pub source_locale: String,
    pub required_target_locales: Vec<String>,
    pub translation_policy_revision: i64,
    pub tenant_locale_policy_revision: i64,
    pub required_units: u64,
    pub exact_required_units: u64,
    pub optional_units: u64,
    pub exact_optional_units: u64,
    pub resource_locale_pairs: u64,
    pub complete_resource_locale_pairs: u64,
    pub freshness: String,
    pub targets: Vec<ProviderProgress>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub source_locale: String,
    pub target_locale: String,
    pub glossary: Option<GlossaryBinding>,
    pub status: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobItem {
    pub id: String,
    pub job_id: String,
    pub identity: TranslationResourceIdentity,
    pub status: String,
    pub assignee: Option<Actor>,
    pub source_digest: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewerQueueItem {
    pub item: JobItem,
    pub proposal_id: String,
    pub proposal_revision: i64,
    pub submitted_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewerWorkload {
    pub job_id: String,
    pub assignee: Option<Actor>,
    pub open_items: u64,
    pub missing_items: u64,
    pub draft_items: u64,
    pub in_review_items: u64,
    pub approved_items: u64,
    pub applying_items: u64,
    pub rebase_required_items: u64,
    pub blocked_items: u64,
    pub source_characters: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowNote {
    pub id: String,
    pub job_id: String,
    pub item_id: Option<String>,
    pub body: String,
    pub author: Actor,
    pub revision: i64,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<Actor>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalValue {
    pub key: String,
    pub value: String,
    pub expected_source_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QaIssue {
    pub field: Option<String>,
    pub severity: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub id: String,
    pub item_id: String,
    pub proposal_revision: i64,
    pub origin: String,
    pub values: Vec<ProposalValue>,
    pub qa_issues: Vec<QaIssue>,
    pub qa_accepted: bool,
    pub status: String,
    pub approval_receipt_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineTranslationAttempt {
    pub attempt: u16,
    pub provider_profile_id: String,
    pub provider_slug: String,
    pub model: String,
    pub fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineTranslationUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost_minor_units: u64,
    pub currency_code: String,
    pub price_snapshot_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineTranslationEstimate {
    pub input_tokens_upper_bound: u64,
    pub output_tokens_upper_bound: u64,
    pub attempts_upper_bound: u16,
    pub cost_minor_units_upper_bound: u64,
    pub currency_code: String,
    pub price_snapshot_digest: String,
    pub review_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineTranslationDiagnostic {
    pub code: String,
    pub blocking: bool,
    pub unit_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineProposal {
    pub operation_id: String,
    pub item_id: String,
    pub proposal_id: String,
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
    pub attempts: Vec<MachineTranslationAttempt>,
    pub usage: MachineTranslationUsage,
    pub diagnostics: Vec<MachineTranslationDiagnostic>,
    pub review_required: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineCancellation {
    pub cancellation_id: String,
    pub operation_id: String,
    pub status: String,
    pub provider_execution_id: Option<String>,
    pub provider_status: String,
    pub provider_error_code: Option<String>,
    pub provider_observed_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineOperationStatus {
    pub operation_id: String,
    pub item_id: String,
    pub status: String,
    pub provider_execution_id: Option<String>,
    pub provider_status: String,
    pub provider_error_code: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub operation_id: String,
    pub item_id: String,
    pub proposal_id: String,
    pub provider_receipt_id: String,
    pub resource_revision: String,
    pub target_revision: String,
    pub applied_field_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assignment {
    pub operation_id: String,
    pub item_id: String,
    pub assignee: Option<Actor>,
    pub item_revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cancellation {
    pub cancellation_id: String,
    pub job_id: String,
    pub job_revision: i64,
    pub cancelled_item_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Retry {
    pub retry_id: String,
    pub item_id: String,
    pub item_revision: i64,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryResult {
    pub observed_resources: u64,
    pub checkpoint: Option<String>,
    pub checkpoint_revision: i64,
}

#[cfg(test)]
mod tests {
    use super::{
        Actor, ActorKind, GlossaryScope, ProposalOrigin, ProposalValueInput,
        TranslationAdminOperation, TranslationResourceIdentity,
    };

    #[test]
    fn every_idempotency_bound_command_exposes_its_caller_key() {
        let identity = TranslationResourceIdentity {
            owner_slug: "media".to_string(),
            resource_kind: "asset".to_string(),
            resource_id: "asset-1".to_string(),
            subresource_id: None,
        };
        let actor = Actor {
            kind: ActorKind::User,
            id: "user-1".to_string(),
        };
        let writes = vec![
            TranslationAdminOperation::ReplacePolicy {
                expected_revision: 1,
                required_target_locales: vec!["de".to_string()],
                idempotency_key: "replace-policy".to_string(),
            },
            TranslationAdminOperation::CreateGlossary {
                name: "Product terminology".to_string(),
                description: String::new(),
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                scope: GlossaryScope::default(),
                idempotency_key: "create-glossary".to_string(),
            },
            TranslationAdminOperation::UpdateGlossary {
                glossary_id: "glossary-1".to_string(),
                expected_revision: 1,
                name: "Updated terminology".to_string(),
                description: String::new(),
                idempotency_key: "update-glossary".to_string(),
            },
            TranslationAdminOperation::ReplaceGlossaryTerms {
                glossary_id: "glossary-1".to_string(),
                expected_revision: 2,
                concepts: Vec::new(),
                idempotency_key: "replace-glossary-terms".to_string(),
            },
            TranslationAdminOperation::SetGlossaryActive {
                glossary_id: "glossary-1".to_string(),
                expected_revision: 3,
                is_active: false,
                idempotency_key: "set-glossary-active".to_string(),
            },
            TranslationAdminOperation::CreateJob {
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                glossary: None,
                idempotency_key: "create-job".to_string(),
            },
            TranslationAdminOperation::CreateWorkflowNote {
                job_id: "job-1".to_string(),
                item_id: Some("item-1".to_string()),
                body: "Private reviewer context".to_string(),
                idempotency_key: "create-workflow-note".to_string(),
            },
            TranslationAdminOperation::ResolveWorkflowNote {
                note_id: "note-1".to_string(),
                expected_revision: 0,
                idempotency_key: "resolve-workflow-note".to_string(),
            },
            TranslationAdminOperation::CreateInterchangeExportArtifact {
                job_id: "job-1".to_string(),
                max_items: 50,
                expires_in_seconds: 86_400,
                idempotency_key: "create-interchange-export-artifact".to_string(),
            },
            TranslationAdminOperation::StoreInterchangeImportArtifact {
                job_id: "job-1".to_string(),
                document_json: "{}".to_string(),
                expires_in_seconds: 86_400,
                idempotency_key: "store-interchange-import-artifact".to_string(),
            },
            TranslationAdminOperation::ProcessInterchangeImportArtifact {
                artifact_id: "artifact-1".to_string(),
                idempotency_key: "process-interchange-import-artifact".to_string(),
            },
            TranslationAdminOperation::AddItem {
                job_id: "job-1".to_string(),
                identity,
                idempotency_key: "add-item".to_string(),
            },
            TranslationAdminOperation::SaveProposal {
                item_id: "item-1".to_string(),
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValueInput {
                    key: "alt".to_string(),
                    value: "Beschreibung".to_string(),
                }],
                idempotency_key: "save-proposal".to_string(),
            },
            TranslationAdminOperation::ImportItem {
                schema_version: 1,
                job_id: "job-1".to_string(),
                item_id: "item-1".to_string(),
                identity: TranslationResourceIdentity {
                    owner_slug: "media".to_string(),
                    resource_kind: "asset".to_string(),
                    resource_id: "asset-1".to_string(),
                    subresource_id: None,
                },
                source_digest: "source-digest".to_string(),
                values: vec![ProposalValueInput {
                    key: "alt".to_string(),
                    value: "Beschreibung".to_string(),
                }],
                idempotency_key: "import-item".to_string(),
            },
            TranslationAdminOperation::EstimateMachineTranslation {
                item_id: "item-1".to_string(),
                field_keys: vec!["alt".to_string()],
                minimum_memory_similarity_basis_points: 7_000,
                tone: None,
                domain: None,
                style: None,
                idempotency_key: "estimate-machine-translation".to_string(),
            },
            TranslationAdminOperation::GenerateMachineProposal {
                item_id: "item-1".to_string(),
                field_keys: vec!["alt".to_string()],
                minimum_memory_similarity_basis_points: 7_000,
                tone: None,
                domain: None,
                style: None,
                idempotency_key: "generate-machine-proposal".to_string(),
            },
            TranslationAdminOperation::CancelMachineOperation {
                operation_id: "operation-1".to_string(),
                reason: "Cancel pending machine translation".to_string(),
                idempotency_key: "cancel-machine-operation".to_string(),
            },
            TranslationAdminOperation::RecoverMachineOperation {
                operation_id: "operation-1".to_string(),
                expected_updated_at: "2026-07-29T12:00:00Z".to_string(),
                item_id: "item-1".to_string(),
                field_keys: vec!["alt".to_string()],
                minimum_memory_similarity_basis_points: 7_000,
                tone: None,
                domain: None,
                style: None,
                reason: "Recover completed provider result".to_string(),
                idempotency_key: "recover-machine-operation".to_string(),
            },
            TranslationAdminOperation::SubmitProposal {
                item_id: "item-1".to_string(),
                proposal_id: "proposal-1".to_string(),
                idempotency_key: "submit-proposal".to_string(),
            },
            TranslationAdminOperation::ApproveProposal {
                item_id: "item-1".to_string(),
                proposal_id: "proposal-1".to_string(),
                idempotency_key: "approve-proposal".to_string(),
            },
            TranslationAdminOperation::ApplyProposal {
                item_id: "item-1".to_string(),
                proposal_id: "proposal-1".to_string(),
                idempotency_key: "apply-proposal".to_string(),
            },
            TranslationAdminOperation::AssignItem {
                item_id: "item-1".to_string(),
                expected_revision: 2,
                assignee: actor,
                idempotency_key: "assign-item".to_string(),
            },
            TranslationAdminOperation::UnassignItem {
                item_id: "item-1".to_string(),
                expected_revision: 3,
                idempotency_key: "unassign-item".to_string(),
            },
            TranslationAdminOperation::CancelJob {
                job_id: "job-1".to_string(),
                expected_revision: 4,
                reason: "Superseded".to_string(),
                idempotency_key: "cancel-job".to_string(),
            },
            TranslationAdminOperation::RetryItem {
                item_id: "item-1".to_string(),
                expected_revision: 5,
                reason: "Owner recovered".to_string(),
                idempotency_key: "retry-item".to_string(),
            },
            TranslationAdminOperation::RecoverApply {
                operation_id: "operation-1".to_string(),
                expected_attempt_count: 1,
                reason: "Reconcile unknown outcome".to_string(),
                idempotency_key: "recover-apply".to_string(),
            },
            TranslationAdminOperation::RebuildJobProgress {
                job_id: "job-1".to_string(),
                idempotency_key: "rebuild-job-progress".to_string(),
            },
        ];

        for operation in writes {
            assert!(
                operation
                    .idempotency_key()
                    .is_some_and(|key| !key.is_empty()),
                "{operation:?}"
            );
        }
    }

    #[test]
    fn read_and_inventory_discovery_operations_do_not_forge_write_identity() {
        let reads = [
            TranslationAdminOperation::ReadPolicy,
            TranslationAdminOperation::ReadMachineOperationStatus {
                operation_id: "operation-1".to_string(),
            },
            TranslationAdminOperation::ListTargets,
            TranslationAdminOperation::ListGlossaries { limit: 50 },
            TranslationAdminOperation::ReadGlossary {
                glossary_id: "glossary-1".to_string(),
                revision: None,
            },
            TranslationAdminOperation::ListInterchangeArtifacts {
                job_id: Some("job-1".to_string()),
                include_expired: false,
                limit: 50,
            },
            TranslationAdminOperation::ReadInterchangeArtifact {
                artifact_id: "artifact-1".to_string(),
            },
            TranslationAdminOperation::ReadJobProgress {
                job_id: "job-1".to_string(),
            },
            TranslationAdminOperation::ReadReviewerQueue {
                job_id: "job-1".to_string(),
                assignee: Some(Actor {
                    kind: ActorKind::User,
                    id: "reviewer-1".to_string(),
                }),
                include_unassigned: true,
                limit: 50,
            },
            TranslationAdminOperation::ReadReviewerWorkload {
                job_id: "job-1".to_string(),
            },
            TranslationAdminOperation::ListWorkflowNotes {
                job_id: "job-1".to_string(),
                item_id: Some("item-1".to_string()),
                include_resolved: true,
                limit: 50,
            },
            TranslationAdminOperation::ExportJob {
                job_id: "job-1".to_string(),
                max_items: 200,
            },
            TranslationAdminOperation::ReadProviderProgress {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
            },
            TranslationAdminOperation::ReadRequiredProviderProgress {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                source_locale: "en".to_string(),
            },
            TranslationAdminOperation::SyncProviderInventory {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                limit: 100,
            },
            TranslationAdminOperation::RebuildProviderInventory {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                page_size: 100,
            },
        ];

        for operation in reads {
            assert_eq!(operation.idempotency_key(), None, "{operation:?}");
        }
    }
}
