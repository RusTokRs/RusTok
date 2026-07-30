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
use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use rustok_seo_targets::register_seo_target_provider;
use sea_orm_migration::MigrationTrait;

pub mod dto;
pub mod entities;
pub mod error;
#[cfg(feature = "index")]
pub mod index;
pub mod migrations;
pub mod ports;
mod public_error;
mod runtime;
mod seo_targets;
pub mod services;

pub use error::{CommerceError, CommerceResult};
#[cfg(feature = "index")]
pub use index::{
    PRODUCT_INDEX_ENTITY, PRODUCT_INDEX_MODULE, PRODUCT_INDEX_SOURCE,
    PRODUCT_INDEX_SOURCE_FACTORY, ProductIndexError, ProductPostgresIndexSource,
    ProductPostgresIndexSourceFactory, product_index_schema,
};
pub use ports::*;
pub use public_error::{ProductPublicError, map_product_public_error};
pub use runtime::{ProductCatalogReadProfile, ProductCatalogReadRuntime};
pub use services::{
    AdminProductList, AdminProductListItem, AdminProductListQuery, CatalogService,
    ProductAttributeFilter, ProductCatalogSchemaService, StorefrontProductList,
    StorefrontProductListItem, StorefrontProductListQuery, StorefrontProductSortBy,
    StorefrontProductSortDirection,
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
        #[cfg(feature = "index")]
        {
            &["taxonomy", "index"]
        }
        #[cfg(not(feature = "index"))]
        {
            &["taxonomy"]
        }
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
        )?;

        #[cfg(feature = "index")]
        {
            let schema = index::product_index_schema().map_err(|error| {
                rustok_core::Error::Validation(format!(
                    "Product Index schema construction failed: {error}"
                ))
            })?;
            rustok_index::register_index_schema_source(extensions, self.slug(), schema).map_err(
                |error| {
                    rustok_core::Error::Validation(format!(
                        "Product Index schema source registration failed: {error}"
                    ))
                },
            )?;
            rustok_index::register_postgres_index_source_factory(
                extensions,
                self.slug(),
                index::PRODUCT_INDEX_SOURCE_FACTORY,
                index::ProductPostgresIndexSourceFactory,
            )
            .map_err(|error| {
                rustok_core::Error::Validation(format!(
                    "Product Index source factory registration failed: {error}"
                ))
            })?;
        }

        Ok(())
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
