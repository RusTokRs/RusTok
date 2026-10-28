//! # RusToK Events
//!
//! Canonical event contracts and schemas library for the RusToK platform.
//!
//! `rustok-events` owns the shared event envelope definitions, root domain events,
//! bounded-family event contracts, and schema validation logic used across sovereign domain
//! modules and the host server.
//!
//! ## Architecture & Components
//!
//! - **Event Envelopes & Types**: [`EventEnvelope`] (and [`RootEventEnvelope`]) wrap root [`DomainEvent`]
//!   records carrying correlation, causation, tenant scope, and actor identity.
//! - **Bounded Typed Contracts**: [`ContractEventEnvelope`] and [`EventContract`] provide sealed,
//!   strongly-typed event payloads for high-throughput and cross-module workflows.
//! - **Schema Validation**: [`EventSchema`], [`FieldSchema`], [`ValidateEvent`], and [`event_schema`]
//!   enforce schema-driven fail-closed validation on serialization and deserialization.
//! - **Contract Digests**: [`event_contract_digests`] guarantees tamper-proof verification of all
//!   versioned event schemas against the committed release artifact.

mod blog_comments_schedule_audit;
mod checkout_operation;
mod contract;
mod forms_submission;
mod forum_mention;
mod forum_search_projection;
mod marketplace_listing;
mod marketplace_seller;
mod product_index_refresh;
mod rbac_artifact_permission;
mod rbac_role_mutation;
mod reactions;
mod schema;
mod social_graph;
mod translation_workflow;
mod types;
pub mod validation;

pub use blog_comments_schedule_audit::{
    blog_comments_schedule_audit_event_schema, BlogCommentsDelegationScheduleAuditEvent,
    BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_SCHEMAS, BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE,
    BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION, BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY,
};
pub use checkout_operation::{
    checkout_operation_event_schema, CheckoutOperationEvent, CHECKOUT_OPERATION_ADMISSIONS,
    CHECKOUT_OPERATION_ADMISSION_CHANGED_EVENT_TYPE, CHECKOUT_OPERATION_ADMISSION_CLOSED,
    CHECKOUT_OPERATION_ADMISSION_OPEN, CHECKOUT_OPERATION_ADMISSION_SETTLING,
    CHECKOUT_OPERATION_EVENT_SCHEMAS, CHECKOUT_OPERATION_EVENT_SCHEMA_VERSION,
    CHECKOUT_OPERATION_OUTCOMES, CHECKOUT_OPERATION_OUTCOME_COMPENSATED,
    CHECKOUT_OPERATION_OUTCOME_COMPENSATION_REQUIRED, CHECKOUT_OPERATION_OUTCOME_FAILED,
    CHECKOUT_OPERATION_PARKED_EVENT_TYPE, CHECKOUT_OPERATION_PARK_REASONS,
    CHECKOUT_OPERATION_PARK_REASON_ATTEMPTS_EXHAUSTED,
    CHECKOUT_OPERATION_PARK_REASON_MANUAL_RECONCILIATION, CHECKOUT_OPERATION_RECONCILED_EVENT_TYPE,
};
pub use contract::{
    ContractEventEnvelope, ContractEventPayload, EventContract, EventContractEnvelopeError,
};
pub use forms_submission::{
    forms_submission_event_schema, FormSubmissionEvent, FORMS_SUBMISSION_EVENT_SCHEMAS,
};
pub use forum_mention::{
    forum_mention_event_schema, ForumMentionEvent, FORUM_MENTION_EVENT_SCHEMAS,
};
pub use forum_search_projection::{
    forum_search_projection_event_schema, ForumSearchProjectionEvent,
    FORUM_SEARCH_PROJECTION_EVENT_SCHEMAS,
};
pub use marketplace_listing::{
    marketplace_listing_event_schema, MarketplaceListingEvent, MARKETPLACE_LISTING_EVENT_SCHEMAS,
};
pub use marketplace_seller::{
    marketplace_seller_event_schema, MarketplaceSellerEvent, MARKETPLACE_SELLER_EVENT_SCHEMAS,
};
pub use product_index_refresh::{
    product_index_refresh_event_schema, ProductIndexRefreshEvent,
    MAX_PRODUCT_INDEX_REFRESH_LOCALE_BYTES, PRODUCT_INDEX_LOCALE_REFRESH_REQUESTED_EVENT_TYPE,
    PRODUCT_INDEX_REFRESH_EVENT_SCHEMAS, PRODUCT_INDEX_REFRESH_EVENT_SCHEMA_VERSION,
    PRODUCT_INDEX_VARIANT_REFRESH_REQUESTED_EVENT_TYPE,
};
pub use rbac_artifact_permission::{
    rbac_artifact_permission_event_schema, RbacArtifactPermissionEvent,
    RBAC_ARTIFACT_PERMISSION_EVENT_SCHEMAS,
};
pub use rbac_role_mutation::{
    rbac_role_mutation_event_schema, RbacRoleMutationEvent,
    RBAC_EVENT_USER_ROLE_ASSIGNMENT_REPAIRED, RBAC_EVENT_USER_ROLE_REPLACED,
    RBAC_ROLE_MUTATION_EVENT_SCHEMAS,
};
pub use reactions::{
    reactions_event_schema, ReactionsEvent, MAX_REACTIONS_EVENT_KEYS,
    REACTIONS_ACTOR_STATE_CHANGED_EVENT_TYPE, REACTIONS_EVENT_SCHEMAS,
    REACTIONS_EVENT_SCHEMA_VERSION, REACTIONS_SUBJECT_RECONCILED_EVENT_TYPE,
};
pub use schema::{
    contract_event_envelope_json_schema, contract_event_payload_json_schema,
    domain_event_json_schema, event_contract_digests, event_envelope_json_schema,
    EventContractDigests, EventSchema, FieldSchema, EVENT_SCHEMAS,
};
pub use social_graph::{
    social_graph_relation_event_schema, SocialGraphRelationEvent,
    SOCIAL_GRAPH_RELATION_EVENT_SCHEMAS,
};
pub use translation_workflow::{
    translation_workflow_event_schema, TranslationWorkflowEvent, TRANSLATION_WORKFLOW_EVENT_SCHEMAS,
};
pub use types::{DomainEvent, EventEnvelope, EventEnvelopeError};
pub use validation::{EventValidationError, ValidateEvent};

