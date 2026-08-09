use async_graphql::{Enum, FieldError, InputObject, SimpleObject};
use chrono::{DateTime, FixedOffset};
use rustok_api::graphql::GraphQLError;
use rustok_api::{PortActor, PortActorKind, TenantLocale};
use rustok_translation_targets::{
    FieldKey, OwnerSlug, ResourceId, ResourceKind, TranslationResourceIdentity,
};
use uuid::Uuid;

use crate::{
    ApplyRecord, AssignmentRecord, CancellationRecord, GlossaryBinding, GlossaryConcept,
    GlossaryMatchKind, GlossaryRecord, GlossaryScope, GlossarySummaryRecord, GlossaryTermPolicy,
    GlossaryVariant, JobItemRecord, JobProgressRecord, JobRecord, MachineCancellationRecord,
    MachineProposalRecord, MemoryEntryRecord, MemoryMatchEvidence, MemoryMatchKind,
    MemoryMutationRecord, MemoryRetentionPolicy, MemorySuggestion, ProposalOrigin, ProposalRecord,
    ProviderProgressRecord, RequiredProviderProgressRecord, RetryRecord, ReviewerQueueRecord,
    ReviewerWorkloadRecord, TranslationInterchangeArtifactContent as InterchangeArtifactContent,
    TranslationInterchangeArtifactRecord as InterchangeArtifactRecord,
    TranslationInterchangeConflictReport as InterchangeConflictReport,
    TranslationInterchangeDocument as InterchangeDocument,
    TranslationInterchangeField as InterchangeField, TranslationInterchangeItem as InterchangeItem,
    TranslationInterchangeItemOutcome as InterchangeItemOutcome, TranslationInventoryRebuildResult,
    TranslationInventorySyncResult, TranslationPolicyFreshness, TranslationPolicyRecord,
    WorkflowNoteRecord,
};

#[derive(InputObject)]
pub struct TranslationResourceIdentityInput {
    pub owner_slug: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub subresource_id: Option<String>,
}

impl TryFrom<TranslationResourceIdentityInput> for TranslationResourceIdentity {
    type Error = async_graphql::Error;

    fn try_from(value: TranslationResourceIdentityInput) -> Result<Self, Self::Error> {
        Ok(Self {
            owner_slug: parse_owner_slug(value.owner_slug)?,
            resource_kind: parse_resource_kind(value.resource_kind)?,
            resource_id: ResourceId::new(value.resource_id).map_err(input_error)?,
            subresource_id: value
                .subresource_id
                .map(ResourceId::new)
                .transpose()
                .map_err(input_error)?,
        })
    }
}

#[derive(InputObject)]
pub struct TranslationProposalValueInput {
    pub key: String,
    pub value: String,
}

#[derive(InputObject)]
pub struct ExportTranslationJobInput {
    pub job_id: Uuid,
    pub max_items: u16,
}

#[derive(InputObject)]
pub struct TranslationInterchangeArtifactsInput {
    pub job_id: Option<Uuid>,
    #[graphql(default = false)]
    pub include_expired: bool,
    #[graphql(default = 50)]
    pub limit: u16,
}

#[derive(InputObject)]
pub struct ReadTranslationInterchangeArtifactInput {
    pub artifact_id: Uuid,
}

