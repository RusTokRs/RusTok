//! # RusToK Customer
//!
//! Storefront customer domain module extracted from commerce.
//! Provides customer profile schema, lifecycle services, and transport-neutral
//! read-projection ports for checkout and order snapshots.

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod ports;
mod read_context;
pub mod services;

#[cfg(test)]
mod contract_tests;

pub use dto::{
    CreateCustomerInput, CustomerResponse, CustomerWithProfileResponse, ListCustomersInput,
    UpdateCustomerInput,
};
pub use entities::customer as customer_entity;
pub use error::{CustomerError, CustomerResult};
pub use module::CustomerModule;
pub use ports::{
    CustomerListProjectionRequest, CustomerListProjectionResponse, CustomerProfileEnrichment,
    CustomerProfileEnrichmentRequest, CustomerProjectionRequest, CustomerReadPort,
    CustomerUserProjectionRequest,
};
pub use read_context::{InProcessCustomerReadPort, in_process_customer_read_port};
pub use services::CustomerService;
