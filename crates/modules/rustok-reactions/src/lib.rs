pub mod entities;
#[cfg(feature = "graphql")]
pub mod graphql;
pub mod migrations;
mod reconciliation;
mod service;

use async_trait::async_trait;
use rustok_core::{
    MigrationDependencyDescriptor, MigrationSource, ModuleRuntimeExtensions, RusToKModule,
};
use rustok_reactions_api::{
    ensure_reaction_subject_factory_registry, ensure_reaction_subject_registry,
};
use sea_orm_migration::MigrationTrait;

pub use reconciliation::{
    MAX_REACTION_RECONCILIATION_ACTOR_STATES, MAX_REACTION_RECONCILIATION_ISSUES,
    ReactionAggregateComparison, ReactionReconciliationIssue, ReactionReconciliationReceipt,
    ReactionReconciliationReport, ReactionReconciliationRequest, ReactionReconciliationStatus,
    RepairReactionSubjectCommand,
};
pub use rustok_reactions_api as api;
pub use service::ReactionsService;

pub struct ReactionsModule;

#[async_trait]
impl RusToKModule for ReactionsModule {
    fn slug(&self) -> &'static str {
        "reactions"
    }

    fn name(&self) -> &'static str {
        "Reactions"
    }

    fn description(&self) -> &'static str {
        "Shared bounded reactions with source-owned subject authorization"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["outbox"]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        let _ = ensure_reaction_subject_registry(extensions);
        let _ = ensure_reaction_subject_factory_registry(extensions);
        Ok(())
    }
}

impl MigrationSource for ReactionsModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
    use rustok_reactions_api::{
        reaction_subject_factory_registry_from_extensions,
        reaction_subject_registry_from_extensions,
    };

    use super::ReactionsModule;

    #[test]
    fn module_initializes_registries_and_declares_owner_schema() {
        let module = ReactionsModule;
        assert_eq!(module.slug(), "reactions");
        assert_eq!(module.dependencies(), &["outbox"]);
        assert_eq!(module.migrations().len(), 1);
        assert_eq!(module.migration_dependencies().len(), 1);

        let mut extensions = ModuleRuntimeExtensions::default();
        module
            .register_runtime_extensions(&mut extensions)
            .expect("reaction runtime extensions should initialize");

        assert!(reaction_subject_registry_from_extensions(&extensions).is_some());
        assert!(reaction_subject_factory_registry_from_extensions(&extensions).is_some());
    }
}
