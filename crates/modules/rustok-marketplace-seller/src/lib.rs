//! # RusToK Marketplace Seller
//!
//! Native module owner for Marketplace Family seller identity, onboarding, lifecycle, and membership rosters.
//! Coordinates seller statuses, onboarding review, membership roles, and localized translation projections.

mod command_receipts;
mod external_events;
mod localized_sellers;
mod receipted_commands;
mod seller_events;
#[cfg(test)]
mod seller_events_tests;
#[cfg(test)]
mod seller_member_events_tests;
mod seller_prose;
#[cfg(test)]
mod seller_response_events_tests;
mod translation;
mod translation_changes;
mod translation_progress;
pub mod translation_target;

pub mod dto;
pub mod entities;
pub mod error;
#[cfg(feature = "graphql")]
pub mod graphql;
#[cfg(feature = "graphql")]
pub mod graphql_events;
#[cfg(feature = "graphql")]
pub mod graphql_root;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod service;

pub use dto::{
    AddMarketplaceSellerMemberInput, CreateMarketplaceSellerInput,
    ListMarketplaceSellerEventsRequest, ListMarketplaceSellerMembersRequest,
    ListMarketplaceSellersInput, MarketplaceSellerEventKind, MarketplaceSellerEventProvenance,
    MarketplaceSellerEventResponse, MarketplaceSellerMemberResponse, MarketplaceSellerMemberRole,
    MarketplaceSellerMemberStatus, MarketplaceSellerOnboardingStatus, MarketplaceSellerResponse,
    MarketplaceSellerStatus, ReadMarketplaceSellerMembershipRequest, ReadMarketplaceSellerRequest,
    ReviewMarketplaceSellerOnboardingInput, SubmitMarketplaceSellerOnboardingInput,
    SuspendMarketplaceSellerInput, UpdateMarketplaceSellerMemberInput,
    UpdateMarketplaceSellerProfileInput,
};
pub use error::{MarketplaceSellerError, MarketplaceSellerResult};
pub use module::MarketplaceSellerModule;
pub use ports::{
    AddMarketplaceSellerMemberRequest, MarketplaceSellerCommandPort,
    MarketplaceSellerListResponse, MarketplaceSellerReadPort, MarketplaceSellerRuntime,
    ReactivateMarketplaceSellerRequest, ReviewMarketplaceSellerOnboardingRequest,
    SubmitMarketplaceSellerOnboardingRequest, SuspendMarketplaceSellerRequest,
    UpdateMarketplaceSellerMemberRequest, UpdateMarketplaceSellerProfileRequest,
};
pub use service::MarketplaceSellerService;
pub use translation::{
    MarketplaceSellerTranslationExactLocaleApply,
    MarketplaceSellerTranslationExactLocaleApplyReceipt,
    MarketplaceSellerTranslationExactLocaleError, MarketplaceSellerTranslationExactLocaleRecord,
    MarketplaceSellerTranslationExactLocaleResult, MarketplaceSellerTranslationExactLocaleSnapshot,
    MarketplaceSellerTranslationService,
};
pub use translation_changes::{
    MarketplaceSellerTranslationChangeLifecycle, MarketplaceSellerTranslationChangeRecord,
};
pub use translation_target::{
    MarketplaceSellerTranslationTargetProvider,
    register_marketplace_seller_translation_target_provider,
};

#[cfg(test)]
mod contract_tests;
