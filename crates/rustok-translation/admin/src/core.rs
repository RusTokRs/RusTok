use std::fmt;

use rustok_ui_core::{UiRouteQueryIntent, normalize_ui_text, parse_ui_csv};
use uuid::Uuid;

use crate::model::{
    GlossaryBinding, GlossaryConcept, GlossaryScope, MemoryRetentionPolicy, ProposalOrigin,
    ProposalValueInput, TranslationAdminOperation, TranslationAdminResponse,
    TranslationAdminTransportContext, TranslationResourceIdentity,
};

pub const TAB_QUERY_KEY: &str = "tab";
pub const GLOSSARY_ID_QUERY_KEY: &str = "glossary_id";
pub const MEMORY_ENTRY_ID_QUERY_KEY: &str = "memory_entry_id";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationAdminTab {
    Overview,
    Jobs,
    Glossaries,
    Memory,
    Inventory,
    Workflow,
}

impl TranslationAdminTab {
    pub const ALL: [Self; 6] = [
        Self::Overview,
        Self::Jobs,
        Self::Glossaries,
        Self::Memory,
        Self::Inventory,
        Self::Workflow,
    ];

    pub const fn query_value(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Jobs => "jobs",
            Self::Glossaries => "glossaries",
            Self::Memory => "memory",
            Self::Inventory => "inventory",
            Self::Workflow => "workflow",
        }
    }
}

pub fn tab_from_query(value: Option<&str>) -> TranslationAdminTab {
    match value {
        Some("jobs") => TranslationAdminTab::Jobs,
        Some("glossaries") => TranslationAdminTab::Glossaries,
        Some("memory") => TranslationAdminTab::Memory,
        Some("inventory") => TranslationAdminTab::Inventory,
        Some("workflow") => TranslationAdminTab::Workflow,
        _ => TranslationAdminTab::Overview,
    }
}

pub fn glossary_selection_intent(glossary_id: Option<&str>) -> UiRouteQueryIntent {
    match glossary_id.and_then(normalize_ui_text) {
        Some(glossary_id) => UiRouteQueryIntent::replace(GLOSSARY_ID_QUERY_KEY, glossary_id),
        None => UiRouteQueryIntent::clear(GLOSSARY_ID_QUERY_KEY),
    }
}

pub fn memory_selection_intent(memory_entry_id: Option<&str>) -> UiRouteQueryIntent {
    match memory_entry_id.and_then(normalize_ui_text) {
        Some(memory_entry_id) => {
            UiRouteQueryIntent::replace(MEMORY_ENTRY_ID_QUERY_KEY, memory_entry_id)
        }
        None => UiRouteQueryIntent::clear(MEMORY_ENTRY_ID_QUERY_KEY),
    }
}

pub fn tab_query_intent(tab: TranslationAdminTab) -> UiRouteQueryIntent {
    if tab == TranslationAdminTab::Overview {
        UiRouteQueryIntent::clear(TAB_QUERY_KEY)
    } else {
        UiRouteQueryIntent::replace(TAB_QUERY_KEY, tab.query_value())
    }
}

pub fn transport_context(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> TranslationAdminTransportContext {
    TranslationAdminTransportContext {
        token,
        tenant_slug,
        locale,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInputError {
    pub field: &'static str,
    pub message: String,
}

impl fmt::Display for CommandInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.field, self.message)
    }
}

