/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Module metadata and migration source registration for `rustok-product-relations`.

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{
    HealthStatus, MigrationDependencyDescriptor, MigrationSource, ModuleKind, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Platform module descriptor for Product Relations.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProductRelationsModule;

impl ProductRelationsModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for ProductRelationsModule {
    fn slug(&self) -> &'static str {
        "product_relations"
    }

    fn name(&self) -> &'static str {
        "Product Relations"
    }

    fn description(&self) -> &'static str {
        "Product relations, merchandising associations (cross-sells, up-sells, related, accessories, alternatives) and reordering"
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

impl MigrationSource for ProductRelationsModule {
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
    fn test_product_relations_module_metadata() {
        let module = ProductRelationsModule::new();
        assert_eq!(module.slug(), "product_relations");
        assert_eq!(module.name(), "Product Relations");
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
    async fn test_product_relations_module_health() {
        let module = ProductRelationsModule;
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }
}
