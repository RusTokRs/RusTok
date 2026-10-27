use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Default cart submodule in the ecommerce family.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CartModule;

impl CartModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for CartModule {
    fn slug(&self) -> &'static str {
        "cart"
    }

    fn name(&self) -> &'static str {
        "Cart"
    }

    fn description(&self) -> &'static str {
        "Default cart submodule in the ecommerce family"
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
}

impl MigrationSource for CartModule {
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
    async fn cart_module_metadata() {
        let module = CartModule::new();
        assert_eq!(module.slug(), "cart");
        assert_eq!(module.name(), "Cart");
        assert_eq!(
            module.description(),
            "Default cart submodule in the ecommerce family"
        );
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        assert!(!module.migrations().is_empty());
        assert!(!module.migration_dependencies().is_empty());
    }
}
