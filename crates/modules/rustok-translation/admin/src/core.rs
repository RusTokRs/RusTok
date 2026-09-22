use std::fmt;

use rustok_ui_core::{UiRouteQueryIntent, normalize_ui_text, parse_ui_csv};
use uuid::Uuid;

use crate::model::{
    Actor, ActorKind, GlossaryBinding, GlossaryConcept, GlossaryScope, InterchangeDocument,
    MemoryRetentionPolicy, ProposalOrigin, ProposalValueInput, TranslationAdminOperation,
    TranslationAdminResponse, TranslationAdminTransportContext, TranslationResourceIdentity,
};

pub const TAB_QUERY_KEY: &str = "tab";
pub const GLOSSARY_ID_QUERY_KEY: &str = "glossary_id";
pub const MEMORY_ENTRY_ID_QUERY_KEY: &str = "memory_entry_id";
const MAX_INTERCHANGE_ARTIFACT_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_INTERCHANGE_ARTIFACT_ITEMS: u16 = 200;
const MIN_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS: u32 = 300;
const MAX_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS: u32 = 7 * 24 * 60 * 60;

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

    pub const fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Jobs => 1,
            Self::Glossaries => 2,
            Self::Memory => 3,
            Self::Inventory => 4,
            Self::Workflow => 5,
        }
    }

    pub const fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub const fn previous(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
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
        TranslationAdminResponse::ReviewerQueue(queue) => OperationReceiptViewModel {
            title_key: "translation.receipt.reviewerQueue",
            fallback_title: "Reviewer queue",
            facts: vec![fact(
                "translation.field.queueItems",
                "Queue items",
                queue.len().to_string(),
            )],
        },
        TranslationAdminResponse::ReviewerWorkloads(workloads) => OperationReceiptViewModel {
            title_key: "translation.receipt.reviewerWorkload",
            fallback_title: "Reviewer workload",
            facts: vec![fact(
                "translation.field.reviewers",
                "Reviewers",
                workloads.len().to_string(),
            )],
        },
        TranslationAdminResponse::WorkflowNotes(notes) => OperationReceiptViewModel {
            title_key: "translation.receipt.workflowNotes",
            fallback_title: "Workflow notes",
            facts: vec![fact(
                "translation.field.workflowNotes",
                "Workflow notes",
                notes.len().to_string(),
            )],
        },
        TranslationAdminResponse::WorkflowNote(note) => OperationReceiptViewModel {
            title_key: "translation.receipt.workflowNote",
            fallback_title: "Workflow note updated",
            facts: vec![
                fact(
                    "translation.field.workflowNoteId",
                    "Workflow note ID",
                    note.id.clone(),
                ),
                fact("translation.field.jobId", "Job ID", note.job_id.clone()),
                fact(
                    "translation.field.revision",
                    "Revision",
                    note.revision.to_string(),
                ),
                fact(
                    "translation.field.status",
                    "Status",
                    if note.resolved_at.is_some() {
                        "resolved".to_string()
                    } else {
                        "open".to_string()
                    },
                ),
            ],
        },
        TranslationAdminResponse::InterchangeDocument(document) => OperationReceiptViewModel {
            title_key: "translation.receipt.interchangeExport",
            fallback_title: "Interchange document exported",
            facts: vec![
                fact("translation.field.jobId", "Job ID", document.job_id.clone()),
                fact(
                    "translation.field.interchangeSchema",
                    "Schema version",
                    document.schema_version.to_string(),
                ),
                fact(
                    "translation.field.totalItems",
                    "Total items",
                    document.items.len().to_string(),
                ),
            ],
        },
        TranslationAdminResponse::InterchangeArtifacts(artifacts) => OperationReceiptViewModel {
            title_key: "translation.receipt.interchangeArtifacts",
            fallback_title: "Interchange artifacts loaded",
            facts: vec![fact(
                "translation.field.interchangeArtifacts",
                "Interchange artifacts",
                artifacts.len().to_string(),
            )],
        },
        TranslationAdminResponse::InterchangeArtifact(artifact) => OperationReceiptViewModel {
            title_key: "translation.receipt.interchangeArtifact",
            fallback_title: "Interchange artifact updated",
            facts: interchange_artifact_facts(&fact, artifact),
        },
        TranslationAdminResponse::InterchangeArtifactContent(content) => {
            OperationReceiptViewModel {
                title_key: "translation.receipt.interchangeArtifact",
                fallback_title: "Interchange artifact loaded",
                facts: interchange_artifact_facts(&fact, &content.artifact),
            }
        }
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
        TranslationAdminResponse::MachineEstimate(estimate) => OperationReceiptViewModel {
            title_key: "translation.receipt.machineEstimate",
            fallback_title: "Machine translation estimated",
            facts: vec![
                fact(
                    "translation.field.maximumCost",
                    "Maximum cost",
                    format!(
                        "{} {}",
                        estimate.cost_minor_units_upper_bound, estimate.currency_code
                    ),
                ),
                fact(
                    "translation.field.maximumAttempts",
                    "Maximum attempts",
                    estimate.attempts_upper_bound.to_string(),
                ),
                fact(
                    "translation.field.reviewRequired",
                    "Review required",
                    estimate.review_required.to_string(),
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
        TranslationAdminResponse::MachineOperationStatus(status) => OperationReceiptViewModel {
            title_key: "translation.receipt.machineOperationStatus",
            fallback_title: "Machine operation status",
            facts: vec![
                fact(
                    "translation.field.operationId",
                    "Operation ID",
                    status.operation_id.clone(),
                ),
                fact("translation.field.status", "Status", status.status.clone()),
                fact(
                    "translation.field.providerStatus",
                    "Provider status",
                    status.provider_status.clone(),
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

fn interchange_artifact_facts(
    fact: &impl Fn(&'static str, &'static str, String) -> ReceiptFact,
    artifact: &crate::model::InterchangeArtifact,
) -> Vec<ReceiptFact> {
    vec![
        fact(
            "translation.field.interchangeArtifactId",
            "Artifact ID",
            artifact.id.clone(),
        ),
        fact("translation.field.jobId", "Job ID", artifact.job_id.clone()),
        fact(
            "translation.field.status",
            "Status",
            artifact.status.clone(),
        ),
        fact(
            "translation.field.interchangeDirection",
            "Direction",
            artifact.direction.clone(),
        ),
    ]
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

pub struct CreateGlossaryOperationInput<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub source_locale: &'a str,
    pub target_locale: &'a str,
    pub owner_slug: &'a str,
    pub resource_kind: &'a str,
    pub field_key: &'a str,
    pub idempotency_key: &'a str,
}

pub fn create_glossary_operation(
    input: CreateGlossaryOperationInput<'_>,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::CreateGlossary {
        name: required_text("name", input.name)?,
        description: input.description.trim().to_string(),
        source_locale: required_text("source_locale", input.source_locale)?,
        target_locale: required_text("target_locale", input.target_locale)?,
        scope: glossary_scope(input.owner_slug, input.resource_kind, input.field_key)?,
        idempotency_key: required_text("idempotency_key", input.idempotency_key)?,
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

#[derive(Debug, Clone, Copy)]
pub struct MemoryLookupInput<'a> {
    pub source_locale: &'a str,
    pub target_locale: &'a str,
    pub owner_slug: &'a str,
    pub resource_kind: &'a str,
    pub resource_id: &'a str,
    pub subresource_id: &'a str,
    pub field_key: &'a str,
    pub source_text: &'a str,
    pub minimum_similarity_basis_points: &'a str,
    pub limit: &'a str,
}

pub fn lookup_memory_operation(
    input: MemoryLookupInput<'_>,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let minimum_similarity_basis_points = parse_nonnegative_u16(
        "minimum_similarity_basis_points",
        input.minimum_similarity_basis_points,
    )?;
    if minimum_similarity_basis_points > 10_000 {
        return Err(CommandInputError {
            field: "minimum_similarity_basis_points",
            message: "must be between 0 and 10000".to_string(),
        });
    }

    Ok(TranslationAdminOperation::LookupMemory {
        source_locale: required_text("source_locale", input.source_locale)?,
        target_locale: required_text("target_locale", input.target_locale)?,
        identity: TranslationResourceIdentity {
            owner_slug: required_text("owner_slug", input.owner_slug)?,
            resource_kind: required_text("resource_kind", input.resource_kind)?,
            resource_id: required_text("resource_id", input.resource_id)?,
            subresource_id: normalize_ui_text(input.subresource_id),
        },
        field_key: required_text("field_key", input.field_key)?,
        source_text: required_value("source_text", input.source_text)?,
        minimum_similarity_basis_points,
        limit: parse_u16("limit", input.limit)?,
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

#[derive(Debug, Clone, Copy)]
pub struct ReviewerQueueOperationInput<'a> {
    pub job_id: &'a str,
    pub assignee_kind: &'a str,
    pub assignee_id: &'a str,
    pub include_unassigned: bool,
    pub limit: &'a str,
}

pub fn read_reviewer_queue_operation(
    input: ReviewerQueueOperationInput<'_>,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let limit = parse_u16("reviewer_queue_limit", input.limit)?;
    if limit == 0 || limit > 200 {
        return Err(CommandInputError {
            field: "reviewer_queue_limit",
            message: "must be between 1 and 200".to_string(),
        });
    }
    let assignee_kind = normalize_ui_text(input.assignee_kind);
    let assignee_id = normalize_ui_text(input.assignee_id);
    let assignee = match (assignee_kind.as_deref(), assignee_id) {
        (None, None) => None,
        (Some(kind), Some(id)) => Some(Actor {
            kind: parse_actor_kind(kind)?,
            id,
        }),
        (None, Some(_)) => {
            return Err(CommandInputError {
                field: "assignee_kind",
                message: "is required when assignee_id is set".to_string(),
            });
        }
        (Some(_), None) => {
            return Err(CommandInputError {
                field: "assignee_id",
                message: "is required when assignee_kind is set".to_string(),
            });
        }
    };

    Ok(TranslationAdminOperation::ReadReviewerQueue {
        job_id: required_text("job_id", input.job_id)?,
        assignee,
        include_unassigned: input.include_unassigned,
        limit,
    })
}

pub fn read_reviewer_workload_operation(
    job_id: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::ReadReviewerWorkload {
        job_id: required_text("job_id", job_id)?,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct WorkflowNotesOperationInput<'a> {
    pub job_id: &'a str,
    pub item_id: &'a str,
    pub include_resolved: bool,
    pub limit: &'a str,
}

pub fn list_workflow_notes_operation(
    input: WorkflowNotesOperationInput<'_>,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let limit = parse_u16("workflow_note_limit", input.limit)?;
    if limit == 0 || limit > 200 {
        return Err(CommandInputError {
            field: "workflow_note_limit",
            message: "must be between 1 and 200".to_string(),
        });
    }
    Ok(TranslationAdminOperation::ListWorkflowNotes {
        job_id: required_text("job_id", input.job_id)?,
        item_id: normalize_ui_text(input.item_id),
        include_resolved: input.include_resolved,
        limit,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct CreateWorkflowNoteOperationInput<'a> {
    pub job_id: &'a str,
    pub item_id: &'a str,
    pub body: &'a str,
    pub idempotency_key: &'a str,
}

pub fn create_workflow_note_operation(
    input: CreateWorkflowNoteOperationInput<'_>,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let body = required_text("workflow_note_body", input.body)?;
    if body.chars().count() > 4_000 {
        return Err(CommandInputError {
            field: "workflow_note_body",
            message: "must not exceed 4000 characters".to_string(),
        });
    }
    Ok(TranslationAdminOperation::CreateWorkflowNote {
        job_id: required_text("job_id", input.job_id)?,
        item_id: normalize_ui_text(input.item_id),
        body,
        idempotency_key: required_text("idempotency_key", input.idempotency_key)?,
    })
}

pub fn resolve_workflow_note_operation(
    note_id: &str,
    expected_revision: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let expected_revision = parse_i64("expected_revision", expected_revision)?;
    if expected_revision < 0 {
        return Err(CommandInputError {
            field: "expected_revision",
            message: "must be zero or greater".to_string(),
        });
    }
    Ok(TranslationAdminOperation::ResolveWorkflowNote {
        note_id: required_text("workflow_note_id", note_id)?,
        expected_revision,
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

pub fn export_job_operation(
    job_id: &str,
    max_items: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let max_items = parse_u16("max_items", max_items)?;
    if max_items > 200 {
        return Err(CommandInputError {
            field: "max_items",
            message: "must be between 1 and 200".to_string(),
        });
    }
    Ok(TranslationAdminOperation::ExportJob {
        job_id: required_text("job_id", job_id)?,
        max_items,
    })
}

pub struct ListInterchangeArtifactsOperationInput<'a> {
    pub job_id: Option<&'a str>,
    pub include_expired: bool,
    pub limit: &'a str,
}

pub fn list_interchange_artifacts_operation(
    input: ListInterchangeArtifactsOperationInput<'_>,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let limit = parse_u16("interchange_artifact_limit", input.limit)?;
    if limit == 0 || limit > MAX_INTERCHANGE_ARTIFACT_ITEMS {
        return Err(CommandInputError {
            field: "interchange_artifact_limit",
            message: format!("must be between 1 and {MAX_INTERCHANGE_ARTIFACT_ITEMS}"),
        });
    }
    Ok(TranslationAdminOperation::ListInterchangeArtifacts {
        job_id: input.job_id.and_then(normalize_ui_text),
        include_expired: input.include_expired,
        limit,
    })
}

pub fn read_interchange_artifact_operation(
    artifact_id: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::ReadInterchangeArtifact {
        artifact_id: required_text("interchange_artifact_id", artifact_id)?,
    })
}

pub fn create_interchange_export_artifact_operation(
    job_id: &str,
    max_items: &str,
    expires_in_seconds: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let max_items = parse_u16("interchange_artifact_max_items", max_items)?;
    if max_items == 0 || max_items > MAX_INTERCHANGE_ARTIFACT_ITEMS {
        return Err(CommandInputError {
            field: "interchange_artifact_max_items",
            message: format!("must be between 1 and {MAX_INTERCHANGE_ARTIFACT_ITEMS}"),
        });
    }
    Ok(TranslationAdminOperation::CreateInterchangeExportArtifact {
        job_id: required_text("job_id", job_id)?,
        max_items,
        expires_in_seconds: parse_interchange_artifact_expiry(expires_in_seconds)?,
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

pub fn store_interchange_import_artifact_operation(
    job_id: &str,
    document_json: &str,
    expires_in_seconds: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let job_id = required_text("job_id", job_id)?;
    let document_json = canonical_interchange_artifact_document(document_json, &job_id)?;
    Ok(TranslationAdminOperation::StoreInterchangeImportArtifact {
        job_id,
        document_json,
        expires_in_seconds: parse_interchange_artifact_expiry(expires_in_seconds)?,
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
    })
}

pub fn process_interchange_import_artifact_operation(
    artifact_id: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(
        TranslationAdminOperation::ProcessInterchangeImportArtifact {
            artifact_id: required_text("interchange_artifact_id", artifact_id)?,
            idempotency_key: required_text("idempotency_key", idempotency_key)?,
        },
    )
}

fn parse_interchange_artifact_expiry(value: &str) -> Result<u32, CommandInputError> {
    let value = value.trim().parse::<u32>().map_err(|_| CommandInputError {
        field: "interchange_artifact_expiry_seconds",
        message: "must be a whole number of seconds".to_string(),
    })?;
    if !(MIN_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS..=MAX_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS)
        .contains(&value)
    {
        return Err(CommandInputError {
            field: "interchange_artifact_expiry_seconds",
            message: format!(
                "must be between {MIN_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS} and {MAX_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS}",
            ),
        });
    }
    Ok(value)
}

fn canonical_interchange_artifact_document(
    document_json: &str,
    job_id: &str,
) -> Result<String, CommandInputError> {
    if document_json.len() > MAX_INTERCHANGE_ARTIFACT_DOCUMENT_BYTES {
        return Err(CommandInputError {
            field: "interchange_artifact_document",
            message: format!("must not exceed {MAX_INTERCHANGE_ARTIFACT_DOCUMENT_BYTES} bytes",),
        });
    }
    let document = serde_json::from_str::<InterchangeDocument>(document_json).map_err(|error| {
        CommandInputError {
            field: "interchange_artifact_document",
            message: format!("must be an interchange document: {error}"),
        }
    })?;
    if document.schema_version != 1 || document.job_id != job_id {
        return Err(CommandInputError {
            field: "interchange_artifact_document",
            message: "must match schema version 1 and the selected job".to_string(),
        });
    }
    if document.items.is_empty()
        || document.items.len() > usize::from(MAX_INTERCHANGE_ARTIFACT_ITEMS)
    {
        return Err(CommandInputError {
            field: "interchange_artifact_document",
            message: format!("must contain between 1 and {MAX_INTERCHANGE_ARTIFACT_ITEMS} items",),
        });
    }
    let canonical = serde_json::to_string(&document).map_err(|error| CommandInputError {
        field: "interchange_artifact_document",
        message: format!("could not serialize the interchange document: {error}"),
    })?;
    if canonical.len() > MAX_INTERCHANGE_ARTIFACT_DOCUMENT_BYTES {
        return Err(CommandInputError {
            field: "interchange_artifact_document",
            message: format!("must not exceed {MAX_INTERCHANGE_ARTIFACT_DOCUMENT_BYTES} bytes",),
        });
    }
    Ok(canonical)
}

pub fn interchange_document_json(
    document: &InterchangeDocument,
) -> Result<String, CommandInputError> {
    serde_json::to_string_pretty(document).map_err(|error| CommandInputError {
        field: "export_document",
        message: format!("could not serialize the interchange document: {error}"),
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportItemDraft {
    schema_version: u16,
    job_id: String,
    item_id: String,
    identity: TranslationResourceIdentity,
    source_digest: String,
    values: Vec<ProposalValueInput>,
}

pub fn import_item_operation(
    draft_json: &str,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let draft =
        serde_json::from_str::<ImportItemDraft>(draft_json).map_err(|error| CommandInputError {
            field: "import_document",
            message: format!("must be an interchange item import object: {error}"),
        })?;
    if draft.schema_version == 0 {
        return Err(CommandInputError {
            field: "schema_version",
            message: "must be greater than zero".to_string(),
        });
    }
    if draft.values.is_empty() {
        return Err(CommandInputError {
            field: "values",
            message: "must contain at least one translated field".to_string(),
        });
    }
    Ok(TranslationAdminOperation::ImportItem {
        schema_version: draft.schema_version,
        job_id: required_text("job_id", &draft.job_id)?,
        item_id: required_text("item_id", &draft.item_id)?,
        identity: draft.identity,
        source_digest: required_text("source_digest", &draft.source_digest)?,
        values: draft.values,
        idempotency_key: required_text("idempotency_key", idempotency_key)?,
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
pub enum MachineProposalCommand {
    Estimate,
    Generate,
}

#[derive(Debug, Clone, Copy)]
pub struct MachineProposalInput<'a> {
    pub item_id: &'a str,
    pub field_keys: &'a str,
    pub minimum_memory_similarity_basis_points: &'a str,
    pub tone: &'a str,
    pub domain: &'a str,
    pub style: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct MachineCancellationInput<'a> {
    pub operation_id: &'a str,
    pub reason: &'a str,
    pub idempotency_key: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct MachineRecoveryInput<'a> {
    pub operation_id: &'a str,
    pub expected_updated_at: &'a str,
    pub proposal: MachineProposalInput<'a>,
    pub reason: &'a str,
    pub idempotency_key: &'a str,
}

struct ParsedMachineProposal {
    item_id: String,
    field_keys: Vec<String>,
    minimum_memory_similarity_basis_points: u16,
    tone: Option<String>,
    domain: Option<String>,
    style: Option<String>,
}

pub fn machine_proposal_operation(
    command: MachineProposalCommand,
    input: MachineProposalInput<'_>,
    idempotency_key: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let ParsedMachineProposal {
        item_id,
        field_keys,
        minimum_memory_similarity_basis_points,
        tone,
        domain,
        style,
    } = parse_machine_proposal(input)?;
    let idempotency_key = required_text("idempotency_key", idempotency_key)?;

    Ok(match command {
        MachineProposalCommand::Estimate => TranslationAdminOperation::EstimateMachineTranslation {
            item_id,
            field_keys,
            minimum_memory_similarity_basis_points,
            tone,
            domain,
            style,
            idempotency_key,
        },
        MachineProposalCommand::Generate => TranslationAdminOperation::GenerateMachineProposal {
            item_id,
            field_keys,
            minimum_memory_similarity_basis_points,
            tone,
            domain,
            style,
            idempotency_key,
        },
    })
}

pub fn read_machine_operation_status_operation(
    operation_id: &str,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::ReadMachineOperationStatus {
        operation_id: required_text("operation_id", operation_id)?,
    })
}

pub fn cancel_machine_operation(
    input: MachineCancellationInput<'_>,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::CancelMachineOperation {
        operation_id: required_text("operation_id", input.operation_id)?,
        reason: required_value("reason", input.reason)?,
        idempotency_key: required_text("idempotency_key", input.idempotency_key)?,
    })
}

pub fn recover_machine_operation(
    input: MachineRecoveryInput<'_>,
) -> Result<TranslationAdminOperation, CommandInputError> {
    let ParsedMachineProposal {
        item_id,
        field_keys,
        minimum_memory_similarity_basis_points,
        tone,
        domain,
        style,
    } = parse_machine_proposal(input.proposal)?;

    Ok(TranslationAdminOperation::RecoverMachineOperation {
        operation_id: required_text("operation_id", input.operation_id)?,
        expected_updated_at: required_text("expected_updated_at", input.expected_updated_at)?,
        item_id,
        field_keys,
        minimum_memory_similarity_basis_points,
        tone,
        domain,
        style,
        reason: required_value("reason", input.reason)?,
        idempotency_key: required_text("idempotency_key", input.idempotency_key)?,
    })
}

fn parse_machine_proposal(
    input: MachineProposalInput<'_>,
) -> Result<ParsedMachineProposal, CommandInputError> {
    let field_keys = parse_ui_csv(input.field_keys);
    if field_keys.is_empty() {
        return Err(CommandInputError {
            field: "field_keys",
            message: "at least one field key is required".to_string(),
        });
    }
    let mut distinct_field_keys = field_keys.clone();
    distinct_field_keys.sort_unstable();
    distinct_field_keys.dedup();
    if distinct_field_keys.len() != field_keys.len() {
        return Err(CommandInputError {
            field: "field_keys",
            message: "must not contain duplicates".to_string(),
        });
    }

    let minimum_memory_similarity_basis_points = parse_nonnegative_u16(
        "minimum_memory_similarity_basis_points",
        input.minimum_memory_similarity_basis_points,
    )?;
    if minimum_memory_similarity_basis_points > 10_000 {
        return Err(CommandInputError {
            field: "minimum_memory_similarity_basis_points",
            message: "must be between 0 and 10000".to_string(),
        });
    }

    Ok(ParsedMachineProposal {
        item_id: required_text("item_id", input.item_id)?,
        field_keys,
        minimum_memory_similarity_basis_points,
        tone: normalize_ui_text(input.tone),
        domain: normalize_ui_text(input.domain),
        style: normalize_ui_text(input.style),
    })
}

#[derive(Debug, Clone, Copy)]
pub struct AssignmentInput<'a> {
    pub item_id: &'a str,
    pub expected_revision: &'a str,
    pub assignee_kind: &'a str,
    pub assignee_id: &'a str,
    pub idempotency_key: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct UnassignmentInput<'a> {
    pub item_id: &'a str,
    pub expected_revision: &'a str,
    pub idempotency_key: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct JobCancellationInput<'a> {
    pub job_id: &'a str,
    pub expected_revision: &'a str,
    pub reason: &'a str,
    pub idempotency_key: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct ItemRetryInput<'a> {
    pub item_id: &'a str,
    pub expected_revision: &'a str,
    pub reason: &'a str,
    pub idempotency_key: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct ApplyRecoveryInput<'a> {
    pub operation_id: &'a str,
    pub expected_attempt_count: &'a str,
    pub reason: &'a str,
    pub idempotency_key: &'a str,
}

pub fn assign_item_operation(
    input: AssignmentInput<'_>,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::AssignItem {
        item_id: required_text("item_id", input.item_id)?,
        expected_revision: parse_positive_i64("expected_revision", input.expected_revision)?,
        assignee: Actor {
            kind: parse_actor_kind(input.assignee_kind)?,
            id: required_text("assignee_id", input.assignee_id)?,
        },
        idempotency_key: required_text("idempotency_key", input.idempotency_key)?,
    })
}

pub fn unassign_item_operation(
    input: UnassignmentInput<'_>,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::UnassignItem {
        item_id: required_text("item_id", input.item_id)?,
        expected_revision: parse_positive_i64("expected_revision", input.expected_revision)?,
        idempotency_key: required_text("idempotency_key", input.idempotency_key)?,
    })
}

pub fn cancel_job_operation(
    input: JobCancellationInput<'_>,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::CancelJob {
        job_id: required_text("job_id", input.job_id)?,
        expected_revision: parse_positive_i64("expected_revision", input.expected_revision)?,
        reason: required_value("reason", input.reason)?,
        idempotency_key: required_text("idempotency_key", input.idempotency_key)?,
    })
}

pub fn retry_item_operation(
    input: ItemRetryInput<'_>,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::RetryItem {
        item_id: required_text("item_id", input.item_id)?,
        expected_revision: parse_positive_i64("expected_revision", input.expected_revision)?,
        reason: required_value("reason", input.reason)?,
        idempotency_key: required_text("idempotency_key", input.idempotency_key)?,
    })
}

pub fn recover_apply_operation(
    input: ApplyRecoveryInput<'_>,
) -> Result<TranslationAdminOperation, CommandInputError> {
    Ok(TranslationAdminOperation::RecoverApply {
        operation_id: required_text("operation_id", input.operation_id)?,
        expected_attempt_count: parse_positive_i64(
            "expected_attempt_count",
            input.expected_attempt_count,
        )?,
        reason: required_value("reason", input.reason)?,
        idempotency_key: required_text("idempotency_key", input.idempotency_key)?,
    })
}

fn parse_actor_kind(value: &str) -> Result<ActorKind, CommandInputError> {
    match normalize_ui_text(value).as_deref() {
        Some("user") => Ok(ActorKind::User),
        Some("service") => Ok(ActorKind::Service),
        _ => Err(CommandInputError {
            field: "assignee_kind",
            message: "must be user or service".to_string(),
        }),
    }
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
            TranslationAdminTab::Overview.previous(),
            TranslationAdminTab::Workflow
        );
        assert_eq!(
            TranslationAdminTab::Workflow.next(),
            TranslationAdminTab::Overview
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
    fn interchange_operations_are_typed_and_bounded_before_transport() {
        assert!(matches!(
            export_job_operation("job-1", "200").unwrap(),
            TranslationAdminOperation::ExportJob { max_items: 200, .. }
        ));
        assert_eq!(
            export_job_operation("job-1", "201").unwrap_err().field,
            "max_items"
        );
        let operation = import_item_operation(
            r#"{
                "schemaVersion": 1,
                "jobId": "job-1",
                "itemId": "item-1",
                "identity": {
                    "ownerSlug": "media",
                    "resourceKind": "asset",
                    "resourceId": "asset-1",
                    "subresourceId": null
                },
                "sourceDigest": "source-digest",
                "values": [{"key": "alt", "value": "Beschreibung"}]
            }"#,
            "import-key",
        )
        .unwrap();
        assert!(matches!(
            operation,
            TranslationAdminOperation::ImportItem {
                schema_version: 1,
                values,
                ..
            } if values.len() == 1
        ));
        assert_eq!(
            import_item_operation("{}", "import-key").unwrap_err().field,
            "import_document"
        );

        assert!(matches!(
            create_interchange_export_artifact_operation("job-1", "200", "86400", "artifact-key")
                .unwrap(),
            TranslationAdminOperation::CreateInterchangeExportArtifact {
                max_items: 200,
                expires_in_seconds: 86400,
                ..
            }
        ));
        assert_eq!(
            create_interchange_export_artifact_operation("job-1", "0", "86400", "artifact-key")
                .unwrap_err()
                .field,
            "interchange_artifact_max_items"
        );
        assert_eq!(
            create_interchange_export_artifact_operation("job-1", "1", "299", "artifact-key")
                .unwrap_err()
                .field,
            "interchange_artifact_expiry_seconds"
        );
        let document = r#"{
            "schemaVersion": 1,
            "jobId": "job-1",
            "sourceLocale": "en",
            "targetLocale": "de",
            "items": [{
                "itemId": "item-1",
                "identity": {
                    "ownerSlug": "media",
                    "resourceKind": "asset",
                    "resourceId": "asset-1",
                    "subresourceId": null
                },
                "sourceDigest": "source-digest",
                "sourceRevision": "resource-1",
                "targetRevision": null,
                "fields": [{
                    "key": "title",
                    "sourceValue": "Hero",
                    "exactTargetValue": null,
                    "proposedValue": "Held",
                    "sourceHash": "source-hash",
                    "required": true,
                    "maxCharacters": 200,
                    "protectedTokens": []
                }]
            }]
        }"#;
        assert!(matches!(
            store_interchange_import_artifact_operation("job-1", document, "86400", "store-key")
                .unwrap(),
            TranslationAdminOperation::StoreInterchangeImportArtifact {
                job_id,
                expires_in_seconds: 86400,
                ..
            } if job_id == "job-1"
        ));
        assert_eq!(
            store_interchange_import_artifact_operation("job-2", document, "86400", "store-key")
                .unwrap_err()
                .field,
            "interchange_artifact_document"
        );
        assert!(matches!(
            list_interchange_artifacts_operation(ListInterchangeArtifactsOperationInput {
                job_id: Some("job-1"),
                include_expired: false,
                limit: "50",
            })
            .unwrap(),
            TranslationAdminOperation::ListInterchangeArtifacts {
                job_id: Some(job_id),
                limit: 50,
                ..
            } if job_id == "job-1"
        ));
        assert!(matches!(
            process_interchange_import_artifact_operation("artifact-1", "process-key").unwrap(),
            TranslationAdminOperation::ProcessInterchangeImportArtifact { .. }
        ));
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
        let input = MemoryLookupInput {
            source_locale: "en",
            target_locale: "de",
            owner_slug: "media",
            resource_kind: "asset",
            resource_id: "asset-1",
            subresource_id: "",
            field_key: "alt",
            source_text: "Source copy",
            minimum_similarity_basis_points: "8500",
            limit: "10",
        };
        let operation = lookup_memory_operation(input).unwrap();
        assert!(matches!(
            operation,
            TranslationAdminOperation::LookupMemory {
                minimum_similarity_basis_points: 8500,
                limit: 10,
                ..
            }
        ));
        assert_eq!(
            lookup_memory_operation(MemoryLookupInput {
                minimum_similarity_basis_points: "10001",
                ..input
            })
            .unwrap_err()
            .field,
            "minimum_similarity_basis_points"
        );
    }

    #[test]
    fn reviewer_queue_and_workload_reads_validate_their_explicit_scope() {
        let operation = read_reviewer_queue_operation(ReviewerQueueOperationInput {
            job_id: "job-1",
            assignee_kind: "user",
            assignee_id: "reviewer-1",
            include_unassigned: true,
            limit: "50",
        })
        .unwrap();
        assert!(matches!(
            operation,
            TranslationAdminOperation::ReadReviewerQueue {
                assignee: Some(Actor {
                    kind: ActorKind::User,
                    id,
                }),
                include_unassigned: true,
                limit: 50,
                ..
            } if id == "reviewer-1"
        ));
        assert!(matches!(
            read_reviewer_workload_operation("job-1").unwrap(),
            TranslationAdminOperation::ReadReviewerWorkload { job_id } if job_id == "job-1"
        ));
        assert_eq!(
            read_reviewer_queue_operation(ReviewerQueueOperationInput {
                job_id: "job-1",
                assignee_kind: "",
                assignee_id: "reviewer-1",
                include_unassigned: false,
                limit: "50",
            })
            .unwrap_err()
            .field,
            "assignee_kind"
        );
        assert_eq!(
            read_reviewer_queue_operation(ReviewerQueueOperationInput {
                job_id: "job-1",
                assignee_kind: "user",
                assignee_id: "",
                include_unassigned: false,
                limit: "50",
            })
            .unwrap_err()
            .field,
            "assignee_id"
        );
        assert_eq!(
            read_reviewer_queue_operation(ReviewerQueueOperationInput {
                job_id: "job-1",
                assignee_kind: "",
                assignee_id: "",
                include_unassigned: false,
                limit: "201",
            })
            .unwrap_err()
            .field,
            "reviewer_queue_limit"
        );
    }

    #[test]
    fn workflow_note_commands_are_typed_bounded_and_private_by_default() {
        let list = list_workflow_notes_operation(WorkflowNotesOperationInput {
            job_id: " job-1 ",
            item_id: " item-1 ",
            include_resolved: false,
            limit: "50",
        })
        .unwrap();
        assert!(matches!(
            list,
            TranslationAdminOperation::ListWorkflowNotes {
                job_id,
                item_id: Some(item_id),
                include_resolved: false,
                limit: 50,
            } if job_id == "job-1" && item_id == "item-1"
        ));

        let create = create_workflow_note_operation(CreateWorkflowNoteOperationInput {
            job_id: "job-1",
            item_id: "",
            body: " private reviewer context ",
            idempotency_key: "create-workflow-note",
        })
        .unwrap();
        assert!(matches!(
            create,
            TranslationAdminOperation::CreateWorkflowNote {
                item_id: None,
                body,
                ..
            } if body == "private reviewer context"
        ));

        assert!(matches!(
            resolve_workflow_note_operation("note-1", "0", "resolve-workflow-note").unwrap(),
            TranslationAdminOperation::ResolveWorkflowNote {
                note_id,
                expected_revision: 0,
                ..
            } if note_id == "note-1"
        ));
        assert_eq!(
            list_workflow_notes_operation(WorkflowNotesOperationInput {
                limit: "201",
                ..WorkflowNotesOperationInput {
                    job_id: "job-1",
                    item_id: "",
                    include_resolved: false,
                    limit: "50",
                }
            })
            .unwrap_err()
            .field,
            "workflow_note_limit"
        );
        assert_eq!(
            create_workflow_note_operation(CreateWorkflowNoteOperationInput {
                job_id: "job-1",
                item_id: "",
                body: &"x".repeat(4_001),
                idempotency_key: "create-workflow-note",
            })
            .unwrap_err()
            .field,
            "workflow_note_body"
        );
        assert_eq!(
            resolve_workflow_note_operation("note-1", "-1", "resolve-workflow-note")
                .unwrap_err()
                .field,
            "expected_revision"
        );
    }

    #[test]
    fn machine_commands_are_typed_bounded_and_share_request_validation() {
        let input = MachineProposalInput {
            item_id: "item-1",
            field_keys: " title, alt_text ",
            minimum_memory_similarity_basis_points: "8500",
            tone: "formal",
            domain: "retail",
            style: "concise",
        };

        let estimate = machine_proposal_operation(
            MachineProposalCommand::Estimate,
            input,
            "estimate-machine-key",
        )
        .unwrap();
        assert!(matches!(
            estimate,
            TranslationAdminOperation::EstimateMachineTranslation {
                field_keys,
                minimum_memory_similarity_basis_points: 8500,
                tone: Some(tone),
                domain: Some(domain),
                style: Some(style),
                ..
            } if field_keys == ["title", "alt_text"]
                && tone == "formal"
                && domain == "retail"
                && style == "concise"
        ));
        assert!(matches!(
            machine_proposal_operation(
                MachineProposalCommand::Generate,
                input,
                "generate-machine-key",
            )
            .unwrap(),
            TranslationAdminOperation::GenerateMachineProposal { .. }
        ));
        assert_eq!(
            machine_proposal_operation(
                MachineProposalCommand::Estimate,
                MachineProposalInput {
                    field_keys: "title, title",
                    ..input
                },
                "estimate-machine-key",
            )
            .unwrap_err()
            .field,
            "field_keys"
        );
        assert_eq!(
            machine_proposal_operation(
                MachineProposalCommand::Estimate,
                MachineProposalInput {
                    minimum_memory_similarity_basis_points: "10001",
                    ..input
                },
                "estimate-machine-key",
            )
            .unwrap_err()
            .field,
            "minimum_memory_similarity_basis_points"
        );
    }

    #[test]
    fn machine_control_commands_preserve_observed_state_and_recovery_request() {
        assert!(matches!(
            read_machine_operation_status_operation("operation-1").unwrap(),
            TranslationAdminOperation::ReadMachineOperationStatus { operation_id }
                if operation_id == "operation-1"
        ));
        assert!(matches!(
            cancel_machine_operation(MachineCancellationInput {
                operation_id: "operation-1",
                reason: "Operator cancelled the request",
                idempotency_key: "cancel-machine-key",
            })
            .unwrap(),
            TranslationAdminOperation::CancelMachineOperation { .. }
        ));
        assert!(matches!(
            recover_machine_operation(MachineRecoveryInput {
                operation_id: "operation-1",
                expected_updated_at: "2026-08-03T10:00:00Z",
                proposal: MachineProposalInput {
                    item_id: "item-1",
                    field_keys: "alt_text",
                    minimum_memory_similarity_basis_points: "7000",
                    tone: "",
                    domain: "",
                    style: "",
                },
                reason: "Recover a stuck proposal save",
                idempotency_key: "recover-machine-key",
            })
            .unwrap(),
            TranslationAdminOperation::RecoverMachineOperation {
                expected_updated_at,
                field_keys,
                tone: None,
                domain: None,
                style: None,
                ..
            } if expected_updated_at == "2026-08-03T10:00:00Z" && field_keys == ["alt_text"]
        ));
    }

    #[test]
    fn workflow_recovery_commands_are_typed_and_revision_guarded() {
        let assignment_input = AssignmentInput {
            item_id: "item-1",
            expected_revision: "4",
            assignee_kind: "user",
            assignee_id: "actor-1",
            idempotency_key: "assign-key",
        };
        assert!(matches!(
            assign_item_operation(assignment_input).unwrap(),
            TranslationAdminOperation::AssignItem {
                expected_revision: 4,
                assignee: Actor {
                    kind: ActorKind::User,
                    id,
                },
                ..
            } if id == "actor-1"
        ));
        assert!(matches!(
            unassign_item_operation(UnassignmentInput {
                item_id: "item-1",
                expected_revision: "5",
                idempotency_key: "unassign-key",
            })
            .unwrap(),
            TranslationAdminOperation::UnassignItem {
                expected_revision: 5,
                ..
            }
        ));
        assert!(matches!(
            cancel_job_operation(JobCancellationInput {
                job_id: "job-1",
                expected_revision: "2",
                reason: "Operator cancelled the job",
                idempotency_key: "cancel-key",
            })
            .unwrap(),
            TranslationAdminOperation::CancelJob {
                expected_revision: 2,
                ..
            }
        ));
        assert!(matches!(
            retry_item_operation(ItemRetryInput {
                item_id: "item-1",
                expected_revision: "6",
                reason: "Retry after owner correction",
                idempotency_key: "retry-key",
            })
            .unwrap(),
            TranslationAdminOperation::RetryItem {
                expected_revision: 6,
                ..
            }
        ));
        assert!(matches!(
            recover_apply_operation(ApplyRecoveryInput {
                operation_id: "operation-1",
                expected_attempt_count: "3",
                reason: "Recover a stuck owner apply",
                idempotency_key: "recover-apply-key",
            })
            .unwrap(),
            TranslationAdminOperation::RecoverApply {
                expected_attempt_count: 3,
                ..
            }
        ));
        assert_eq!(
            assign_item_operation(AssignmentInput {
                assignee_kind: "automation",
                ..assignment_input
            })
            .unwrap_err()
            .field,
            "assignee_kind"
        );
        assert_eq!(
            recover_apply_operation(ApplyRecoveryInput {
                operation_id: "operation-1",
                expected_attempt_count: "0",
                reason: "Recover a stuck owner apply",
                idempotency_key: "recover-apply-key",
            })
            .unwrap_err()
            .field,
            "expected_attempt_count"
        );
        assert_eq!(
            retry_item_operation(ItemRetryInput {
                item_id: "item-1",
                expected_revision: "6",
                reason: "",
                idempotency_key: "retry-key",
            })
            .unwrap_err()
            .field,
            "reason"
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
