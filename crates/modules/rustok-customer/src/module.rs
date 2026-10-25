use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Default storefront customer submodule in the ecommerce family.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CustomerModule;

impl CustomerModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for CustomerModule {
    fn slug(&self) -> &'static str {
        "customer"
    }

    fn name(&self) -> &'static str {
        "Customer"
    }

    fn description(&self) -> &'static str {
        "Default storefront customer submodule in the ecommerce family"
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
            Permission::CUSTOMERS_CREATE,
            Permission::CUSTOMERS_READ,
            Permission::CUSTOMERS_UPDATE,
            Permission::CUSTOMERS_DELETE,
            Permission::CUSTOMERS_LIST,
            Permission::CUSTOMERS_MANAGE,
        ]
    }
}

impl MigrationSource for CustomerModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn customer_module_metadata_and_traits() {
        let module = CustomerModule::new();
        assert_eq!(module.slug(), "customer");
        assert_eq!(module.name(), "Customer");
        assert_eq!(
            module.description(),
            "Default storefront customer submodule in the ecommerce family"
        );
        assert!(!module.version().is_empty());
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        assert!(module.dependencies().is_empty());
        assert_eq!(module.permissions().len(), 6);
        assert!(!module.migrations().is_empty());
    }

    #[test]
    fn module_permissions_cover_customer_lifecycle() {
        let module = CustomerModule::new();
        let permissions = module.permissions();

        assert!(permissions.contains(&Permission::CUSTOMERS_CREATE));
        assert!(permissions.contains(&Permission::CUSTOMERS_READ));
        assert!(permissions.contains(&Permission::CUSTOMERS_UPDATE));
        assert!(permissions.contains(&Permission::CUSTOMERS_DELETE));
        assert!(permissions.contains(&Permission::CUSTOMERS_LIST));
        assert!(permissions.contains(&Permission::CUSTOMERS_MANAGE));
    }
}
