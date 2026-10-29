//! # RusToK Marketplace Payout
//!
//! Native module owner for Marketplace Family seller payout scheduling and ledger-entry assignments.
//! Coordinates seller payout batches, payout provider dispatches, and settlement lifecycle tracking.

#![deny(clippy::float_arithmetic)]

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
    ListMarketplaceSellerPayoutsRequest, MAX_PAYOUTS_PER_PAGE, MAX_PAYOUT_ITEMS_PER_BATCH,
    MarketplacePayoutItemResponse, MarketplacePayoutListResponse, MarketplacePayoutResponse,
    MarketplacePayoutStatus, ReadMarketplacePayoutRequest, ScheduleMarketplacePayoutInput,
};
pub use error::{MarketplacePayoutError, MarketplacePayoutResult};
pub use module::MarketplacePayoutModule;
pub use ports::{MarketplacePayoutCommandPort, MarketplacePayoutReadPort};
pub use service::MarketplacePayoutService;

#[cfg(test)]
mod contract_tests;
