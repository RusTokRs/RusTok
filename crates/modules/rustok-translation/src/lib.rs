//! # rustok-translation
//!
//! Owner-safe translation workflow and inventory control plane for RusToK.
//!
//! Provides translation memory, glossaries, reviewer queues, interchange artifacts,
//! and provider-safe translation application.

#![doc = include_str!("../CRATE_API.md")]
#![deny(clippy::unwrap_used, clippy::expect_used)]
mod idempotency;
mod collaboration;
pub mod entities;
mod error;
mod exchange;
mod glossary;
#[cfg(feature = "graphql")]
pub mod graphql;
#[cfg(feature = "graphql")]
pub mod graphql_runtime;
mod interchange;
mod inventory;
mod machine;
mod machine_service;
mod memory;
pub mod migrations;
mod observability;
mod policy;
mod progress;
mod public_error;
mod qa;
#[cfg(feature = "runtime")]
mod scheduler;
mod workflow;


pub use collaboration::{
    CreateWorkflowNoteInput, ListWorkflowNotesInput, MAX_WORKFLOW_NOTE_BODY_CHARACTERS,
    MAX_WORKFLOW_NOTE_LIST_LIMIT, ResolveWorkflowNoteInput, TranslationCollaborationService,
    WorkflowNoteRecord,
};
pub use error::{TranslationError, TranslationResult};
pub use exchange::{
    CreateInterchangeExportArtifactInput, DEFAULT_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS,
    ListInterchangeArtifactsInput, MAX_INTERCHANGE_ARTIFACT_BYTES,
    MAX_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS, MAX_INTERCHANGE_ARTIFACT_LIST_LIMIT,
    MIN_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS, ProcessInterchangeImportArtifactInput,
    ReadInterchangeArtifactInput, StoreInterchangeImportArtifactInput, TranslationExchangeService,
    TranslationInterchangeArtifactContent, TranslationInterchangeArtifactRecord,
    TranslationInterchangeArtifactStatus, TranslationInterchangeConflictReport,
    TranslationInterchangeDirection, TranslationInterchangeItemOutcome, parse_artifact_document,
};
pub use glossary::{
    CreateGlossaryInput, GlossaryBinding, GlossaryConcept, GlossaryMatchKind, GlossaryRecord,
    GlossaryScope, GlossarySummaryRecord, GlossaryTermPolicy, GlossaryVariant,
    ReplaceGlossaryTermsInput, SetGlossaryActiveInput, TranslationGlossaryService,
    UpdateGlossaryInput,
};
pub use interchange::{
    ExportTranslationJobInput, ImportTranslationItemInput, TranslationInterchangeDocument,
    TranslationInterchangeField, TranslationInterchangeItem, TranslationInterchangeService,
};
pub use inventory::{
    TranslationInventoryRebuildResult, TranslationInventoryService, TranslationInventorySyncResult,
};
pub use machine::{
    MAX_MACHINE_TRANSLATION_BATCH_CHARACTERS, MAX_MACHINE_TRANSLATION_BATCH_UNITS,
    MAX_MACHINE_TRANSLATION_GLOSSARY_TERMS, MAX_MACHINE_TRANSLATION_MEMORY_SUGGESTIONS_PER_UNIT,
    MAX_MACHINE_TRANSLATION_PROTECTED_TOKENS_PER_UNIT, MachineTranslationAttemptEvidence,
    MachineTranslationBatchExecution, MachineTranslationBatchRequest,
    MachineTranslationBatchResult, MachineTranslationDiagnostic, MachineTranslationEstimate,
    MachineTranslationExecutionEvidence, MachineTranslationExecutionStatus,
    MachineTranslationExecutionStatusEvidence, MachineTranslationGlossaryTerm,
    MachineTranslationMemorySuggestion, MachineTranslationPort,
    MachineTranslationProviderDescriptor, MachineTranslationProviderHealth,
    MachineTranslationProviderState, MachineTranslationResourceContext, MachineTranslationUnit,
    MachineTranslationUnitResult, MachineTranslationUsage,
};
#[cfg(feature = "runtime")]
pub use machine::{
    MachineTranslationPortFactory, SharedMachineTranslationPortFactory,
    machine_translation_port_from_context,
};
pub use machine_service::{
    CancelMachineOperationInput, GenerateMachineProposalInput, MachineCancellationRecord,
    MachineDiagnosticEvidence, MachineOperationStatusRecord, MachineProposalOutcome,
    MachineProposalRecord, RecoverMachineOperationInput, TranslationMachineControlService,
    TranslationMachineService,
};
pub use memory::{
    MemoryEntryRecord, MemoryListInput, MemoryLookupInput, MemoryMatchEvidence, MemoryMatchKind,
    MemoryMutationRecord, MemorySuggestion, PurgeMemoryEntryInput, SetMemoryRetentionInput,
    TombstoneMemoryEntryInput, TranslationMemoryService,
};
pub use policy::{
    ReplaceRequiredTargetLocalesInput, TranslationPolicyFreshness, TranslationPolicyRecord,
    TranslationPolicyService,
};
pub use progress::{
    JobProgressRecord, ProviderProgressRecord, ProviderProjectionFreshness,
    RequiredProviderProgressRecord, ReviewerQueueInput, ReviewerQueueRecord, ReviewerWorkloadInput,
    ReviewerWorkloadRecord, TranslationProgressService,
};
pub use public_error::{
    TranslationPublicError, TranslationPublicErrorKind, map_translation_public_error,
};
pub use qa::evaluate_patch_qa;
pub use workflow::{
    AddItemInput, ApplyProposalInput, ApplyRecord, ApproveProposalInput, AssignItemInput,
    AssignmentRecord, CancelJobInput, CancellationRecord, CreateJobInput, JobItemRecord, JobRecord,
    ProposalOrigin, ProposalRecord, ProposalValue, RecoverApplyInput, RetryItemInput, RetryRecord,
    SaveProposalInput, SubmitProposalInput, TranslationWorkflowService, UnassignItemInput,
};

pub mod module;
pub use module::TranslationModule;

#[cfg(test)]
mod contract_tests;

