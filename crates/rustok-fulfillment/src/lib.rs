use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod checkout_execution;
pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod providers;
pub mod services;

pub use checkout_execution::*;
pub use dto::*;
pub use entities::*;
pub use ports::*;
pub use providers::*;

pub use error::{FulfillmentError, FulfillmentResult};
pub use services::{
    BeginProviderOperation, FulfillmentProviderOperationJournal,
    FulfillmentProviderOperationRecovery, FulfillmentService, PROVIDER_OPERATION_COMMITTED,
    PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_PENDING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED,
};

pub struct FulfillmentModule;

#[async_trait]
impl RusToKModule for FulfillmentModule {
    fn slug(&self) -> &'static str {
        "fulfillment"
    }

    fn name(&self) -> &'static str {
        "Fulfillment"
    }

    fn description(&self) -> &'static str {
        "Default fulfillment submodule in the ecommerce family"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::FULFILLMENTS_CREATE,
            Permission::FULFILLMENTS_READ,
            Permission::FULFILLMENTS_UPDATE,
            Permission::FULFILLMENTS_DELETE,
            Permission::FULFILLMENTS_LIST,
            Permission::FULFILLMENTS_MANAGE,
        ]
    }
}

impl MigrationSource for FulfillmentModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
