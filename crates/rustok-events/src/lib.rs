//! Canonical event contracts crate for RusToK.

mod contract;
mod forum_mention;
mod marketplace_listing;
mod marketplace_seller;
mod schema;
mod types;
pub mod validation;

pub use contract::{
    ContractEventEnvelope, ContractEventPayload, EventContract, EventContractEnvelopeError,
};
pub use forum_mention::{
    FORUM_MENTION_EVENT_SCHEMAS, ForumMentionEvent, forum_mention_event_schema,
};
pub use marketplace_listing::{
    MARKETPLACE_LISTING_EVENT_SCHEMAS, MarketplaceListingEvent, marketplace_listing_event_schema,
};
pub use marketplace_seller::{
    MARKETPLACE_SELLER_EVENT_SCHEMAS, MarketplaceSellerEvent, marketplace_seller_event_schema,
};
pub use schema::{
    EVENT_SCHEMAS, EventContractDigests, EventSchema, FieldSchema,
    contract_event_envelope_json_schema, contract_event_payload_json_schema,
    domain_event_json_schema, event_contract_digests, event_envelope_json_schema,
};
pub use types::{DomainEvent, EventEnvelope, EventEnvelopeError};
pub use validation::{EventValidationError, ValidateEvent};

pub use DomainEvent as RootDomainEvent;
pub use EventEnvelope as RootEventEnvelope;

pub fn event_schema(event_type: &str) -> Option<&'static EventSchema> {
    schema::event_schema(event_type)
        .or_else(|| forum_mention_event_schema(event_type))
        .or_else(|| marketplace_listing_event_schema(event_type))
        .or_else(|| marketplace_seller_event_schema(event_type))
}

pub fn event_schemas() -> impl Iterator<Item = &'static EventSchema> {
    EVENT_SCHEMAS
        .iter()
        .chain(FORUM_MENTION_EVENT_SCHEMAS.iter())
        .chain(MARKETPLACE_LISTING_EVENT_SCHEMAS.iter())
        .chain(MARKETPLACE_SELLER_EVENT_SCHEMAS.iter())
}

#[cfg(test)]
mod contract_tests;
