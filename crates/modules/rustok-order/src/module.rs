use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Default order submodule in the ecommerce family.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OrderModule;

impl OrderModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for OrderModule {
    fn slug(&self) -> &'static str {
        "order"
    }

    fn name(&self) -> &'static str {
        "Order"
    }

    fn description(&self) -> &'static str {
        "Default order submodule in the ecommerce family"
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

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::ORDERS_CREATE,
            Permission::ORDERS_READ,
            Permission::ORDERS_UPDATE,
            Permission::ORDERS_DELETE,
            Permission::ORDERS_LIST,
            Permission::ORDERS_MANAGE,
        ]
    }
}

impl MigrationSource for OrderModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn order_module_metadata_and_traits() {
        let module = OrderModule::new();
        assert_eq!(module.slug(), "order");
        assert_eq!(module.name(), "Order");
        assert_eq!(
            module.description(),
            "Default order submodule in the ecommerce family"
        );
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }

    #[test]
    fn module_permissions_cover_order_lifecycle() {
        let module = OrderModule::new();
        let perms = module.permissions();
        assert!(perms.contains(&Permission::ORDERS_CREATE));
        assert!(perms.contains(&Permission::ORDERS_READ));
        assert!(perms.contains(&Permission::ORDERS_UPDATE));
        assert!(perms.contains(&Permission::ORDERS_DELETE));
        assert!(perms.contains(&Permission::ORDERS_LIST));
        assert!(perms.contains(&Permission::ORDERS_MANAGE));
    }
}
