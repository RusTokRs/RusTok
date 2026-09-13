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
pub use error::{BrandError, BrandResult};
pub use ports::BrandPort;
pub use services::BrandService;

pub struct BrandModule;

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

impl MigrationSource for BrandModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