pub fn new_idempotency_key(operation: &str) -> String {
    format!("translation-admin:{operation}:{}", Uuid::new_v4())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptFact {
    pub label_key: &'static str,
    pub fallback_label: &'static str,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationReceiptViewModel {
    pub title_key: &'static str,
    pub fallback_title: &'static str,
    pub facts: Vec<ReceiptFact>,
}

pub fn operation_receipt_view_model(
    response: &TranslationAdminResponse,
) -> OperationReceiptViewModel {
    let fact = |label_key, fallback_label, value| ReceiptFact {
        label_key,
        fallback_label,
        value,
    };

    match response {
        TranslationAdminResponse::Policy(policy) => OperationReceiptViewModel {
            title_key: "translation.receipt.policy",
            fallback_title: "Policy updated",
            facts: vec![
                fact(
                    "translation.field.revision",
                    "Revision",
                    policy.revision.to_string(),
                ),
                fact(
                    "translation.field.locales",
                    "Required locales",
                    policy.required_target_locales.join(", "),
                ),
                fact(
                    "translation.field.freshness",
                    "Freshness",
                    policy.freshness.clone(),
                ),
            ],
        },
        TranslationAdminResponse::Targets(targets) => OperationReceiptViewModel {
            title_key: "translation.receipt.targets",
            fallback_title: "Targets loaded",
            facts: vec![fact(
                "translation.field.targets",
                "Targets",
                targets.len().to_string(),
            )],
        },
        TranslationAdminResponse::Glossaries(glossaries) => OperationReceiptViewModel {
            title_key: "translation.receipt.glossaries",
            fallback_title: "Glossaries loaded",
            facts: vec![fact(
                "translation.field.glossaries",
                "Glossaries",
                glossaries.len().to_string(),
            )],
        },
        TranslationAdminResponse::Glossary(glossary) => OperationReceiptViewModel {
            title_key: "translation.receipt.glossary",
            fallback_title: "Glossary updated",
            facts: vec![
                fact(
                    "translation.field.glossary",
                    "Glossary",
                    glossary.name.clone(),
                ),
                fact(
                    "translation.field.revision",
                    "Revision",
                    glossary.revision.to_string(),
                ),
                fact(
                    "translation.field.concepts",
                    "Concepts",
                    glossary.concepts.len().to_string(),
                ),
                fact(
                    "translation.field.status",
                    "Status",
                    if glossary.is_active {
                        "active".to_string()
                    } else {
                        "inactive".to_string()
                    },
                ),
            ],
        },
        TranslationAdminResponse::MemoryEntries(entries) => OperationReceiptViewModel {
            title_key: "translation.receipt.memoryEntries",
            fallback_title: "Memory entries loaded",
            facts: vec![fact(
                "translation.field.memoryEntries",
                "Memory entries",
                entries.len().to_string(),
            )],
        },
        TranslationAdminResponse::MemoryEntry(entry) => OperationReceiptViewModel {
            title_key: "translation.receipt.memoryEntry",
            fallback_title: "Memory entry loaded",
            facts: vec![
                fact(
                    "translation.field.memoryEntryId",
                    "Memory entry ID",
                    entry.id.clone(),
                ),
                fact(
                    "translation.field.localePair",
                    "Locale pair",
                    format!("{} -> {}", entry.source_locale, entry.target_locale),
                ),
                fact(
                    "translation.field.revision",
                    "Revision",
                    entry.revision.to_string(),
                ),
            ],
        },
        TranslationAdminResponse::MemorySuggestions(suggestions) => OperationReceiptViewModel {
            title_key: "translation.receipt.memorySuggestions",
            fallback_title: "Memory suggestions loaded",
            facts: vec![fact(
                "translation.field.suggestions",
                "Suggestions",
                suggestions.len().to_string(),
            )],
        },
        TranslationAdminResponse::MemoryMutation(mutation) => OperationReceiptViewModel {
            title_key: "translation.receipt.memoryMutation",
            fallback_title: "Memory lifecycle updated",
            facts: vec![
                fact(
                    "translation.field.memoryEntryId",
                    "Memory entry ID",
                    mutation.entry_id.clone(),
                ),
                fact("translation.field.status", "Status", mutation.state.clone()),
                fact(
                    "translation.field.revision",
                    "Revision",
                    mutation.revision.to_string(),
                ),
            ],
        },
        TranslationAdminResponse::JobProgress(progress) => OperationReceiptViewModel {
            title_key: "translation.receipt.jobProgress",
            fallback_title: "Job progress",
            facts: vec![
                fact("translation.field.jobId", "Job ID", progress.job_id.clone()),
                fact(
                    "translation.field.totalItems",
                    "Total items",
                    progress.total_items.to_string(),
                ),
                fact(
                    "translation.field.appliedItems",
                    "Applied items",
                    progress.applied_items.to_string(),
                ),
                fact(
                    "translation.field.blockedItems",
                    "Blocked items",
                    progress.blocked_items.to_string(),
                ),
                fact(
                    "translation.field.revision",
                    "Revision",
                    progress.revision.to_string(),
                ),
            ],
        },
        TranslationAdminResponse::ProviderProgress(progress) => OperationReceiptViewModel {
            title_key: "translation.receipt.providerProgress",
            fallback_title: "Provider progress",
            facts: vec![
                fact(
                    "translation.field.provider",
                    "Provider",
                    format!("{}/{}", progress.owner_slug, progress.resource_kind),
                ),
                fact(
                    "translation.field.localePair",
                    "Locale pair",
                    format!("{} → {}", progress.source_locale, progress.target_locale),
                ),
                fact(
                    "translation.field.completeResources",
                    "Complete resources",
                    format!("{}/{}", progress.complete_resources, progress.resources),
                ),
                fact(
                    "translation.field.freshness",
                    "Freshness",
                    progress.freshness.clone(),
                ),
            ],
        },
        TranslationAdminResponse::RequiredProviderProgress(progress) => OperationReceiptViewModel {
            title_key: "translation.receipt.requiredProgress",
            fallback_title: "Required-target progress",
            facts: vec![
                fact(
                    "translation.field.provider",
                    "Provider",
                    format!("{}/{}", progress.owner_slug, progress.resource_kind),
                ),
                fact(
                    "translation.field.locales",
                    "Required locales",
                    progress.required_target_locales.join(", "),
                ),
                fact(
                    "translation.field.completeResources",
                    "Complete resource-locale pairs",
                    format!(
                        "{}/{}",
                        progress.complete_resource_locale_pairs, progress.resource_locale_pairs
                    ),
                ),
                fact(
                    "translation.field.freshness",
                    "Freshness",
                    progress.freshness.clone(),
                ),
            ],
        },
        TranslationAdminResponse::Job(job) => OperationReceiptViewModel {
            title_key: "translation.receipt.job",
            fallback_title: "Job created",
            facts: vec![
                fact("translation.field.jobId", "Job ID", job.id.clone()),
                fact(
                    "translation.field.localePair",
                    "Locale pair",
                    format!("{} → {}", job.source_locale, job.target_locale),
                ),
                fact("translation.field.status", "Status", job.status.clone()),
                fact(
                    "translation.field.revision",
                    "Revision",
                    job.revision.to_string(),
                ),
            ],
        },
        TranslationAdminResponse::Item(item) => OperationReceiptViewModel {
            title_key: "translation.receipt.item",
            fallback_title: "Item updated",
            facts: vec![
                fact("translation.field.itemId", "Item ID", item.id.clone()),
                fact("translation.field.jobId", "Job ID", item.job_id.clone()),
                fact(
                    "translation.field.resource",
                    "Resource",
                    format!(
                        "{}/{}/{}",
                        item.identity.owner_slug,
                        item.identity.resource_kind,
                        item.identity.resource_id
                    ),
                ),
                fact("translation.field.status", "Status", item.status.clone()),
                fact(
                    "translation.field.revision",
                    "Revision",
                    item.revision.to_string(),
                ),
            ],
        },
        TranslationAdminResponse::Proposal(proposal) => OperationReceiptViewModel {
            title_key: "translation.receipt.proposal",
            fallback_title: "Proposal updated",
            facts: vec![
                fact(
                    "translation.field.proposalId",
                    "Proposal ID",
                    proposal.id.clone(),
                ),
                fact(
                    "translation.field.itemId",
                    "Item ID",
                    proposal.item_id.clone(),
                ),
                fact(
                    "translation.field.status",
                    "Status",
                    proposal.status.clone(),
                ),
                fact(
                    "translation.field.qaIssues",
                    "QA issues",
                    proposal.qa_issues.len().to_string(),
                ),
            ],
        },
        TranslationAdminResponse::MachineProposal(proposal) => OperationReceiptViewModel {
            title_key: "translation.receipt.machineProposal",
            fallback_title: "Machine proposal generated",
            facts: vec![
                fact(
                    "translation.field.proposalId",
                    "Proposal ID",
                    proposal.proposal_id.clone(),
                ),
                fact(
                    "translation.field.provider",
                    "Provider",
                    proposal.provider_slug.clone(),
                ),
                fact(
                    "translation.field.executionId",
                    "Execution ID",
                    proposal.execution_id.clone(),
                ),
                fact(
                    "translation.field.reviewRequired",
                    "Review required",
                    proposal.review_required.to_string(),
                ),
            ],
        },
        TranslationAdminResponse::MachineCancellation(cancellation) => OperationReceiptViewModel {
            title_key: "translation.receipt.machineCancellation",
            fallback_title: "Machine operation cancelled",
            facts: vec![
                fact(
                    "translation.field.operationId",
                    "Operation ID",
                    cancellation.operation_id.clone(),
                ),
                fact(
                    "translation.field.status",
                    "Status",
                    cancellation.status.clone(),
                ),
                fact(
                    "translation.field.providerStatus",
                    "Provider status",
                    cancellation.provider_status.clone(),
                ),
                fact(
                    "translation.field.providerExecutionId",
                    "Provider execution ID",
                    cancellation
                        .provider_execution_id
                        .clone()
                        .unwrap_or_else(|| "—".to_string()),
                ),
            ],
        },
        TranslationAdminResponse::Apply(apply) => OperationReceiptViewModel {
            title_key: "translation.receipt.apply",
            fallback_title: "Translation applied",
            facts: vec![
                fact(
                    "translation.field.operationId",
                    "Operation ID",
                    apply.operation_id.clone(),
                ),
                fact(
                    "translation.field.providerReceipt",
                    "Provider receipt",
                    apply.provider_receipt_id.clone(),
                ),
                fact(
                    "translation.field.targetRevision",
                    "Target revision",
                    apply.target_revision.clone(),
                ),
            ],
        },
        TranslationAdminResponse::Assignment(assignment) => OperationReceiptViewModel {
            title_key: "translation.receipt.assignment",
            fallback_title: "Assignment updated",
            facts: vec![
                fact(
                    "translation.field.itemId",
                    "Item ID",
                    assignment.item_id.clone(),
                ),
                fact(
                    "translation.field.revision",
                    "Revision",
                    assignment.item_revision.to_string(),
                ),
            ],
        },
        TranslationAdminResponse::Cancellation(cancellation) => OperationReceiptViewModel {
            title_key: "translation.receipt.cancellation",
            fallback_title: "Job cancelled",
            facts: vec![
                fact(
                    "translation.field.jobId",
                    "Job ID",
                    cancellation.job_id.clone(),
                ),
                fact(
                    "translation.field.cancelledItems",
                    "Cancelled items",
                    cancellation.cancelled_item_count.to_string(),
                ),
            ],
        },
        TranslationAdminResponse::Retry(retry) => OperationReceiptViewModel {
            title_key: "translation.receipt.retry",
            fallback_title: "Item retry accepted",
            facts: vec![
                fact("translation.field.itemId", "Item ID", retry.item_id.clone()),
                fact("translation.field.status", "Status", retry.status.clone()),
            ],
        },
        TranslationAdminResponse::Inventory(inventory) => OperationReceiptViewModel {
            title_key: "translation.receipt.inventory",
            fallback_title: "Inventory updated",
            facts: vec![
                fact(
                    "translation.field.observedResources",
                    "Observed resources",
                    inventory.observed_resources.to_string(),
                ),
                fact(
                    "translation.field.checkpoint",
                    "Checkpoint",
                    inventory
                        .checkpoint
                        .clone()
                        .unwrap_or_else(|| "none".to_string()),
                ),
                fact(
                    "translation.field.revision",
                    "Checkpoint revision",
                    inventory.checkpoint_revision.to_string(),
                ),
            ],
        },
    }
}

pub fn create_job_with_glossary_operation(
    source_locale: &str,
    target_locale: &str,
    glossary_id: &str,
    glossary_revision: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let glossary_id = normalize_ui_text(glossary_id);
    let glossary_revision = normalize_ui_text(glossary_revision);
    let glossary = match (glossary_id, glossary_revision) {
        (None, None) => None,
        (Some(glossary_id), Some(revision)) => Some(GlossaryBinding {
            glossary_id,
            revision: parse_positive_i64("glossary_revision", &revision)?,
        }),
        (None, Some(_)) => {
            return Err(CommandInputError {
                field: "glossary_id",
                message: "value is required when glossary_revision is set".to_string(),
            });
        }
        (Some(_), None) => {
            return Err(CommandInputError {
                field: "glossary_revision",
                message: "value is required when glossary_id is set".to_string(),
            });
        }
    };
    Ok(TranslationAdminOperation::CreateJob {
        source_locale: required_text("source_locale", source_locale)?,
        target_locale: required_text("target_locale", target_locale)?,
        glossary,
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

pub fn create_glossary_operation(
    name: &str,
    description: &str,
    source_locale: &str,
    target_locale: &str,
    owner_slug: &str,
    resource_kind: &str,
    field_key: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::CreateGlossary {
        name: required_text("name", name)?,
        description: description.trim().to_string(),
        source_locale: required_text("source_locale", source_locale)?,
        target_locale: required_text("target_locale", target_locale)?,
        scope: glossary_scope(owner_slug, resource_kind, field_key)?,
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

pub fn update_glossary_operation(
    glossary_id: &str,
    expected_revision: &str,
    name: &str,
    description: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::UpdateGlossary {
        glossary_id: required_text("glossary_id", glossary_id)?,
        expected_revision: parse_positive_i64("expected_revision", expected_revision)?,
        name: required_text("name", name)?,
        description: description.trim().to_string(),
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

pub fn replace_glossary_terms_operation(
    glossary_id: &str,
    expected_revision: &str,
    concepts_json: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let concepts =
        serde_json::from_str::<Vec<GlossaryConcept>>(concepts_json).map_err(|error| {
            CommandInputError {
                field: "concepts_json",
                message: format!("must be a glossary concept array: {error}"),
            }
        })?;
    Ok(TranslationAdminOperation::ReplaceGlossaryTerms {
        glossary_id: required_text("glossary_id", glossary_id)?,
        expected_revision: parse_positive_i64("expected_revision", expected_revision)?,
        concepts,
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

pub fn set_glossary_active_operation(
    glossary_id: &str,
    expected_revision: &str,
    is_active: bool,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::SetGlossaryActive {
        glossary_id: required_text("glossary_id", glossary_id)?,
        expected_revision: parse_positive_i64("expected_revision", expected_revision)?,
        is_active,
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

fn glossary_scope(
    owner_slug: &str,
    resource_kind: &str,
    field_key: &str,
) -> Result<GlossaryScope, CommandInputError> {
    let owner_slug = normalize_ui_text(owner_slug);
    let resource_kind = normalize_ui_text(resource_kind);
    let field_key = normalize_ui_text(field_key);
    if resource_kind.is_some() && owner_slug.is_none() {
        return Err(CommandInputError {
            field: "owner_slug",
            message: "value is required when resource_kind is set".to_string(),
        });
    }
    if field_key.is_some() && resource_kind.is_none() {
        return Err(CommandInputError {
            field: "resource_kind",
            message: "value is required when field_key is set".to_string(),
        });
    }
    Ok(GlossaryScope {
        owner_slug,
        resource_kind,
        field_key,
    })
}

pub fn list_memory_entries_operation(
    source_locale: &str,
    target_locale: &str,
    include_tombstoned: bool,
    limit: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::ListMemoryEntries {
        source_locale: normalize_ui_text(source_locale),
        target_locale: normalize_ui_text(target_locale),
        include_tombstoned,
        limit: parse_u16("limit", limit)?,
    })
}

pub fn read_memory_entry_operation(
    entry_id: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::ReadMemoryEntry {
        entry_id: required_text("memory_entry_id", entry_id)?,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn lookup_memory_operation(
    source_locale: &str,
    target_locale: &str,
    owner_slug: &str,
    resource_kind: &str,
    resource_id: &str,
    subresource_id: &str,
    field_key: &str,
    source_text: &str,
    minimum_similarity_basis_points: &str,
    limit: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let minimum_similarity_basis_points = parse_nonnegative_u16(
        "minimum_similarity_basis_points",
        minimum_similarity_basis_points,
    )?;
    if minimum_similarity_basis_points > 10_000 {
        return Err(CommandInputError {
            field: "minimum_similarity_basis_points",
            message: "must be between 0 and 10000".to_string(),
        });
    }

    Ok(TranslationAdminOperation::LookupMemory {
        source_locale: required_text("source_locale", source_locale)?,
        target_locale: required_text("target_locale", target_locale)?,
        identity: TranslationResourceIdentity {
            owner_slug: required_text("owner_slug", owner_slug)?,
            resource_kind: required_text("resource_kind", resource_kind)?,
            resource_id: required_text("resource_id", resource_id)?,
            subresource_id: normalize_ui_text(subresource_id),
        },
        field_key: required_text("field_key", field_key)?,
        source_text: required_value("source_text", source_text)?,
        minimum_similarity_basis_points,
        limit: parse_u16("limit", limit)?,
    })
}

pub fn set_memory_retention_operation(
    entry_id: &str,
    expected_revision: &str,
    policy: &str,
    retain_until: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let policy = match policy {
        "owner_lifecycle" => MemoryRetentionPolicy::OwnerLifecycle,
        "retain_until" => MemoryRetentionPolicy::RetainUntil,
        "legal_hold" => MemoryRetentionPolicy::LegalHold,
        _ => {
            return Err(CommandInputError {
                field: "retention_policy",
                message: "must be owner_lifecycle, retain_until, or legal_hold".to_string(),
            });
        }
    };
    let retain_until = normalize_ui_text(retain_until);
    match (policy, retain_until.as_ref()) {
        (MemoryRetentionPolicy::RetainUntil, None) => {
            return Err(CommandInputError {
                field: "retain_until",
                message: "value is required for retain_until policy".to_string(),
            });
        }
        (MemoryRetentionPolicy::OwnerLifecycle | MemoryRetentionPolicy::LegalHold, Some(_)) => {
            return Err(CommandInputError {
                field: "retain_until",
                message: "value is valid only for retain_until policy".to_string(),
            });
        }
        _ => {}
    }

    Ok(TranslationAdminOperation::SetMemoryRetention {
        entry_id: required_text("memory_entry_id", entry_id)?,
        expected_revision: parse_positive_i64("expected_revision", expected_revision)?,
        policy,
        retain_until,
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

pub fn tombstone_memory_entry_operation(
    entry_id: &str,
    expected_revision: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::TombstoneMemoryEntry {
        entry_id: required_text("memory_entry_id", entry_id)?,
        expected_revision: parse_positive_i64("expected_revision", expected_revision)?,
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

pub fn purge_memory_entry_operation(
    entry_id: &str,
    expected_revision: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::PurgeMemoryEntry {
        entry_id: required_text("memory_entry_id", entry_id)?,
        expected_revision: parse_positive_i64("expected_revision", expected_revision)?,
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

pub fn read_job_progress_operation(
    job_id: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::ReadJobProgress {
        job_id: required_text("job_id", job_id)?,
    })
}

pub fn rebuild_job_progress_operation(
    job_id: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::RebuildJobProgress {
        job_id: required_text("job_id", job_id)?,
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

pub fn replace_policy_operation(
    expected_revision: &str,
    required_target_locales: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let locales = parse_ui_csv(required_target_locales);
    if locales.is_empty() {
        return Err(CommandInputError {
            field: "required_target_locales",
            message: "at least one comma-separated locale is required".to_string(),
        });
    }

    Ok(TranslationAdminOperation::ReplacePolicy {
        expected_revision: parse_i64("expected_revision", expected_revision)?,
        required_target_locales: locales,
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

pub fn read_provider_progress_operation(
    owner_slug: &str,
    resource_kind: &str,
    source_locale: &str,
    target_locale: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::ReadProviderProgress {
        owner_slug: required_text("owner_slug", owner_slug)?,
        resource_kind: required_text("resource_kind", resource_kind)?,
        source_locale: required_text("source_locale", source_locale)?,
        target_locale: required_text("target_locale", target_locale)?,
    })
}

pub fn read_required_provider_progress_operation(
    owner_slug: &str,
    resource_kind: &str,
    source_locale: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::ReadRequiredProviderProgress {
        owner_slug: required_text("owner_slug", owner_slug)?,
        resource_kind: required_text("resource_kind", resource_kind)?,
        source_locale: required_text("source_locale", source_locale)?,
    })
}

pub fn sync_inventory_operation(
    owner_slug: &str,
    resource_kind: &str,
    limit: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::SyncProviderInventory {
        owner_slug: required_text("owner_slug", owner_slug)?,
        resource_kind: required_text("resource_kind", resource_kind)?,
        limit: parse_u16("limit", limit)?,
    })
}

pub fn rebuild_inventory_operation(
    owner_slug: &str,
    resource_kind: &str,
    source_locale: &str,
    target_locale: &str,
    page_size: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::RebuildProviderInventory {
        owner_slug: required_text("owner_slug", owner_slug)?,
        resource_kind: required_text("resource_kind", resource_kind)?,
        source_locale: required_text("source_locale", source_locale)?,
        target_locale: required_text("target_locale", target_locale)?,
        page_size: parse_u16("page_size", page_size)?,
    })
}

pub fn add_item_operation(
    job_id: &str,
    owner_slug: &str,
    resource_kind: &str,
    resource_id: &str,
    subresource_id: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::AddItem {
        job_id: required_text("job_id", job_id)?,
        identity: TranslationResourceIdentity {
            owner_slug: required_text("owner_slug", owner_slug)?,
            resource_kind: required_text("resource_kind", resource_kind)?,
            resource_id: required_text("resource_id", resource_id)?,
            subresource_id: normalize_ui_text(subresource_id),
        },
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

pub fn save_proposal_operation(
    item_id: &str,
    field_key: &str,
    value: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::SaveProposal {
        item_id: required_text("item_id", item_id)?,
        origin: ProposalOrigin::Manual,
        values: vec![ProposalValueInput {
            key: required_text("field_key", field_key)?,
            value: required_value("value", value)?,
        }],
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalCommand {
    Submit,
    Approve,
    Apply,
}

pub fn proposal_command_operation(
    command: ProposalCommand,
    item_id: &str,
    proposal_id: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let item_id = required_text("item_id", item_id)?;
    let proposal_id = required_text("proposal_id", proposal_id)?;
    let idempotency_key = required_text("idempotency_key", idempotency_key)?;

    Ok(match command {
        ProposalCommand::Submit => TranslationAdminOperation::SubmitProposal {
            item_id,
            proposal_id,
            idempotency_key,
        },
        ProposalCommand::Approve => TranslationAdminOperation::ApproveProposal {
            item_id,
            proposal_id,
            idempotency_key,
        },
        ProposalCommand::Apply => TranslationAdminOperation::ApplyProposal {
            item_id,
            proposal_id,
            idempotency_key,
        },
    })
}

fn required_text(field: &'static str, value: &str) -> Result<String, CommandInputError> {
    normalize_ui_text(value).ok_or_else(|| CommandInputError {
        field,
        message: "value is required".to_string(),
    })
}

fn required_value(field: &'static str, value: &str) -> Result<String, CommandInputError> {
    if value.trim().is_empty() {
        Err(CommandInputError {
            field,
            message: "value is required".to_string(),
        })
    } else {
        Ok(value.to_string())
    }
}

fn parse_i64(field: &'static str, value: &str) -> Result<i64, CommandInputError> {
    required_text(field, value)?
        .parse()
        .map_err(|_| CommandInputError {
            field,
            message: "must be a signed integer".to_string(),
        })
}

fn parse_positive_i64(field: &'static str, value: &str) -> Result<i64, CommandInputError> {
    let parsed = parse_i64(field, value)?;
    if parsed < 1 {
        return Err(CommandInputError {
            field,
            message: "must be greater than zero".to_string(),
        });
    }
    Ok(parsed)
}

fn parse_u16(field: &'static str, value: &str) -> Result<u16, CommandInputError> {
    let parsed = required_text(field, value)?
        .parse()
        .map_err(|_| CommandInputError {
            field,
            message: "must be an integer between 1 and 65535".to_string(),
        })?;
    if parsed == 0 {
        return Err(CommandInputError {
            field,
            message: "must be greater than zero".to_string(),
        });
    }
    Ok(parsed)
}

fn parse_nonnegative_u16(field: &'static str, value: &str) -> Result<u16, CommandInputError> {
    required_text(field, value)?
        .parse()
        .map_err(|_| CommandInputError {
            field,
            message: "must be an integer between 0 and 65535".to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_contract_is_strict_and_uses_snake_case_query_key() {
        assert_eq!(TAB_QUERY_KEY, "tab");
        assert_eq!(GLOSSARY_ID_QUERY_KEY, "glossary_id");
        assert_eq!(MEMORY_ENTRY_ID_QUERY_KEY, "memory_entry_id");
        assert_eq!(TranslationAdminTab::ALL.len(), 6);
        assert_eq!(
            tab_from_query(Some("inventory")),
            TranslationAdminTab::Inventory
        );
        assert_eq!(tab_from_query(Some("memory")), TranslationAdminTab::Memory);
        assert_eq!(
            tab_from_query(Some("Inventory")),
            TranslationAdminTab::Overview
        );
        assert_eq!(
            tab_query_intent(TranslationAdminTab::Jobs),
            UiRouteQueryIntent::replace("tab", "jobs")
        );
        assert_eq!(
            glossary_selection_intent(Some(" glossary-1 ")),
            UiRouteQueryIntent::replace("glossary_id", "glossary-1")
        );
        assert_eq!(
            glossary_selection_intent(None),
            UiRouteQueryIntent::clear("glossary_id")
        );
        assert_eq!(
            memory_selection_intent(Some(" memory-1 ")),
            UiRouteQueryIntent::replace("memory_entry_id", "memory-1")
        );
        assert_eq!(
            memory_selection_intent(None),
            UiRouteQueryIntent::clear("memory_entry_id")
        );
    }

    #[test]
    fn policy_command_normalizes_locales_and_preserves_caller_identity() {
        let operation = replace_policy_operation("7", " de, fr ,de-CH ", "policy-key").unwrap();
        assert_eq!(
            operation,
            TranslationAdminOperation::ReplacePolicy {
                expected_revision: 7,
                required_target_locales: vec![
                    "de".to_string(),
                    "fr".to_string(),
                    "de-CH".to_string()
                ],
                idempotency_key: "policy-key".to_string(),
            }
        );
    }

    #[test]
    fn item_command_rejects_missing_owner_identity() {
        let error =
            add_item_operation("job-1", "", "asset", "asset-1", "", "item-key").unwrap_err();
        assert_eq!(error.field, "owner_slug");
    }

    #[test]
    fn inventory_bounds_fail_before_transport() {
        let error = sync_inventory_operation("media", "asset", "0").unwrap_err();
        assert_eq!(error.field, "limit");
    }

    #[test]
    fn job_glossary_binding_is_all_or_nothing_and_revisioned() {
        let operation =
            create_job_with_glossary_operation("en", "de", "glossary-1", "7", "job-key").unwrap();
        assert_eq!(
            operation,
            TranslationAdminOperation::CreateJob {
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                glossary: Some(GlossaryBinding {
                    glossary_id: "glossary-1".to_string(),
                    revision: 7,
                }),
                idempotency_key: "job-key".to_string(),
            }
        );
        assert_eq!(
            create_job_with_glossary_operation("en", "de", "glossary-1", "", "job-key")
                .unwrap_err()
                .field,
            "glossary_revision"
        );
    }

    #[test]
    fn glossary_terms_json_is_typed_before_transport() {
        let operation = replace_glossary_terms_operation(
            "glossary-1",
            "3",
            r#"[{"conceptKey":"checkout","sourceTerm":"Checkout","variants":[{"value":"Kasse","policy":"PREFERRED"}],"matchKind":"WHOLE_WORD","caseSensitive":false,"notes":""}]"#,
            "terms-key",
        )
        .unwrap();
        assert!(matches!(
            operation,
            TranslationAdminOperation::ReplaceGlossaryTerms {
                expected_revision: 3,
                concepts,
                ..
            } if concepts.len() == 1
        ));
        assert_eq!(
            replace_glossary_terms_operation("glossary-1", "3", "{}", "terms-key")
                .unwrap_err()
                .field,
            "concepts_json"
        );
    }

    #[test]
    fn memory_lookup_is_typed_and_bounded_before_transport() {
        let operation = lookup_memory_operation(
            "en",
            "de",
            "media",
            "asset",
            "asset-1",
            "",
            "alt",
            "Source copy",
            "8500",
            "10",
        )
        .unwrap();
        assert!(matches!(
            operation,
            TranslationAdminOperation::LookupMemory {
                minimum_similarity_basis_points: 8500,
                limit: 10,
                ..
            }
        ));
        assert_eq!(
            lookup_memory_operation(
                "en",
                "de",
                "media",
                "asset",
                "asset-1",
                "",
                "alt",
                "Source copy",
                "10001",
                "10",
            )
            .unwrap_err()
            .field,
            "minimum_similarity_basis_points"
        );
    }

    #[test]
    fn memory_retention_requires_the_matching_timestamp_shape() {
        let operation =
            set_memory_retention_operation("entry-1", "3", "legal_hold", "", "retention-key")
                .unwrap();
        assert!(matches!(
            operation,
            TranslationAdminOperation::SetMemoryRetention {
                policy: MemoryRetentionPolicy::LegalHold,
                retain_until: None,
                ..
            }
        ));
        assert_eq!(
            set_memory_retention_operation("entry-1", "3", "retain_until", "", "retention-key",)
                .unwrap_err()
                .field,
            "retain_until"
        );
        assert_eq!(
            set_memory_retention_operation(
                "entry-1",
                "3",
                "owner_lifecycle",
                "2030-01-01T00:00:00Z",
                "retention-key",
            )
            .unwrap_err()
            .field,
            "retain_until"
        );
    }
}
