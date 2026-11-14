//! # RusToK Marketplace Listing
//!
//! Native module owner for Marketplace Family seller listing identity, versioned commercial terms,
//! approval workflows, lifecycle states, and deterministic eligibility projections.

mod command_receipts;
#[cfg(test)]
mod command_receipts_tests;
mod evented_commands;
mod external_events;
mod lifecycle_event_commands;
mod listing_events;
mod replay_safe_commands;

pub mod dto;
pub mod entities;
pub mod error;
#[cfg(feature = "graphql")]
pub mod graphql;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod service;

pub use dto::{
    CreateMarketplaceListingInput, ListMarketplaceListingEventsRequest,
    ListMarketplaceListingsInput, MarketplaceListingApprovalStatus,
    MarketplaceListingEligibilityProjection, MarketplaceListingEligibilityRequest,
    MarketplaceListingEventKind, MarketplaceListingEventProvenance,
    MarketplaceListingEventResponse, MarketplaceListingListResponse, MarketplaceListingResponse,
    MarketplaceListingStatus, MarketplaceListingTermsResponse, ReadMarketplaceListingRequest,
    ReviewMarketplaceListingInput, SuspendMarketplaceListingInput,
    UpdateMarketplaceListingTermsInput,
};
pub use error::{MarketplaceListingError, MarketplaceListingResult};
pub use module::MarketplaceListingModule;
pub use ports::{
    MarketplaceListingCommandPort, MarketplaceListingIdRequest, MarketplaceListingPorts,
    MarketplaceListingReadPort, MarketplaceListingRuntime,
};
pub const MARKETPLACE_LISTINGS_CREATE: rustok_api::Permission =
    rustok_api::Permission::MARKETPLACE_LISTINGS_CREATE;
pub const MARKETPLACE_LISTINGS_READ: rustok_api::Permission =
    rustok_api::Permission::MARKETPLACE_LISTINGS_READ;
pub const MARKETPLACE_LISTINGS_UPDATE: rustok_api::Permission =
    rustok_api::Permission::MARKETPLACE_LISTINGS_UPDATE;
pub const MARKETPLACE_LISTINGS_LIST: rustok_api::Permission =
    rustok_api::Permission::MARKETPLACE_LISTINGS_LIST;
pub const MARKETPLACE_LISTINGS_MANAGE: rustok_api::Permission =
    rustok_api::Permission::MARKETPLACE_LISTINGS_MANAGE;
pub const MARKETPLACE_LISTINGS_PUBLISH: rustok_api::Permission =
    rustok_api::Permission::MARKETPLACE_LISTINGS_PUBLISH;
pub const MARKETPLACE_LISTINGS_MODERATE: rustok_api::Permission =
    rustok_api::Permission::MARKETPLACE_LISTINGS_MODERATE;
pub use service::MarketplaceListingService;

#[cfg(test)]
mod contract_tests;
