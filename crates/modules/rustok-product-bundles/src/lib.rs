use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod services;

pub use dto::*;
pub use error::{BundleError, BundleResult};
pub use ports::BundlePort;
pub use services::BundleService;

pub struct ProductBundlesModule;

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

    fn dependencies(&self) -> &[&'static str] {
        &["product"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::PRODUCTS_READ,
            Permission::PRODUCTS_UPDATE,
        ]
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
