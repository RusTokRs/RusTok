use async_trait::async_trait;
use rustok_core::permissions::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod migrations;
pub mod services;

pub use services::InventoryService;

pub struct InventoryModule;

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
}
