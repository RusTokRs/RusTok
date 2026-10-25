use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::MigrationDependencyDescriptor;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Variant pricing, price lists, regions, and discount calculations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PricingModule;

impl PricingModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for PricingModule {
    fn slug(&self) -> &'static str {
        "pricing"
    }

    fn name(&self) -> &'static str {
        "Pricing"
    }

    fn description(&self) -> &'static str {
        "Variant pricing, price lists, regions, and discount calculations"
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
            Permission::PRODUCTS_READ,
            Permission::PRODUCTS_UPDATE,
            Permission::new(Resource::Discounts, Action::Create),
            Permission::new(Resource::Discounts, Action::Read),
            Permission::new(Resource::Discounts, Action::Update),
            Permission::new(Resource::Discounts, Action::Delete),
            Permission::new(Resource::Discounts, Action::List),
            Permission::new(Resource::Discounts, Action::Manage),
        ]
    }
}

impl MigrationSource for PricingModule {
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
    async fn pricing_module_metadata_and_traits() {
        let module = PricingModule::new();
        assert_eq!(module.slug(), "pricing");
        assert_eq!(module.name(), "Pricing");
        assert_eq!(
            module.description(),
            "Variant pricing, price lists, regions, and discount calculations"
        );
        assert!(!module.version().is_empty());
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        assert_eq!(module.dependencies(), &["product"]);
        assert_eq!(module.permissions().len(), 8);
        assert!(!module.migrations().is_empty());
    }

    #[test]
    fn pricing_module_permissions_cover_lifecycle() {
        let module = PricingModule::new();
        let permissions = module.permissions();

        assert!(permissions.contains(&Permission::PRODUCTS_READ));
        assert!(permissions.contains(&Permission::PRODUCTS_UPDATE));
        assert!(permissions.contains(&Permission::new(Resource::Discounts, Action::Create)));
        assert!(permissions.contains(&Permission::new(Resource::Discounts, Action::Read)));
        assert!(permissions.contains(&Permission::new(Resource::Discounts, Action::Update)));
        assert!(permissions.contains(&Permission::new(Resource::Discounts, Action::Delete)));
        assert!(permissions.contains(&Permission::new(Resource::Discounts, Action::List)));
        assert!(permissions.contains(&Permission::new(Resource::Discounts, Action::Manage)));
    }
}
