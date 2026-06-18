use async_trait::async_trait;
use rustok_core::permissions::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod services;

pub use dto::*;
pub use entities::*;
pub use error::{CustomerError, CustomerResult};
pub use ports::*;
pub use services::CustomerService;

pub struct CustomerModule;

#[async_trait]
impl RusToKModule for CustomerModule {
    fn slug(&self) -> &'static str {
        "customer"
    }

    fn name(&self) -> &'static str {
        "Customer"
    }

    fn description(&self) -> &'static str {
        "Default storefront customer submodule in the ecommerce family"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::CUSTOMERS_CREATE,
            Permission::CUSTOMERS_READ,
            Permission::CUSTOMERS_UPDATE,
            Permission::CUSTOMERS_DELETE,
            Permission::CUSTOMERS_LIST,
            Permission::CUSTOMERS_MANAGE,
        ]
    }
}

impl MigrationSource for CustomerModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
