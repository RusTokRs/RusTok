//! Canonical event contracts crate for RusToK.

mod contract;
mod marketplace_listing;
mod marketplace_seller;
mod schema;
mod types;
pub mod validation;

pub use contract::{
    ContractEventEnvelope, ContractEventPayload, EventContract, EventContractEnvelopeError,
};
pub use marketplace_listing::{
    marketplace_listing_event_schema, MarketplaceListingEvent, MARKETPLACE_LISTING_EVENT_SCHEMAS,
};
pub use marketplace_seller::{
    marketplace_seller_event_schema, MarketplaceSellerEvent, MARKETPLACE_SELLER_EVENT_SCHEMAS,
};
pub use schema::{EventSchema, FieldSchema, EVENT_SCHEMAS};
pub use types::{DomainEvent, EventEnvelope};
pub use validation::{EventValidationError, ValidateEvent};

pub use DomainEvent as RootDomainEvent;
pub use EventEnvelope as RootEventEnvelope;

pub fn event_schema(event_type: &str) -> Option<&'static EventSchema> {
    schema::event_schema(event_type)
        .or_else(|| marketplace_listing_event_schema(event_type))
        .or_else(|| marketplace_seller_event_schema(event_type))
}

pub fn event_schemas() -> impl Iterator<Item = &'static EventSchema> {
    EVENT_SCHEMAS
        .iter()
        .chain(MARKETPLACE_LISTING_EVENT_SCHEMAS.iter())
        .chain(MARKETPLACE_SELLER_EVENT_SCHEMAS.iter())
}

#[cfg(test)]
mod contract_tests;
