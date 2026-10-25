use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{
    HealthStatus, MigrationDependencyDescriptor, MigrationSource, ModuleKind, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BrandModule;

impl BrandModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for BrandModule {
    fn slug(&self) -> &'static str {
        "brand"
    }

    fn name(&self) -> &'static str {
        "Brand"
    }

    fn description(&self) -> &'static str {
        "Brand catalog, manufacturers, media presentation, and product associations"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    fn dependencies(&self) -> &[&'static str] {
        &["product"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE]
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

impl MigrationSource for BrandModule {
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

    #[test]
    fn brand_module_metadata_matches_contract() {
        let module = BrandModule::new();
        assert_eq!(module.slug(), "brand");
        assert_eq!(module.name(), "Brand");
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.dependencies(), &["product"]);
        assert_eq!(
            module.permissions(),
            vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE]
        );
        assert!(!module.description().is_empty());
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn brand_module_health_reports_healthy() {
        let module = BrandModule::default();
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }

    #[test]
    fn brand_module_migrations_configured() {
        let module = BrandModule;
        assert_eq!(module.migrations().len(), 1);
        let deps = module.migration_dependencies();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].migration, "m20260915_000001_create_brands");
        assert_eq!(
            deps[0].after,
            vec!["m20250130_000012_create_commerce_products"]
        );
    }
}