pub use DomainEvent as RootDomainEvent;
pub use EventEnvelope as RootEventEnvelope;

pub fn event_schema(event_type: &str) -> Option<&'static EventSchema> {
    schema::event_schema(event_type)
        .or_else(|| blog_comments_schedule_audit_event_schema(event_type))
        .or_else(|| checkout_operation_event_schema(event_type))
        .or_else(|| forum_mention_event_schema(event_type))
        .or_else(|| forum_search_projection_event_schema(event_type))
        .or_else(|| forms_submission_event_schema(event_type))
        .or_else(|| marketplace_listing_event_schema(event_type))
        .or_else(|| marketplace_seller_event_schema(event_type))
        .or_else(|| product_index_refresh_event_schema(event_type))
        .or_else(|| rbac_artifact_permission_event_schema(event_type))
        .or_else(|| rbac_role_mutation_event_schema(event_type))
        .or_else(|| reactions_event_schema(event_type))
        .or_else(|| social_graph_relation_event_schema(event_type))
        .or_else(|| translation_workflow_event_schema(event_type))
}

pub fn event_schemas() -> impl Iterator<Item = &'static EventSchema> {
    EVENT_SCHEMAS
        .iter()
        .chain(BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_SCHEMAS.iter())
        .chain(CHECKOUT_OPERATION_EVENT_SCHEMAS.iter())
        .chain(FORUM_MENTION_EVENT_SCHEMAS.iter())
        .chain(FORUM_SEARCH_PROJECTION_EVENT_SCHEMAS.iter())
        .chain(MARKETPLACE_LISTING_EVENT_SCHEMAS.iter())
        .chain(MARKETPLACE_SELLER_EVENT_SCHEMAS.iter())
        .chain(PRODUCT_INDEX_REFRESH_EVENT_SCHEMAS.iter())
        .chain(RBAC_ARTIFACT_PERMISSION_EVENT_SCHEMAS.iter())
        .chain(RBAC_ROLE_MUTATION_EVENT_SCHEMAS.iter())
        .chain(REACTIONS_EVENT_SCHEMAS.iter())
        .chain(SOCIAL_GRAPH_RELATION_EVENT_SCHEMAS.iter())
        .chain(TRANSLATION_WORKFLOW_EVENT_SCHEMAS.iter())
}

#[cfg(test)]
mod contract_tests;
