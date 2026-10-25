use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::MigrationDependencyDescriptor;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Inventory adjustments, availability checks, and stock-level persistence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InventoryModule;

impl InventoryModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for InventoryModule {
    fn slug(&self) -> &'static str {
        "inventory"
    }

    fn name(&self) -> &'static str {
        "Inventory"
    }

    fn description(&self) -> &'static str {
        "Inventory adjustments, availability checks, and stock-level persistence"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn dependencies(&self) -> &[&'static str] {
        &["product"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::INVENTORY_CREATE,
            Permission::INVENTORY_READ,
            Permission::INVENTORY_UPDATE,
            Permission::INVENTORY_DELETE,
            Permission::INVENTORY_LIST,
            Permission::INVENTORY_MANAGE,
        ]
    }
}

impl MigrationSource for InventoryModule {
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

    #[tokio::test]
    async fn inventory_module_metadata_and_traits() {
        let module = InventoryModule::new();
        assert_eq!(module.slug(), "inventory");
        assert_eq!(module.name(), "Inventory");
        assert_eq!(
            module.description(),
            "Inventory adjustments, availability checks, and stock-level persistence"
        );
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.dependencies(), &["product"]);
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }

    #[test]
    fn module_permissions_cover_inventory_lifecycle() {
        let module = InventoryModule::new();
        let perms = module.permissions();
        assert!(perms.contains(&Permission::INVENTORY_CREATE));
        assert!(perms.contains(&Permission::INVENTORY_READ));
        assert!(perms.contains(&Permission::INVENTORY_UPDATE));
        assert!(perms.contains(&Permission::INVENTORY_DELETE));
        assert!(perms.contains(&Permission::INVENTORY_LIST));
        assert!(perms.contains(&Permission::INVENTORY_MANAGE));
    }
}
