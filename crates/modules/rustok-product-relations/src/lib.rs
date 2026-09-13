use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod services;

pub use dto::*;
pub use error::{ProductRelationError, ProductRelationResult};
pub use ports::ProductRelationsPort;
pub use services::ProductRelationService;

pub struct ProductRelationsModule;

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

impl MigrationSource for ProductRelationsModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
