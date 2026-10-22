pub mod model;
pub mod provider;

pub use model::*;
pub use provider::*;

pub mod api {
    pub use crate::model::*;
    pub use crate::provider::*;
}

#[cfg(feature = "server")]
pub mod entities;
#[cfg(all(feature = "server", feature = "graphql"))]
pub mod graphql;
#[cfg(feature = "server")]
pub mod migrations;
#[cfg(feature = "server")]
mod reconciliation;
#[cfg(feature = "server")]
mod service;
#[cfg(feature = "server")]
mod target_lifecycle;

#[cfg(feature = "server")]
use async_trait::async_trait;
#[cfg(feature = "server")]
use rustok_core::{
    MigrationDependencyDescriptor, MigrationSource, ModuleEventListenerContext,
    ModuleEventListenerRegistry, ModuleRuntimeExtensions, RusToKModule,
};
#[cfg(feature = "server")]
use sea_orm_migration::MigrationTrait;
#[cfg(feature = "server")]
use std::sync::Arc;

#[cfg(feature = "server")]
pub use reconciliation::{
    MAX_REACTION_RECONCILIATION_ACTOR_STATES, MAX_REACTION_RECONCILIATION_ISSUES,
    ReactionAggregateComparison, ReactionReconciliationIssue, ReactionReconciliationReceipt,
    ReactionReconciliationReport, ReactionReconciliationRequest, ReactionReconciliationStatus,
    RepairReactionSubjectCommand,
};
#[cfg(feature = "server")]
pub use service::ReactionsService;

#[cfg(feature = "server")]
pub struct ReactionsModule;

#[cfg(feature = "server")]
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

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        ctx: &ModuleEventListenerContext<'_>,
    ) {
        let subject_registry = reaction_subject_registry_from_extensions(ctx.extensions)
            .unwrap_or_else(|| Arc::new(ReactionSubjectRegistry::default()));
        registry.register(target_lifecycle::ReactionTargetDeletionHandler::new(
            ctx.db.clone(),
            subject_registry,
        ));
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

#[cfg(feature = "server")]
impl MigrationSource for ReactionsModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};

    use super::*;

    #[test]
    fn module_registers_target_deletion_listener() {
        let module = ReactionsModule;
        let _ = module;
    }

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
