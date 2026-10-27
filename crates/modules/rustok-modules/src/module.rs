use async_trait::async_trait;
use rustok_core::{
    MigrationDependencyDescriptor, MigrationSource, ModuleKind, ModuleRuntimeExtensions,
    Result as CoreResult, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

use crate::{
    ArtifactEventDeliveryWorkRegistration, ArtifactScheduleDeliveryWorkRegistration, migrations,
};

/// Mandatory Core entry point for module and marketplace control-plane ownership.
pub struct ModulesModule;

impl MigrationSource for ModulesModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        vec![
            MigrationDependencyDescriptor::new(
                "m20260717_000017_create_registry_publish_build_staging",
                vec!["m20260403_000061_create_registry_publish_tables"],
            ),
            MigrationDependencyDescriptor::new(
                "m20260722_000034_static_promotions",
                vec!["m20260403_000061_create_registry_publish_tables"],
            ),
            MigrationDependencyDescriptor::new(
                "m20260727_000040_registry_platform_admission_contracts",
                vec!["m20260403_000061_create_registry_publish_tables"],
            ),
            MigrationDependencyDescriptor::new(
                "m20260727_000041_registry_release_artifact_contracts",
                vec!["m20260717_000017_create_registry_publish_build_staging"],
            ),
        ]
    }
}

#[async_trait]
impl RusToKModule for ModulesModule {
    fn slug(&self) -> &'static str {
        "modules"
    }

    fn name(&self) -> &'static str {
        "Module Platform"
    }

    fn description(&self) -> &'static str {
        "Mandatory module artifact, marketplace, and lifecycle control plane"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> CoreResult<()> {
        let registrations = extensions
            .get_or_insert_with::<rustok_runtime::ModuleWorkRegistrations, _>(Default::default);
        registrations.register(std::sync::Arc::new(
            ArtifactEventDeliveryWorkRegistration::default(),
        ));
        registrations.register(std::sync::Arc::new(
            ArtifactScheduleDeliveryWorkRegistration::default(),
        ));
        Ok(())
    }
}
