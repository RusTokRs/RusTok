/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{
    HealthStatus, MigrationDependencyDescriptor, MigrationSource, ModuleKind, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProductBundlesModule;

impl ProductBundlesModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for ProductBundlesModule {
    fn slug(&self) -> &'static str {
        "product_bundles"
    }

    fn name(&self) -> &'static str {
        "Product Bundles"
    }

    fn description(&self) -> &'static str {
        "Product bundles, kits, configurable sets, and package discounts"
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

impl MigrationSource for ProductBundlesModule {
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
    fn test_product_bundles_module_metadata() {
        let module = ProductBundlesModule::new();
        assert_eq!(module.slug(), "product_bundles");
        assert_eq!(module.name(), "Product Bundles");
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.dependencies(), &["product"]);
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(
            module.permissions(),
            vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE]
        );
        assert!(!module.migrations().is_empty());
    }

    #[tokio::test]
    async fn test_product_bundles_module_health() {
        let module = ProductBundlesModule;
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }
}
