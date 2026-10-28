//! # RusToK Fulfillment
//!
//! Fulfillment domain module extracted from commerce.
//! Provides shipping-option schemas, carrier provider SPI abstraction, fulfillment lifecycle,
//! checkout execution ports, and item-level delivery progress tracking.

mod admin_command;
mod admin_create_command;
pub mod checkout_execution;
mod checkout_execution_typed;
pub mod dto;
pub mod entities;
pub mod error;
mod fulfillment_read;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod providers;
pub mod services;
mod shipment_stock;
mod shipping_option_admin_command;
mod shipping_option_read;
pub mod status;
mod translation_changes;
mod translation_target;

#[cfg(test)]
mod contract_tests;

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
    EnsureCheckoutFulfillmentsRequest, ReadCheckoutFulfillmentsRequest,
};
pub use checkout_execution_typed::{
    TypedCheckoutFulfillmentExecutionPort, in_process_checkout_fulfillment_execution_port,
};
pub use dto::{
    CancelFulfillmentInput, CreateFulfillmentInput, CreateFulfillmentItemInput,
    CreateShippingOptionInput, DeliverFulfillmentInput, FulfillmentItemQuantityInput,
    FulfillmentItemResponse, FulfillmentResponse, ListFulfillmentsInput, ReopenFulfillmentInput,
    ReshipFulfillmentInput, ShipFulfillmentInput, ShippingOptionResponse,
    ShippingOptionTranslationInput, ShippingOptionTranslationResponse, UpdateShippingOptionInput,
};
pub use entities::fulfillment as fulfillment_entity;
pub use entities::fulfillment_item as fulfillment_item_entity;
pub use entities::shipping_option as shipping_option_entity;
pub use fulfillment_read::{
    FindLatestFulfillmentByOrderProjectionRequest, FulfillmentProjectionPage, FulfillmentReadPort,
    InProcessFulfillmentReadPort, ListFulfillmentProjectionsRequest,
    ReadFulfillmentProjectionRequest, in_process_fulfillment_read_port,
};
pub use module::FulfillmentModule;
pub use ports::{
    ListSellerShippingOptionsRequest, SelectShippingOptionPortRequest,
    SelectedShippingOptionSnapshot, SellerShippingOptionsSnapshot, ShippingOptionProjection,
    ShippingSelectionPort,
};
pub use providers::{
    ExternalFulfillmentProviderRegistration, FulfillmentProvider, FulfillmentProviderCapabilities,
    FulfillmentProviderDegradedMode, FulfillmentProviderDescriptor, FulfillmentProviderHealth,
    FulfillmentProviderOperationRequest, FulfillmentProviderOperationResult,
    FulfillmentProviderRegistry, FulfillmentProviderRuntimeMode, FulfillmentRateQuote,
    FulfillmentRateQuoteRequest, MANUAL_FULFILLMENT_PROVIDER_ID, ManualFulfillmentProvider,
};
pub use shipment_stock::{
    NoopShipmentStockPort, ShipmentStockError, ShipmentStockPort, ShipmentStockRequest,
    UnconfiguredShipmentStockPort,
};
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
pub use status::FulfillmentStatusKind;
pub use translation_changes::{
    ShippingOptionTranslationChangeLifecycle, ShippingOptionTranslationChangeRecord,
};
pub use translation_target::{
    ShippingOptionTranslationTargetProvider, register_shipping_option_translation_target_provider,
};

pub use error::{FulfillmentError, FulfillmentResult};
pub use services::{
    BeginProviderOperation, FulfillmentProviderOperationJournal,
    FulfillmentProviderOperationRecovery, FulfillmentService, MAX_PROVIDER_OPERATION_PAYLOAD_BYTES,
    PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_EXECUTING,
    PROVIDER_OPERATION_PENDING, PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
    PROVIDER_OPERATION_SUCCEEDED, ShippingOptionTranslationExactLocaleApply,
    ShippingOptionTranslationExactLocaleApplyReceipt, ShippingOptionTranslationExactLocaleError,
    ShippingOptionTranslationExactLocaleRecord, ShippingOptionTranslationExactLocaleResult,
    ShippingOptionTranslationExactLocaleSnapshot, ShippingOptionTranslationService,
};
