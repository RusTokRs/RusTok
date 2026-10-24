use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{
    MigrationDependencyDescriptor, MigrationSource, ModuleRuntimeExtensions,
    module::{HealthStatus, ModuleKind, RusToKModule},
};
use sea_orm_migration::MigrationTrait;

use crate::{application_scheduler, migrations};

/// Cross-domain moderation module for reports, cases, decisions, and auditable enforcement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModerationModule;

impl ModerationModule {
    pub fn new() -> Self {
        Self
    }
}

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

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
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
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        extensions
            .get_or_insert_with::<rustok_runtime::ModuleWorkRegistrations, _>(Default::default)
            .register(Arc::new(
                application_scheduler::ModerationApplicationWorkRegistration,
            ));
        Ok(())
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
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
    use super::ModerationModule;
    use rustok_api::Permission;
    use rustok_core::{
        MigrationSource,
        module::{HealthStatus, ModuleKind, RusToKModule},
    };

    #[tokio::test]
    async fn moderation_module_metadata_and_health() {
        let module = ModerationModule;
        assert_eq!(module.slug(), "moderation");
        assert_eq!(module.name(), "Moderation");
        assert_eq!(module.kind(), ModuleKind::Core);
        assert!(module.dependencies().is_empty());
        assert_eq!(module.health().await, HealthStatus::Healthy);
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
