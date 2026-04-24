use async_trait::async_trait;
use rustok_core::permissions::Permission;
use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use rustok_seo_targets::register_seo_target_provider;
use sea_orm_migration::MigrationTrait;

pub mod entities;
pub mod migrations;
mod seo_targets;
pub mod services;

pub use services::{CatalogService, StorefrontProductList, StorefrontProductListItem};

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

    fn register_runtime_extensions(&self, extensions: &mut ModuleRuntimeExtensions) {
        register_seo_target_provider(extensions, seo_targets::ProductSeoTargetProvider)
            .expect("product SEO target registration should remain unique");
    }
}

impl MigrationSource for ProductModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
