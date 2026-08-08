pub mod entities;
mod error;
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
mod policy;
mod progress;
mod public_error;
mod qa;
#[cfg(feature = "runtime")]
mod scheduler;
mod workflow;

use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub use error::{TranslationError, TranslationResult};
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
    MachineTranslationBatchRequest, MachineTranslationBatchResult, MachineTranslationDiagnostic,
    MachineTranslationEstimate, MachineTranslationExecutionEvidence,
    MachineTranslationExecutionStatus, MachineTranslationExecutionStatusEvidence,
    MachineTranslationGlossaryTerm, MachineTranslationMemorySuggestion, MachineTranslationPort,
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
    MachineDiagnosticEvidence, MachineOperationStatusRecord, MachineProposalRecord,
    RecoverMachineOperationInput, TranslationMachineControlService, TranslationMachineService,
};
pub use memory::{
    MemoryEntryRecord, MemoryListInput, MemoryLookupInput, MemoryMatchEvidence, MemoryMatchKind,
    MemoryMutationRecord, MemoryRetentionPolicy, MemorySuggestion, PurgeMemoryEntryInput,
    SetMemoryRetentionInput, TombstoneMemoryEntryInput, TranslationMemoryService,
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

pub struct TranslationModule;

#[async_trait]
impl RusToKModule for TranslationModule {
    fn slug(&self) -> &'static str {
        "translation"
    }

    fn name(&self) -> &'static str {
        "Translation"
    }

    fn description(&self) -> &'static str {
        "Owner-safe translation workflow, inventory, memory, glossary, and machine-translation control plane"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        let workflow_actions = [
            Action::Create,
            Action::Read,
            Action::Update,
            Action::List,
            Action::Resolve,
            Action::Publish,
            Action::Import,
            Action::Export,
            Action::Run,
            Action::Manage,
        ];
        let library_actions = [
            Action::Create,
            Action::Read,
            Action::Update,
            Action::Delete,
            Action::List,
            Action::Import,
            Action::Export,
            Action::Manage,
        ];
        workflow_actions
            .into_iter()
            .map(|action| Permission::new(Resource::Translations, action))
            .chain(library_actions.into_iter().flat_map(|action| {
                [
                    Permission::new(Resource::TranslationMemory, action),
                    Permission::new(Resource::TranslationGlossaries, action),
                ]
            }))
            .collect()
    }

    fn register_runtime_extensions(
        &self,
        _extensions: &mut rustok_core::ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        #[cfg(feature = "runtime")]
        _extensions
            .get_or_insert_with::<rustok_runtime::ModuleWorkRegistrations, _>(Default::default)
            .register(std::sync::Arc::new(
                scheduler::TranslationMemoryRetentionWorkRegistration,
            ));
        Ok(())
    }
}

impl MigrationSource for TranslationModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
