use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::MigrationDependencyDescriptor;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Default fulfillment submodule in the ecommerce family.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FulfillmentModule;

impl FulfillmentModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for FulfillmentModule {
    fn slug(&self) -> &'static str {
        "fulfillment"
    }

    fn name(&self) -> &'static str {
        "Fulfillment"
    }

    fn description(&self) -> &'static str {
        "Default fulfillment submodule in the ecommerce family"
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
            Permission::FULFILLMENTS_CREATE,
            Permission::FULFILLMENTS_READ,
            Permission::FULFILLMENTS_UPDATE,
            Permission::FULFILLMENTS_DELETE,
            Permission::FULFILLMENTS_LIST,
            Permission::FULFILLMENTS_MANAGE,
        ]
    }
}

impl MigrationSource for FulfillmentModule {
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
    async fn fulfillment_module_metadata_and_traits() {
        let module = FulfillmentModule::new();
        assert_eq!(module.slug(), "fulfillment");
        assert_eq!(module.name(), "Fulfillment");
        assert_eq!(
            module.description(),
            "Default fulfillment submodule in the ecommerce family"
        );
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }

    #[test]
    fn module_permissions_cover_fulfillment_lifecycle() {
        let module = FulfillmentModule::new();
        let perms = module.permissions();
        assert!(perms.contains(&Permission::FULFILLMENTS_CREATE));
        assert!(perms.contains(&Permission::FULFILLMENTS_READ));
        assert!(perms.contains(&Permission::FULFILLMENTS_UPDATE));
        assert!(perms.contains(&Permission::FULFILLMENTS_DELETE));
        assert!(perms.contains(&Permission::FULFILLMENTS_LIST));
        assert!(perms.contains(&Permission::FULFILLMENTS_MANAGE));
    }
}
