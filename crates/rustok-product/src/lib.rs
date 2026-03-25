use async_trait::async_trait;
use rustok_core::permissions::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod migrations;
pub mod services;

pub use services::CatalogService;

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
}

impl MigrationSource for ProductModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
