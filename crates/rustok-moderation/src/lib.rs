pub mod application;
pub mod application_dispatch;
pub mod application_recovery;
mod application_scheduler;
mod commands;
pub mod domain;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
mod receipts;
pub mod service;

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub use application::{
    DEFAULT_APPLICATION_LEASE_SECONDS, MAX_APPLICATION_LEASE_SECONDS,
    MAX_APPLICATION_RETRY_SECONDS, MAX_DUE_APPLICATION_OPERATIONS,
};
pub use application_dispatch::{
    APPLICATION_ADAPTER_DEADLINE_SECONDS, APPLICATION_RETRY_BASE_SECONDS,
    APPLICATION_RETRY_MAX_SECONDS, application_retry_delay_seconds,
};
pub use application_recovery::MAX_APPLICATION_RECOVERY_REASON_BYTES;
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

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::MODERATION_CASES_READ,
            Permission::MODERATION_CASES_LIST,
            Permission::MODERATION_CASES_OVERRIDE,
            Permission::MODERATION_CASES_MANAGE,
        ]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut rustok_core::ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        extensions
            .get_or_insert_with::<rustok_runtime::ModuleWorkRegistrations, _>(Default::default)
            .register(std::sync::Arc::new(
                application_scheduler::ModerationApplicationWorkRegistration,
            ));
        Ok(())
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
        assert_eq!(module.migrations().len(), 4);
        assert_eq!(module.migration_dependencies().len(), 4);
        assert_eq!(
            module.permissions(),
            vec![
                Permission::MODERATION_CASES_READ,
                Permission::MODERATION_CASES_LIST,
                Permission::MODERATION_CASES_OVERRIDE,
                Permission::MODERATION_CASES_MANAGE,
            ]
        );

        let mut extensions = rustok_core::ModuleRuntimeExtensions::default();
        module.register_runtime_extensions(&mut extensions).unwrap();
        assert!(
            !extensions
                .get::<rustok_runtime::ModuleWorkRegistrations>()
                .unwrap()
                .is_empty()
        );
    }
}
