pub mod entities;
mod error;
mod inventory;
pub mod migrations;
mod progress;
mod workflow;

use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub use error::{TranslationError, TranslationResult};
pub use inventory::{
    TranslationInventoryRebuildResult, TranslationInventoryService, TranslationInventorySyncResult,
};
pub use progress::{JobProgressRecord, TranslationProgressService};
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
}

impl MigrationSource for TranslationModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
