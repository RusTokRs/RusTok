pub mod model;
pub mod provider;

pub use model::*;
pub use provider::*;

pub mod api {
    pub use super::model::*;
    pub use super::provider::*;
}

#[cfg(feature = "server")]
pub mod application;
#[cfg(feature = "server")]
pub mod application_dispatch;
#[cfg(feature = "server")]
pub mod application_recovery;
#[cfg(feature = "server")]
mod application_scheduler;
#[cfg(feature = "server")]
mod commands;
#[cfg(feature = "server")]
pub mod domain;
#[cfg(feature = "server")]
pub mod entities;
#[cfg(feature = "server")]
pub mod error;
#[cfg(feature = "server")]
pub mod migrations;
#[cfg(feature = "server")]
pub mod ports;
#[cfg(feature = "server")]
mod receipts;
#[cfg(feature = "server")]
pub mod service;

#[cfg(feature = "server")]
pub use application::{
    DEFAULT_APPLICATION_LEASE_SECONDS, MAX_APPLICATION_LEASE_SECONDS,
    MAX_APPLICATION_RETRY_SECONDS, MAX_DUE_APPLICATION_OPERATIONS,
};
#[cfg(feature = "server")]
pub use application_dispatch::{
    APPLICATION_ADAPTER_DEADLINE_SECONDS, APPLICATION_RETRY_BASE_SECONDS,
    APPLICATION_RETRY_MAX_SECONDS, application_retry_delay_seconds,
};
#[cfg(feature = "server")]
pub use application_recovery::MAX_APPLICATION_RECOVERY_REASON_BYTES;
#[cfg(feature = "server")]
pub use domain::*;
#[cfg(feature = "server")]
pub use error::{ModerationError, ModerationResult};
#[cfg(feature = "server")]
pub use ports::*;
#[cfg(feature = "server")]
pub use service::ModerationService;

#[cfg(feature = "server")]
pub struct ModerationModule;

#[cfg(feature = "server")]
#[async_trait::async_trait]
impl rustok_core::RusToKModule for ModerationModule {
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

    fn permissions(&self) -> Vec<rustok_api::Permission> {
        vec![
            rustok_api::Permission::MODERATION_CASES_READ,
            rustok_api::Permission::MODERATION_CASES_LIST,
            rustok_api::Permission::MODERATION_CASES_OVERRIDE,
            rustok_api::Permission::MODERATION_CASES_MANAGE,
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

#[cfg(feature = "server")]
impl rustok_core::MigrationSource for ModerationModule {
    fn migrations(&self) -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use rustok_api::Permission;
    use rustok_core::RusToKModule;

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
