/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

// Existing absolute owner paths inside this crate now resolve to Product itself.
// This alias is crate-local; no foundation compatibility surface is exported.
extern crate self as rustok_commerce_foundation;

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use rustok_seo_targets::register_seo_target_provider;
use sea_orm_migration::MigrationTrait;

pub mod dto;
pub mod entities;
pub mod error {
    // Error ownership still crosses the legacy commerce boundary. Keep the exact
    // type identity until Inventory/Pricing return owner-specific errors.
    pub use commerce_foundation::error::*;
}
pub mod migrations;
pub mod ports;
mod seo_targets;
pub mod services;

pub use error::{CommerceError, CommerceResult};
pub use ports::*;
pub use services::{
    CatalogService, ProductCatalogSchemaService, StorefrontProductList, StorefrontProductListItem,
};

pub struct ProductModule;

#[async_trait]
impl RusToKModule for ProductModule {
    fn slug(&self) -> &'static str {
        "product"
    }

    fn name(&self) -> &'static str {
        "Product"
    }

    fn description(&self) -> &'static str {
        "Product catalog, variants, translations, options, and publication lifecycle"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["taxonomy"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::PRODUCTS_CREATE,
            Permission::PRODUCTS_READ,
            Permission::PRODUCTS_UPDATE,
            Permission::PRODUCTS_DELETE,
            Permission::PRODUCTS_LIST,
            Permission::PRODUCTS_MANAGE,
        ]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        register_seo_target_provider(extensions, seo_targets::ProductSeoTargetProvider).map_err(
            |error| {
                rustok_core::Error::Validation(format!(
                    "product SEO target registration failed: {error}"
                ))
            },
        )
    }
}

impl MigrationSource for ProductModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod contract_tests;
