mod commands;
pub mod domain;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
mod receipts;
pub mod service;

use async_trait::async_trait;
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub use domain::*;
pub use error::{ModerationError, ModerationResult};
pub use ports::*;
pub use rustok_moderation_api::{
    MAX_MODERATION_CAPABILITY_KEY_BYTES, MAX_MODERATION_EFFECT_CAPABILITIES,
    MODERATION_DECISION_EFFECT_SCHEMA_V1, ModerationCapabilityKey,
    ModerationSubjectAdapterBuildError, ModerationSubjectAdapterFactory,
    ModerationSubjectAdapterFactoryRegistry, ModerationSubjectAdapterKey,
    ModerationSubjectAdapterRegistry, ModerationSubjectAdapterRegistryError,
    materialize_moderation_subject_adapter_registry,
    moderation_subject_adapter_registry_from_extensions, register_moderation_subject_adapter,
    register_moderation_subject_adapter_factory,
};
pub use service::ModerationService;

pub struct ModerationModule;

#[async_trait]
impl RusToKModule for ModerationModule {
    fn slug(&self) -> &'static str {
        "moderation"
    }

    fn name(&self) -> &'static str {
        "Moderation"
    }

    fn description(&self) -> &'static str {
        "Cross-domain moderation owner for reports, cases, decisions, and auditable enforcement"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}

impl MigrationSource for ModerationModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_boundary_is_owner_neutral() {
        let module = ModerationModule;
        assert_eq!(module.slug(), "moderation");
        assert!(module.dependencies().is_empty());
        assert_eq!(module.migrations().len(), 3);
        assert_eq!(module.migration_dependencies().len(), 3);
        assert!(module.permissions().is_empty());
    }
}
