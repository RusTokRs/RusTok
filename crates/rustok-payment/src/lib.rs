use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod providers;
pub mod services;

pub use dto::*;
pub use entities::*;
pub use ports::*;
pub use providers::*;

pub use error::{PaymentError, PaymentResult};
pub use services::PaymentService;

pub struct PaymentModule;

#[async_trait]
impl RusToKModule for PaymentModule {
    fn slug(&self) -> &'static str {
        "payment"
    }

    fn name(&self) -> &'static str {
        "Payment"
    }

    fn description(&self) -> &'static str {
        "Default payment submodule in the ecommerce family"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::PAYMENTS_CREATE,
            Permission::PAYMENTS_READ,
            Permission::PAYMENTS_UPDATE,
            Permission::PAYMENTS_DELETE,
            Permission::PAYMENTS_LIST,
            Permission::PAYMENTS_MANAGE,
        ]
    }
}

impl MigrationSource for PaymentModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