#[derive(InputObject)]
pub struct CreateTranslationInterchangeExportArtifactInput {
    pub job_id: Uuid,
    pub max_items: u16,
    pub expires_in_seconds: u32,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct StoreTranslationInterchangeImportArtifactInput {
    pub job_id: Uuid,
    pub document_json: String,
    pub expires_in_seconds: u32,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct ProcessTranslationInterchangeImportArtifactInput {
    pub artifact_id: Uuid,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct ImportTranslationItemInput {
    pub schema_version: u16,
    pub job_id: Uuid,
    pub item_id: Uuid,
    pub identity: TranslationResourceIdentityInput,
    pub source_digest: String,
    pub values: Vec<TranslationProposalValueInput>,
    pub idempotency_key: String,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum TranslationProposalOriginInput {
    Manual,
    Import,
    Memory,
    Ai,
}

impl From<TranslationProposalOriginInput> for ProposalOrigin {
    fn from(value: TranslationProposalOriginInput) -> Self {
        match value {
            TranslationProposalOriginInput::Manual => Self::Manual,
            TranslationProposalOriginInput::Import => Self::Import,
            TranslationProposalOriginInput::Memory => Self::Memory,
            TranslationProposalOriginInput::Ai => Self::Ai,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum TranslationActorKindInput {
    User,
    Service,
}

#[derive(InputObject)]
pub struct TranslationActorInput {
    pub kind: TranslationActorKindInput,
    pub id: String,
}

impl From<TranslationActorInput> for PortActor {
    fn from(value: TranslationActorInput) -> Self {
        let kind = match value.kind {
            TranslationActorKindInput::User => PortActorKind::User,
            TranslationActorKindInput::Service => PortActorKind::Service,
        };
        Self { kind, id: value.id }
    }
}

#[derive(InputObject)]
pub struct ReplaceTranslationPolicyInput {
    pub expected_revision: i64,
    pub required_target_locales: Vec<String>,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct CreateTranslationJobInput {
    pub source_locale: String,
    pub target_locale: String,
    pub glossary: Option<TranslationGlossaryBindingInput>,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct TranslationGlossaryBindingInput {
    pub glossary_id: Uuid,
    pub revision: i64,
}

impl From<TranslationGlossaryBindingInput> for GlossaryBinding {
    fn from(value: TranslationGlossaryBindingInput) -> Self {
        Self {
            glossary_id: value.glossary_id,
            revision: value.revision,
        }
    }
}

#[derive(InputObject)]
pub struct TranslationGlossaryScopeInput {
    pub owner_slug: Option<String>,
    pub resource_kind: Option<String>,
    pub field_key: Option<String>,
}

impl TryFrom<TranslationGlossaryScopeInput> for GlossaryScope {
    type Error = async_graphql::Error;

    fn try_from(value: TranslationGlossaryScopeInput) -> Result<Self, Self::Error> {
        Ok(Self {
            owner_slug: value.owner_slug.map(parse_owner_slug).transpose()?,
            resource_kind: value.resource_kind.map(parse_resource_kind).transpose()?,
            field_key: value.field_key.map(parse_field_key).transpose()?,
        })
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum TranslationGlossaryTermPolicy {
    Preferred,
    Allowed,
    Forbidden,
    DoNotTranslate,
}

impl From<TranslationGlossaryTermPolicy> for GlossaryTermPolicy {
    fn from(value: TranslationGlossaryTermPolicy) -> Self {
        match value {
            TranslationGlossaryTermPolicy::Preferred => Self::Preferred,
            TranslationGlossaryTermPolicy::Allowed => Self::Allowed,
            TranslationGlossaryTermPolicy::Forbidden => Self::Forbidden,
            TranslationGlossaryTermPolicy::DoNotTranslate => Self::DoNotTranslate,
        }
    }
}

impl From<GlossaryTermPolicy> for TranslationGlossaryTermPolicy {
    fn from(value: GlossaryTermPolicy) -> Self {
        match value {
            GlossaryTermPolicy::Preferred => Self::Preferred,
            GlossaryTermPolicy::Allowed => Self::Allowed,
            GlossaryTermPolicy::Forbidden => Self::Forbidden,
            GlossaryTermPolicy::DoNotTranslate => Self::DoNotTranslate,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum TranslationGlossaryMatchKind {
    Exact,
    WholeWord,
    Substring,
}

impl From<TranslationGlossaryMatchKind> for GlossaryMatchKind {
    fn from(value: TranslationGlossaryMatchKind) -> Self {
        match value {
            TranslationGlossaryMatchKind::Exact => Self::Exact,
            TranslationGlossaryMatchKind::WholeWord => Self::WholeWord,
            TranslationGlossaryMatchKind::Substring => Self::Substring,
        }
    }
}

impl From<GlossaryMatchKind> for TranslationGlossaryMatchKind {
    fn from(value: GlossaryMatchKind) -> Self {
        match value {
            GlossaryMatchKind::Exact => Self::Exact,
            GlossaryMatchKind::WholeWord => Self::WholeWord,
            GlossaryMatchKind::Substring => Self::Substring,
        }
    }
}

#[derive(InputObject)]
pub struct TranslationGlossaryVariantInput {
    pub value: String,
    pub policy: TranslationGlossaryTermPolicy,
}

impl From<TranslationGlossaryVariantInput> for GlossaryVariant {
    fn from(value: TranslationGlossaryVariantInput) -> Self {
        Self {
            value: value.value,
            policy: value.policy.into(),
        }
    }
}

#[derive(InputObject)]
pub struct TranslationGlossaryConceptInput {
    pub concept_key: String,
    pub source_term: String,
    pub variants: Vec<TranslationGlossaryVariantInput>,
    pub match_kind: TranslationGlossaryMatchKind,
    pub case_sensitive: bool,
    pub notes: String,
}

impl From<TranslationGlossaryConceptInput> for GlossaryConcept {
    fn from(value: TranslationGlossaryConceptInput) -> Self {
        Self {
            concept_key: value.concept_key,
            source_term: value.source_term,
            variants: value.variants.into_iter().map(Into::into).collect(),
            match_kind: value.match_kind.into(),
            case_sensitive: value.case_sensitive,
            notes: value.notes,
        }
    }
}

#[derive(InputObject)]
pub struct CreateTranslationGlossaryInput {
    pub name: String,
    pub description: String,
    pub source_locale: String,
    pub target_locale: String,
    pub scope: TranslationGlossaryScopeInput,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct UpdateTranslationGlossaryInput {
    pub glossary_id: Uuid,
    pub expected_revision: i64,
    pub name: String,
    pub description: String,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct ReplaceTranslationGlossaryTermsInput {
    pub glossary_id: Uuid,
    pub expected_revision: i64,
    pub concepts: Vec<TranslationGlossaryConceptInput>,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct SetTranslationGlossaryActiveInput {
    pub glossary_id: Uuid,
    pub expected_revision: i64,
    pub is_active: bool,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct AddTranslationJobItemInput {
    pub job_id: Uuid,
    pub identity: TranslationResourceIdentityInput,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct TranslationReviewerQueueInput {
    pub job_id: Uuid,
    pub assignee: Option<TranslationActorInput>,
    #[graphql(default = false)]
    pub include_unassigned: bool,
    #[graphql(default = 50)]
    pub limit: u16,
}

#[derive(InputObject)]
pub struct TranslationReviewerWorkloadInput {
    pub job_id: Uuid,
}

#[derive(InputObject)]
pub struct TranslationWorkflowNotesInput {
    pub job_id: Uuid,
    pub item_id: Option<Uuid>,
    #[graphql(default = false)]
    pub include_resolved: bool,
    #[graphql(default = 50)]
    pub limit: u16,
}

#[derive(InputObject)]
pub struct CreateTranslationWorkflowNoteInput {
    pub job_id: Uuid,
    pub item_id: Option<Uuid>,
    pub body: String,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct ResolveTranslationWorkflowNoteInput {
    pub note_id: Uuid,
    pub expected_revision: i64,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct SaveTranslationProposalInput {
    pub item_id: Uuid,
    pub origin: TranslationProposalOriginInput,
    pub values: Vec<TranslationProposalValueInput>,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct GenerateMachineTranslationProposalInput {
    pub item_id: Uuid,
    pub field_keys: Vec<String>,
    pub minimum_memory_similarity_basis_points: u16,
    pub tone: Option<String>,
    pub domain: Option<String>,
    pub style: Option<String>,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct CancelMachineTranslationOperationInput {
    pub operation_id: Uuid,
    pub reason: String,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct MachineTranslationProposalRequestInput {
    pub item_id: Uuid,
    pub field_keys: Vec<String>,
    pub minimum_memory_similarity_basis_points: u16,
    pub tone: Option<String>,
    pub domain: Option<String>,
    pub style: Option<String>,
}

#[derive(InputObject)]
pub struct RecoverMachineTranslationOperationInput {
    pub operation_id: Uuid,
    pub expected_updated_at: DateTime<FixedOffset>,
    pub proposal: MachineTranslationProposalRequestInput,
    pub reason: String,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct TransitionTranslationProposalInput {
    pub item_id: Uuid,
    pub proposal_id: Uuid,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct AssignTranslationItemInput {
    pub item_id: Uuid,
    pub expected_revision: i64,
    pub assignee: TranslationActorInput,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct UnassignTranslationItemInput {
    pub item_id: Uuid,
    pub expected_revision: i64,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct CancelTranslationJobInput {
    pub job_id: Uuid,
    pub expected_revision: i64,
    pub reason: String,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct RetryTranslationItemInput {
    pub item_id: Uuid,
    pub expected_revision: i64,
    pub reason: String,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct RecoverTranslationApplyInput {
    pub operation_id: Uuid,
    pub expected_attempt_count: i64,
    pub reason: String,
    pub idempotency_key: String,
}

#[derive(SimpleObject)]
pub struct TranslationPolicy {
    pub tenant_id: Uuid,
    pub required_target_locales: Vec<String>,
    pub tenant_locale_policy_revision: i64,
    pub revision: i64,
    pub freshness: String,
    pub disabled_required_target_locales: Vec<String>,
}

impl From<TranslationPolicyRecord> for TranslationPolicy {
    fn from(value: TranslationPolicyRecord) -> Self {
        Self {
            tenant_id: value.tenant_id,
            required_target_locales: locale_strings(value.required_target_locales),
            tenant_locale_policy_revision: value.tenant_locale_policy_revision,
            revision: value.revision,
            freshness: match value.freshness {
                TranslationPolicyFreshness::Current => "current",
                TranslationPolicyFreshness::Stale => "stale",
            }
            .to_string(),
            disabled_required_target_locales: locale_strings(
                value.disabled_required_target_locales,
            ),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationTargetDescriptor {
    pub owner_slug: String,
    pub resource_kind: String,
    pub display_name: String,
    pub capabilities: Vec<String>,
    pub read_permission_floor: Vec<String>,
    pub apply_permission_floor: Vec<String>,
}

impl From<rustok_translation_targets::TranslationTargetProviderDescriptor>
    for TranslationTargetDescriptor
{
    fn from(value: rustok_translation_targets::TranslationTargetProviderDescriptor) -> Self {
        Self {
            owner_slug: value.owner_slug.to_string(),
            resource_kind: value.resource_kind.to_string(),
            display_name: value.display_name,
            capabilities: value
                .capabilities
                .into_iter()
                .map(|capability| {
                    serde_json::to_value(capability)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned))
                        .unwrap_or_default()
                })
                .collect(),
            read_permission_floor: value.read_permission_floor.into_iter().collect(),
            apply_permission_floor: value.apply_permission_floor.into_iter().collect(),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationJobProgress {
    pub job_id: Uuid,
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

impl From<JobProgressRecord> for TranslationJobProgress {
    fn from(value: JobProgressRecord) -> Self {
        Self {
            job_id: value.job_id,
            source_digest: value.source_digest,
            total_items: value.total_items,
            assigned_items: value.assigned_items,
            terminal_items: value.terminal_items,
            missing_items: value.missing_items,
            draft_items: value.draft_items,
            in_review_items: value.in_review_items,
            approved_items: value.approved_items,
            applying_items: value.applying_items,
            applied_items: value.applied_items,
            stale_items: value.stale_items,
            conflict_items: value.conflict_items,
            blocked_items: value.blocked_items,
            excluded_items: value.excluded_items,
            cancelled_items: value.cancelled_items,
            required_units: value.required_units,
            optional_units: value.optional_units,
            applied_required_units: value.applied_required_units,
            applied_optional_units: value.applied_optional_units,
            approved_required_units: value.approved_required_units,
            approved_optional_units: value.approved_optional_units,
            complete_resources: value.complete_resources,
            source_characters: value.source_characters,
            translated_characters: value.translated_characters,
            revision: value.revision,
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationProviderProgress {
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

impl From<ProviderProgressRecord> for TranslationProviderProgress {
    fn from(value: ProviderProgressRecord) -> Self {
        Self {
            owner_slug: value.owner_slug.to_string(),
            resource_kind: value.resource_kind.to_string(),
            source_locale: value.source_locale.as_str().to_string(),
            target_locale: value.target_locale.as_str().to_string(),
            required_units: value.facts.required_units,
            exact_required_units: value.facts.exact_required_units,
            optional_units: value.facts.optional_units,
            exact_optional_units: value.facts.exact_optional_units,
            resources: value.facts.resources,
            complete_resources: value.facts.complete_resources,
            owner_change_cursor: value
                .facts
                .owner_change_cursor
                .map(|cursor| cursor.to_string()),
            projected_cursor: value.projected_cursor.map(|cursor| cursor.to_string()),
            checkpoint_revision: value.checkpoint_revision,
            checkpoint_updated_at: value
                .checkpoint_updated_at
                .map(|timestamp| timestamp.to_rfc3339()),
            freshness: format!("{:?}", value.freshness).to_ascii_lowercase(),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationRequiredProviderProgress {
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
    pub targets: Vec<TranslationProviderProgress>,
}

impl From<RequiredProviderProgressRecord> for TranslationRequiredProviderProgress {
    fn from(value: RequiredProviderProgressRecord) -> Self {
        Self {
            owner_slug: value.owner_slug.to_string(),
            resource_kind: value.resource_kind.to_string(),
            source_locale: value.source_locale.as_str().to_string(),
            required_target_locales: locale_strings(value.required_target_locales),
            translation_policy_revision: value.translation_policy_revision,
            tenant_locale_policy_revision: value.tenant_locale_policy_revision,
            required_units: value.required_units,
            exact_required_units: value.exact_required_units,
            optional_units: value.optional_units,
            exact_optional_units: value.exact_optional_units,
            resource_locale_pairs: value.resource_locale_pairs,
            complete_resource_locale_pairs: value.complete_resource_locale_pairs,
            freshness: format!("{:?}", value.freshness).to_ascii_lowercase(),
            targets: value.targets.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationJob {
    pub id: Uuid,
    pub source_locale: String,
    pub target_locale: String,
    pub glossary: Option<TranslationGlossaryBinding>,
    pub status: String,
    pub revision: i64,
}

#[derive(SimpleObject)]
pub struct TranslationInterchangeIdentity {
    pub owner_slug: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub subresource_id: Option<String>,
}

impl From<TranslationResourceIdentity> for TranslationInterchangeIdentity {
    fn from(value: TranslationResourceIdentity) -> Self {
        Self {
            owner_slug: value.owner_slug.to_string(),
            resource_kind: value.resource_kind.to_string(),
            resource_id: value.resource_id.to_string(),
            subresource_id: value.subresource_id.map(|id| id.to_string()),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationInterchangeField {
    pub key: String,
    pub source_value: String,
    pub exact_target_value: Option<String>,
    pub proposed_value: Option<String>,
    pub source_hash: String,
    pub required: bool,
    pub max_characters: Option<u32>,
    pub protected_tokens: Vec<String>,
}

impl From<InterchangeField> for TranslationInterchangeField {
    fn from(value: InterchangeField) -> Self {
        Self {
            key: value.key.to_string(),
            source_value: value.source_value,
            exact_target_value: value.exact_target_value,
            proposed_value: value.proposed_value,
            source_hash: value.source_hash,
            required: value.required,
            max_characters: value.max_characters,
            protected_tokens: value.protected_tokens,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationInterchangeItem {
    pub item_id: Uuid,
    pub identity: TranslationInterchangeIdentity,
    pub source_digest: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub fields: Vec<TranslationInterchangeField>,
}

impl From<InterchangeItem> for TranslationInterchangeItem {
    fn from(value: InterchangeItem) -> Self {
        Self {
            item_id: value.item_id,
            identity: value.identity.into(),
            source_digest: value.source_digest,
            source_revision: value.source_revision.to_string(),
            target_revision: value.target_revision.map(|revision| revision.to_string()),
            fields: value.fields.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationInterchangeDocument {
    pub schema_version: u16,
    pub job_id: Uuid,
    pub source_locale: String,
    pub target_locale: String,
    pub items: Vec<TranslationInterchangeItem>,
}

impl From<InterchangeDocument> for TranslationInterchangeDocument {
    fn from(value: InterchangeDocument) -> Self {
        Self {
            schema_version: value.schema_version,
            job_id: value.job_id,
            source_locale: value.source_locale.as_str().to_string(),
            target_locale: value.target_locale.as_str().to_string(),
            items: value.items.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationInterchangeArtifactItemOutcome {
    pub item_id: Uuid,
    pub status: String,
}

impl From<InterchangeItemOutcome> for TranslationInterchangeArtifactItemOutcome {
    fn from(value: InterchangeItemOutcome) -> Self {
        Self {
            item_id: value.item_id,
            status: value.status,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationInterchangeConflictReport {
    pub total_items: u16,
    pub accepted_items: u16,
    pub conflict_items: u16,
    pub rejected_items: u16,
    pub outcomes: Vec<TranslationInterchangeArtifactItemOutcome>,
}

impl From<InterchangeConflictReport> for TranslationInterchangeConflictReport {
    fn from(value: InterchangeConflictReport) -> Self {
        Self {
            total_items: value.total_items,
            accepted_items: value.accepted_items,
            conflict_items: value.conflict_items,
            rejected_items: value.rejected_items,
            outcomes: value.outcomes.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationInterchangeArtifact {
    pub id: Uuid,
    pub job_id: Uuid,
    pub direction: String,
    pub status: String,
    pub content_length: u64,
    pub checksum_sha256: String,
    pub expires_at: DateTime<FixedOffset>,
    pub processed_at: Option<DateTime<FixedOffset>>,
    pub report: Option<TranslationInterchangeConflictReport>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

impl From<InterchangeArtifactRecord> for TranslationInterchangeArtifact {
    fn from(value: InterchangeArtifactRecord) -> Self {
        Self {
            id: value.id,
            job_id: value.job_id,
            direction: value.direction.as_str().to_string(),
            status: value.status.as_str().to_string(),
            content_length: value.content_length,
            checksum_sha256: value.checksum_sha256,
            expires_at: value.expires_at,
            processed_at: value.processed_at,
            report: value.report.map(Into::into),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationInterchangeArtifactContent {
    pub artifact: TranslationInterchangeArtifact,
    pub document: TranslationInterchangeDocument,
}

impl From<InterchangeArtifactContent> for TranslationInterchangeArtifactContent {
    fn from(value: InterchangeArtifactContent) -> Self {
        Self {
            artifact: value.artifact.into(),
            document: value.document.into(),
        }
    }
}

impl From<JobRecord> for TranslationJob {
    fn from(value: JobRecord) -> Self {
        Self {
            id: value.id,
            source_locale: value.source_locale.as_str().to_string(),
            target_locale: value.target_locale.as_str().to_string(),
            glossary: value.glossary.map(Into::into),
            status: value.status,
            revision: value.revision,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationGlossaryBinding {
    pub glossary_id: Uuid,
    pub revision: i64,
}

impl From<GlossaryBinding> for TranslationGlossaryBinding {
    fn from(value: GlossaryBinding) -> Self {
        Self {
            glossary_id: value.glossary_id,
            revision: value.revision,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationGlossaryScope {
    pub owner_slug: Option<String>,
    pub resource_kind: Option<String>,
    pub field_key: Option<String>,
}

impl From<GlossaryScope> for TranslationGlossaryScope {
    fn from(value: GlossaryScope) -> Self {
        Self {
            owner_slug: value.owner_slug.map(|value| value.to_string()),
            resource_kind: value.resource_kind.map(|value| value.to_string()),
            field_key: value.field_key.map(|value| value.to_string()),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationGlossaryVariant {
    pub value: String,
    pub policy: TranslationGlossaryTermPolicy,
}

impl From<GlossaryVariant> for TranslationGlossaryVariant {
    fn from(value: GlossaryVariant) -> Self {
        Self {
            value: value.value,
            policy: value.policy.into(),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationGlossaryConcept {
    pub concept_key: String,
    pub source_term: String,
    pub variants: Vec<TranslationGlossaryVariant>,
    pub match_kind: TranslationGlossaryMatchKind,
    pub case_sensitive: bool,
    pub notes: String,
}

impl From<GlossaryConcept> for TranslationGlossaryConcept {
    fn from(value: GlossaryConcept) -> Self {
        Self {
            concept_key: value.concept_key,
            source_term: value.source_term,
            variants: value.variants.into_iter().map(Into::into).collect(),
            match_kind: value.match_kind.into(),
            case_sensitive: value.case_sensitive,
            notes: value.notes,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationGlossarySummary {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub source_locale: String,
    pub target_locale: String,
    pub scope: TranslationGlossaryScope,
    pub is_active: bool,
    pub revision: i64,
}

impl From<GlossarySummaryRecord> for TranslationGlossarySummary {
    fn from(value: GlossarySummaryRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            description: value.description,
            source_locale: value.source_locale.as_str().to_string(),
            target_locale: value.target_locale.as_str().to_string(),
            scope: value.scope.into(),
            is_active: value.is_active,
            revision: value.revision,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationGlossary {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub source_locale: String,
    pub target_locale: String,
    pub scope: TranslationGlossaryScope,
    pub is_active: bool,
    pub revision: i64,
    pub concepts: Vec<TranslationGlossaryConcept>,
}

impl From<GlossaryRecord> for TranslationGlossary {
    fn from(value: GlossaryRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            description: value.description,
            source_locale: value.source_locale.as_str().to_string(),
            target_locale: value.target_locale.as_str().to_string(),
            scope: value.scope.into(),
            is_active: value.is_active,
            revision: value.revision,
            concepts: value.concepts.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationActor {
    pub kind: String,
    pub id: String,
}

impl From<PortActor> for TranslationActor {
    fn from(value: PortActor) -> Self {
        Self {
            kind: match value.kind {
                PortActorKind::User => "user",
                PortActorKind::Service => "service",
                PortActorKind::System => "system",
            }
            .to_string(),
            id: value.id,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationJobItem {
    pub id: Uuid,
    pub job_id: Uuid,
    pub owner_slug: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub subresource_id: Option<String>,
    pub status: String,
    pub assignee: Option<TranslationActor>,
    pub source_digest: String,
    pub revision: i64,
}

impl From<JobItemRecord> for TranslationJobItem {
    fn from(value: JobItemRecord) -> Self {
        Self {
            id: value.id,
            job_id: value.job_id,
            owner_slug: value.identity.owner_slug.to_string(),
            resource_kind: value.identity.resource_kind.to_string(),
            resource_id: value.identity.resource_id.to_string(),
            subresource_id: value.identity.subresource_id.map(|id| id.to_string()),
            status: value.status,
            assignee: value.assignee.map(Into::into),
            source_digest: value.source_digest,
            revision: value.revision,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationReviewerQueueItem {
    pub item: TranslationJobItem,
    pub proposal_id: Uuid,
    pub proposal_revision: i64,
    pub submitted_at: DateTime<FixedOffset>,
}

impl From<ReviewerQueueRecord> for TranslationReviewerQueueItem {
    fn from(value: ReviewerQueueRecord) -> Self {
        Self {
            item: value.item.into(),
            proposal_id: value.proposal_id,
            proposal_revision: value.proposal_revision,
            submitted_at: value.submitted_at,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationReviewerWorkload {
    pub job_id: Uuid,
    pub assignee: Option<TranslationActor>,
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

impl From<ReviewerWorkloadRecord> for TranslationReviewerWorkload {
    fn from(value: ReviewerWorkloadRecord) -> Self {
        Self {
            job_id: value.job_id,
            assignee: value.assignee.map(Into::into),
            open_items: value.open_items,
            missing_items: value.missing_items,
            draft_items: value.draft_items,
            in_review_items: value.in_review_items,
            approved_items: value.approved_items,
            applying_items: value.applying_items,
            rebase_required_items: value.rebase_required_items,
            blocked_items: value.blocked_items,
            source_characters: value.source_characters,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationWorkflowNote {
    pub id: Uuid,
    pub job_id: Uuid,
    pub item_id: Option<Uuid>,
    pub body: String,
    pub author: TranslationActor,
    pub revision: i64,
    pub resolved_at: Option<DateTime<FixedOffset>>,
    pub resolved_by: Option<TranslationActor>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

impl From<WorkflowNoteRecord> for TranslationWorkflowNote {
    fn from(value: WorkflowNoteRecord) -> Self {
        Self {
            id: value.id,
            job_id: value.job_id,
            item_id: value.item_id,
            body: value.body,
            author: value.author.into(),
            revision: value.revision,
            resolved_at: value.resolved_at,
            resolved_by: value.resolved_by.map(Into::into),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationProposalValue {
    pub key: String,
    pub value: String,
    pub expected_source_hash: String,
}

#[derive(SimpleObject)]
pub struct TranslationQaIssue {
    pub field: Option<String>,
    pub severity: String,
    pub code: String,
    pub message: String,
}

#[derive(SimpleObject)]
pub struct TranslationProposal {
    pub id: Uuid,
    pub item_id: Uuid,
    pub proposal_revision: i64,
    pub origin: String,
    pub values: Vec<TranslationProposalValue>,
    pub qa_issues: Vec<TranslationQaIssue>,
    pub qa_accepted: bool,
    pub status: String,
    pub approval_receipt_id: Option<String>,
}

impl From<ProposalRecord> for TranslationProposal {
    fn from(value: ProposalRecord) -> Self {
        Self {
            id: value.id,
            item_id: value.item_id,
            proposal_revision: value.proposal_revision,
            origin: format!("{:?}", value.origin).to_ascii_lowercase(),
            values: value
                .values
                .into_iter()
                .map(|field| TranslationProposalValue {
                    key: field.key.to_string(),
                    value: field.value,
                    expected_source_hash: field.expected_source_hash,
                })
                .collect(),
            qa_issues: value
                .qa_issues
                .into_iter()
                .map(|issue| TranslationQaIssue {
                    field: issue.field.map(|field| field.to_string()),
                    severity: format!("{:?}", issue.severity).to_ascii_lowercase(),
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
            qa_accepted: value.qa_accepted,
            status: value.status,
            approval_receipt_id: value.approval_receipt_id,
        }
    }
}

#[derive(SimpleObject)]
pub struct MachineTranslationAttempt {
    pub attempt: u16,
    pub provider_profile_id: String,
    pub provider_slug: String,
    pub model: String,
    pub fallback: bool,
}

#[derive(SimpleObject)]
pub struct MachineTranslationUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost_minor_units: u64,
    pub currency_code: String,
    pub price_snapshot_digest: String,
}

#[derive(SimpleObject)]
pub struct MachineTranslationEstimate {
    pub input_tokens_upper_bound: u64,
    pub output_tokens_upper_bound: u64,
    pub attempts_upper_bound: u16,
    pub cost_minor_units_upper_bound: u64,
    pub currency_code: String,
    pub price_snapshot_digest: String,
    pub review_required: bool,
}

impl From<crate::MachineTranslationEstimate> for MachineTranslationEstimate {
    fn from(value: crate::MachineTranslationEstimate) -> Self {
        Self {
            input_tokens_upper_bound: value.input_tokens_upper_bound,
            output_tokens_upper_bound: value.output_tokens_upper_bound,
            attempts_upper_bound: value.attempts_upper_bound,
            cost_minor_units_upper_bound: value.cost_minor_units_upper_bound,
            currency_code: value.currency_code,
            price_snapshot_digest: value.price_snapshot_digest,
            review_required: value.review_required,
        }
    }
}

#[derive(SimpleObject)]
pub struct MachineTranslationDiagnostic {
    pub code: String,
    pub blocking: bool,
    pub unit_id: Option<String>,
}

#[derive(SimpleObject)]
pub struct MachineTranslationProposal {
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
    pub attempts: Vec<MachineTranslationAttempt>,
    pub usage: MachineTranslationUsage,
    pub diagnostics: Vec<MachineTranslationDiagnostic>,
    pub review_required: bool,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

impl From<MachineProposalRecord> for MachineTranslationProposal {
    fn from(value: MachineProposalRecord) -> Self {
        Self {
            operation_id: value.operation_id,
            item_id: value.item_id,
            proposal_id: value.proposal_id,
            adapter_slug: value.adapter_slug,
            provider_slug: value.provider_slug,
            provider_policy_digest: value.provider_policy_digest,
            machine_request_digest: value.machine_request_digest,
            glossary_revision: value.glossary_revision,
            glossary_digest: value.glossary_digest,
            memory_digest: value.memory_digest,
            execution_id: value.execution_id,
            execution_request_digest: value.execution_request_digest,
            prompt_policy_digest: value.prompt_policy_digest,
            attempts: value
                .attempts
                .into_iter()
                .map(|attempt| MachineTranslationAttempt {
                    attempt: attempt.attempt,
                    provider_profile_id: attempt.provider_profile_id,
                    provider_slug: attempt.provider_slug,
                    model: attempt.model,
                    fallback: attempt.fallback,
                })
                .collect(),
            usage: MachineTranslationUsage {
                input_tokens: value.usage.input_tokens,
                output_tokens: value.usage.output_tokens,
                total_tokens: value.usage.total_tokens,
                cost_minor_units: value.usage.cost_minor_units,
                currency_code: value.usage.currency_code,
                price_snapshot_digest: value.usage.price_snapshot_digest,
            },
            diagnostics: value
                .diagnostics
                .into_iter()
                .map(|diagnostic| MachineTranslationDiagnostic {
                    code: diagnostic.code,
                    blocking: diagnostic.blocking,
                    unit_id: diagnostic.unit_id,
                })
                .collect(),
            review_required: value.review_required,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(SimpleObject)]
pub struct MachineTranslationCancellation {
    pub cancellation_id: Uuid,
    pub operation_id: Uuid,
    pub status: String,
    pub provider_execution_id: Option<String>,
    pub provider_status: String,
    pub provider_error_code: Option<String>,
    pub provider_observed_at: DateTime<FixedOffset>,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(SimpleObject)]
pub struct MachineTranslationOperationStatus {
    pub operation_id: Uuid,
    pub item_id: Uuid,
    pub status: String,
    pub provider_execution_id: Option<String>,
    pub provider_status: String,
    pub provider_error_code: Option<String>,
    pub updated_at: DateTime<FixedOffset>,
}

impl From<crate::MachineOperationStatusRecord> for MachineTranslationOperationStatus {
    fn from(value: crate::MachineOperationStatusRecord) -> Self {
        Self {
            operation_id: value.operation_id,
            item_id: value.item_id,
            status: value.status,
            provider_execution_id: value.provider_execution_id,
            provider_status: value.provider_status,
            provider_error_code: value.provider_error_code,
            updated_at: value.updated_at,
        }
    }
}

impl From<MachineCancellationRecord> for MachineTranslationCancellation {
    fn from(value: MachineCancellationRecord) -> Self {
        Self {
            cancellation_id: value.cancellation_id,
            operation_id: value.operation_id,
            status: value.status,
            provider_execution_id: value.provider_execution_id,
            provider_status: value.provider_status,
            provider_error_code: value.provider_error_code,
            provider_observed_at: value.provider_observed_at,
            created_at: value.created_at,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationApply {
    pub operation_id: Uuid,
    pub item_id: Uuid,
    pub proposal_id: Uuid,
    pub provider_receipt_id: String,
    pub resource_revision: String,
    pub target_revision: String,
    pub applied_field_keys: Vec<String>,
}

impl From<ApplyRecord> for TranslationApply {
    fn from(value: ApplyRecord) -> Self {
        Self {
            operation_id: value.operation_id,
            item_id: value.item_id,
            proposal_id: value.proposal_id,
            provider_receipt_id: value.provider_receipt_id,
            resource_revision: value.resource_revision.to_string(),
            target_revision: value.target_revision.to_string(),
            applied_field_keys: value
                .applied_field_keys
                .into_iter()
                .map(|key| key.to_string())
                .collect(),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationAssignment {
    pub operation_id: Uuid,
    pub item_id: Uuid,
    pub assignee: Option<TranslationActor>,
    pub item_revision: i64,
}

impl From<AssignmentRecord> for TranslationAssignment {
    fn from(value: AssignmentRecord) -> Self {
        Self {
            operation_id: value.operation_id,
            item_id: value.item_id,
            assignee: value.assignee.map(Into::into),
            item_revision: value.item_revision,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationCancellation {
    pub cancellation_id: Uuid,
    pub job_id: Uuid,
    pub job_revision: i64,
    pub cancelled_item_count: u64,
}

impl From<CancellationRecord> for TranslationCancellation {
    fn from(value: CancellationRecord) -> Self {
        Self {
            cancellation_id: value.cancellation_id,
            job_id: value.job_id,
            job_revision: value.job_revision,
            cancelled_item_count: value.cancelled_item_count,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationRetry {
    pub retry_id: Uuid,
    pub item_id: Uuid,
    pub item_revision: i64,
    pub status: String,
}

impl From<RetryRecord> for TranslationRetry {
    fn from(value: RetryRecord) -> Self {
        Self {
            retry_id: value.retry_id,
            item_id: value.item_id,
            item_revision: value.item_revision,
            status: value.status,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationInventorySync {
    pub observed_resources: u64,
    pub checkpoint: Option<String>,
    pub checkpoint_revision: i64,
}

impl From<TranslationInventorySyncResult> for TranslationInventorySync {
    fn from(value: TranslationInventorySyncResult) -> Self {
        Self {
            observed_resources: value.observed_resources,
            checkpoint: value.checkpoint.map(|cursor| cursor.to_string()),
            checkpoint_revision: value.checkpoint_revision,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationInventoryRebuild {
    pub observed_resources: u64,
    pub checkpoint: Option<String>,
    pub checkpoint_revision: i64,
}

impl From<TranslationInventoryRebuildResult> for TranslationInventoryRebuild {
    fn from(value: TranslationInventoryRebuildResult) -> Self {
        Self {
            observed_resources: value.observed_resources,
            checkpoint: value.checkpoint.map(|cursor| cursor.to_string()),
            checkpoint_revision: value.checkpoint_revision,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum TranslationMemoryRetentionPolicy {
    OwnerLifecycle,
    RetainUntil,
    LegalHold,
}

impl From<TranslationMemoryRetentionPolicy> for MemoryRetentionPolicy {
    fn from(value: TranslationMemoryRetentionPolicy) -> Self {
        match value {
            TranslationMemoryRetentionPolicy::OwnerLifecycle => Self::OwnerLifecycle,
            TranslationMemoryRetentionPolicy::RetainUntil => Self::RetainUntil,
            TranslationMemoryRetentionPolicy::LegalHold => Self::LegalHold,
        }
    }
}

impl From<MemoryRetentionPolicy> for TranslationMemoryRetentionPolicy {
    fn from(value: MemoryRetentionPolicy) -> Self {
        match value {
            MemoryRetentionPolicy::OwnerLifecycle => Self::OwnerLifecycle,
            MemoryRetentionPolicy::RetainUntil => Self::RetainUntil,
            MemoryRetentionPolicy::LegalHold => Self::LegalHold,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum TranslationMemoryMatchKind {
    Exact,
    ContextualFuzzy,
    Fuzzy,
}

impl From<MemoryMatchKind> for TranslationMemoryMatchKind {
    fn from(value: MemoryMatchKind) -> Self {
        match value {
            MemoryMatchKind::Exact => Self::Exact,
            MemoryMatchKind::ContextualFuzzy => Self::ContextualFuzzy,
            MemoryMatchKind::Fuzzy => Self::Fuzzy,
        }
    }
}

#[derive(InputObject)]
pub struct LookupTranslationMemoryInput {
    pub source_locale: String,
    pub target_locale: String,
    pub identity: TranslationResourceIdentityInput,
    pub field_key: String,
    pub source_text: String,
    pub minimum_similarity_basis_points: u16,
    pub limit: u16,
}

#[derive(InputObject)]
pub struct SetTranslationMemoryRetentionInput {
    pub entry_id: Uuid,
    pub expected_revision: i64,
    pub policy: TranslationMemoryRetentionPolicy,
    pub retain_until: Option<DateTime<FixedOffset>>,
    pub idempotency_key: String,
}

#[derive(InputObject)]
pub struct TransitionTranslationMemoryEntryInput {
    pub entry_id: Uuid,
    pub expected_revision: i64,
    pub idempotency_key: String,
}

#[derive(SimpleObject)]
pub struct TranslationMemoryEntry {
    pub id: Uuid,
    pub tenant_id: Uuid,
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
    pub proposal_id: Uuid,
    pub apply_receipt_id: Uuid,
    pub retention_policy: TranslationMemoryRetentionPolicy,
    pub retain_until: Option<DateTime<FixedOffset>>,
    pub tombstoned_at: Option<DateTime<FixedOffset>>,
    pub revision: i64,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

impl From<MemoryEntryRecord> for TranslationMemoryEntry {
    fn from(value: MemoryEntryRecord) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            source_locale: value.source_locale,
            target_locale: value.target_locale,
            owner_slug: value.owner_slug,
            resource_kind: value.resource_kind,
            resource_id: value.resource_id,
            subresource_id: value.subresource_id,
            field_key: value.field_key,
            source_text: value.source_text,
            target_text: value.target_text,
            source_hash: value.source_hash,
            target_hash: value.target_hash,
            context_fingerprint: value.context_fingerprint,
            segmentation_version: value.segmentation_version,
            origin: value.origin,
            quality_state: value.quality_state,
            reviewer_actor_kind: value.reviewer_actor_kind,
            reviewer_actor_id: value.reviewer_actor_id,
            proposal_id: value.proposal_id,
            apply_receipt_id: value.apply_receipt_id,
            retention_policy: value.retention_policy.into(),
            retain_until: value.retain_until,
            tombstoned_at: value.tombstoned_at,
            revision: value.revision,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationMemoryMatchEvidence {
    pub kind: TranslationMemoryMatchKind,
    pub source_exact: bool,
    pub context_match: bool,
    pub base_similarity_basis_points: u16,
    pub context_bonus_basis_points: u16,
    pub final_similarity_basis_points: u16,
    pub segmentation_version: String,
}

impl From<MemoryMatchEvidence> for TranslationMemoryMatchEvidence {
    fn from(value: MemoryMatchEvidence) -> Self {
        Self {
            kind: value.kind.into(),
            source_exact: value.source_exact,
            context_match: value.context_match,
            base_similarity_basis_points: value.base_similarity_basis_points,
            context_bonus_basis_points: value.context_bonus_basis_points,
            final_similarity_basis_points: value.final_similarity_basis_points,
            segmentation_version: value.segmentation_version,
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationMemorySuggestion {
    pub entry_id: Uuid,
    pub source_text: String,
    pub target_text: String,
    pub source_hash: String,
    pub owner_slug: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub field_key: String,
    pub origin: String,
    pub proposal_id: Uuid,
    pub apply_receipt_id: Uuid,
    pub evidence: TranslationMemoryMatchEvidence,
}

impl From<MemorySuggestion> for TranslationMemorySuggestion {
    fn from(value: MemorySuggestion) -> Self {
        Self {
            entry_id: value.entry_id,
            source_text: value.source_text,
            target_text: value.target_text,
            source_hash: value.source_hash,
            owner_slug: value.owner_slug,
            resource_kind: value.resource_kind,
            resource_id: value.resource_id,
            field_key: value.field_key,
            origin: value.origin,
            proposal_id: value.proposal_id,
            apply_receipt_id: value.apply_receipt_id,
            evidence: value.evidence.into(),
        }
    }
}

#[derive(SimpleObject)]
pub struct TranslationMemoryMutation {
    pub entry_id: Uuid,
    pub revision: i64,
    pub state: String,
    pub retention_policy: TranslationMemoryRetentionPolicy,
    pub retain_until: Option<DateTime<FixedOffset>>,
    pub tombstoned_at: Option<DateTime<FixedOffset>>,
}

impl From<MemoryMutationRecord> for TranslationMemoryMutation {
    fn from(value: MemoryMutationRecord) -> Self {
        Self {
            entry_id: value.entry_id,
            revision: value.revision,
            state: value.state,
            retention_policy: value.retention_policy.into(),
            retain_until: value.retain_until,
            tombstoned_at: value.tombstoned_at,
        }
    }
}

pub(crate) fn parse_owner_slug(value: String) -> async_graphql::Result<OwnerSlug> {
    OwnerSlug::new(value).map_err(input_error)
}

pub(crate) fn parse_resource_kind(value: String) -> async_graphql::Result<ResourceKind> {
    ResourceKind::new(value).map_err(input_error)
}

pub(crate) fn parse_field_key(value: String) -> async_graphql::Result<FieldKey> {
    FieldKey::new(value).map_err(input_error)
}

pub(crate) fn parse_locale(value: String) -> async_graphql::Result<TenantLocale> {
    TenantLocale::new(value).map_err(input_error)
}

pub(crate) fn parse_interchange_document(
    value: &str,
) -> async_graphql::Result<InterchangeDocument> {
    crate::parse_artifact_document(value).map_err(input_error)
}

fn locale_strings(locales: Vec<TenantLocale>) -> Vec<String> {
    locales
        .into_iter()
        .map(|locale| locale.as_str().to_string())
        .collect()
}

fn input_error(error: impl std::fmt::Display) -> async_graphql::Error {
    <FieldError as GraphQLError>::bad_user_input(error.to_string().as_str())
}
