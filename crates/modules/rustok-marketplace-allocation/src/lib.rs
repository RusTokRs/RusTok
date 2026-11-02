//! # RusToK Marketplace Allocation
//!
//! Native module owner for Marketplace Family order-line seller and listing allocations.
//! Persists immutable mapping between customer order lines, seller organizations, and listing records.

mod receipts;
#[cfg(test)]
mod tests;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod service;

pub use dto::{
    AllocateMarketplaceOrderLineInput, AllocateMarketplaceOrderLinesInput,
    AllocateMarketplaceOrderLinesResponse, ListMarketplaceAllocationsByOrderRequest,
    ListMarketplaceAllocationsBySellerRequest, MAX_ALLOCATION_LINES_PER_COMMAND,
    MAX_ALLOCATIONS_PER_PAGE, MarketplaceAllocationListResponse, MarketplaceAllocationStatus,
    MarketplaceOrderAllocationResponse, ReadMarketplaceAllocationByLineRequest,
};
pub use error::{MarketplaceAllocationError, MarketplaceAllocationResult};
pub use module::MarketplaceAllocationModule;
pub use ports::{
    MarketplaceAllocationCommandPort, MarketplaceAllocationReadPort,
    in_process_marketplace_allocation_command_port,
};
pub use service::MarketplaceAllocationService;

#[cfg(test)]
mod contract_tests;
