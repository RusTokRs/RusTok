use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{
    MigrationDependencyDescriptor, MigrationPhaseConstraint, MigrationSafetyClass,
    MigrationSafetyMetadata, MigrationSource, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlexModule;

impl FlexModule {
    pub fn new() -> Self {
        Self
    }
}

impl MigrationSource for FlexModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        crate::migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        vec![
            MigrationDependencyDescriptor::new(
                "m20260909_000003_add_attached_translation_change_journal",
                vec![
                    "m20260405_000004_create_flex_attached_localized_values",
                    "m20260822_000001_create_generic_attached_donor_storage",
                ],
            ),
            MigrationDependencyDescriptor::new(
                "m20260910_000005_add_standalone_translation_change_journal",
                vec![
                    "m20260317_000001_create_flex_standalone_tables",
                    "m20260407_000001_split_flex_entry_localized_values",
                ],
            ),
            MigrationDependencyDescriptor::new(
                "m20260911_000006_add_standalone_field_policies",
                vec!["m20260317_000001_create_flex_standalone_tables"],
            ),
        ]
    }

    fn migration_safety_metadata(&self) -> Vec<MigrationSafetyMetadata> {
        vec![
            MigrationSafetyMetadata::new(
                "m20260909_000003_add_attached_translation_change_journal",
                MigrationSafetyClass::ExpandContract,
                MigrationPhaseConstraint::PreActivation,
            ),
            MigrationSafetyMetadata::new(
                "m20260910_000004_add_attached_field_policies",
                MigrationSafetyClass::ExpandContract,
                MigrationPhaseConstraint::PreActivation,
            ),
            MigrationSafetyMetadata::new(
                "m20260910_000005_add_standalone_translation_change_journal",
                MigrationSafetyClass::ExpandContract,
                MigrationPhaseConstraint::PreActivation,
            ),
            MigrationSafetyMetadata::new(
                "m20260911_000006_add_standalone_field_policies",
                MigrationSafetyClass::ExpandContract,
                MigrationPhaseConstraint::PreActivation,
            ),
        ]
    }
}

#[async_trait]
impl RusToKModule for FlexModule {
    fn slug(&self) -> &'static str {
        "flex"
    }

    fn name(&self) -> &'static str {
        "Flex"
    }

    fn description(&self) -> &'static str {
        "Capability-only custom fields runtime for attached and standalone extension flows"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::FLEX_SCHEMAS_CREATE,
            Permission::FLEX_SCHEMAS_READ,
            Permission::FLEX_SCHEMAS_UPDATE,
            Permission::FLEX_SCHEMAS_DELETE,
            Permission::FLEX_SCHEMAS_LIST,
            Permission::FLEX_SCHEMAS_MANAGE,
            Permission::FLEX_ENTRIES_CREATE,
            Permission::FLEX_ENTRIES_READ,
            Permission::FLEX_ENTRIES_UPDATE,
            Permission::FLEX_ENTRIES_DELETE,
            Permission::FLEX_ENTRIES_LIST,
            Permission::FLEX_ENTRIES_MANAGE,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_metadata() {
        let module = FlexModule::new();
        assert_eq!(module.slug(), "flex");
        assert_eq!(module.name(), "Flex");
        assert_eq!(
            module.description(),
            "Capability-only custom fields runtime for attached and standalone extension flows"
        );
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert!(
            module
                .permissions()
                .contains(&Permission::FLEX_SCHEMAS_MANAGE)
        );
    }
}
