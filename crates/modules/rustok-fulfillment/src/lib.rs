use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

mod admin_command;
mod admin_create_command;
pub mod checkout_execution;
mod checkout_execution_typed;
pub mod dto;
pub mod entities;
pub mod error;
mod fulfillment_read;
pub mod migrations;
pub mod ports;
pub mod providers;
pub mod services;
mod shipping_option_admin_command;
mod shipping_option_read;
pub mod status;

pub use admin_command::{
    CancelAdminFulfillmentRequest, DeliverAdminFulfillmentRequest, FulfillmentAdminCommandPort,
    FulfillmentAdminCommandRuntime, InProcessFulfillmentAdminCommandPort,
    ReopenAdminFulfillmentRequest, ReshipAdminFulfillmentRequest, ShipAdminFulfillmentRequest,
    in_process_fulfillment_admin_command_port,
};
pub use admin_create_command::{
    CreateAdminFulfillmentRequest, FulfillmentAdminCreateCommandPort,
    FulfillmentAdminCreateCommandRuntime, InProcessFulfillmentAdminCreateCommandPort,
    in_process_fulfillment_admin_create_command_port,
};
pub use checkout_execution::{
    CheckoutFulfillmentCommand, CheckoutFulfillmentExecutionPort, CheckoutFulfillmentItemCommand,
    EnsureCheckoutFulfillmentsRequest, InProcessCheckoutFulfillmentExecutionPort,
    ReadCheckoutFulfillmentsRequest,
};
pub use checkout_execution_typed::{
    TypedCheckoutFulfillmentExecutionPort, in_process_checkout_fulfillment_execution_port,
};
pub use dto::*;
pub use entities::*;
pub use fulfillment_read::{
    FindLatestFulfillmentByOrderProjectionRequest, FulfillmentProjectionPage, FulfillmentReadPort,
    InProcessFulfillmentReadPort, ListFulfillmentProjectionsRequest,
    ReadFulfillmentProjectionRequest, in_process_fulfillment_read_port,
};
pub use ports::*;
pub use providers::*;
pub use shipping_option_admin_command::{
    CreateAdminShippingOptionRequest, DeactivateAdminShippingOptionRequest,
    InProcessShippingOptionAdminCommandPort, ReactivateAdminShippingOptionRequest,
    ShippingOptionAdminCommandPort, ShippingOptionAdminCommandRuntime,
    UpdateAdminShippingOptionRequest, in_process_shipping_option_admin_command_port,
};
pub use shipping_option_read::{
    InProcessShippingOptionAdminReadPort, InProcessShippingOptionReadPort,
    ListAllShippingOptionProjectionsRequest, ListShippingOptionProjectionsRequest,
    ReadShippingOptionProjectionRequest, ShippingOptionAdminReadPort, ShippingOptionReadPort,
    in_process_shipping_option_admin_read_port, in_process_shipping_option_read_port,
};
pub use status::*;

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
