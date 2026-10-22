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

pub use dto::*;
pub use entities::*;
pub use error::{CustomerError, CustomerResult};
pub use module::CustomerModule;
pub use ports::{
    CustomerListProjectionRequest, CustomerListProjectionResponse, CustomerProfileEnrichment,
    CustomerProfileEnrichmentRequest, CustomerProjectionRequest, CustomerReadPort,
    CustomerUserProjectionRequest,
};
pub use read_context::{InProcessCustomerReadPort, in_process_customer_read_port};
pub use services::CustomerService;
