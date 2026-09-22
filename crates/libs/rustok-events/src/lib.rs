//! Canonical event contracts crate for RusToK.

mod blog_comments_schedule_audit;
mod contract;
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
    BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_SCHEMAS, BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE,
    BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION, BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY,
    BlogCommentsDelegationScheduleAuditEvent, blog_comments_schedule_audit_event_schema,
};
pub use contract::{
    ContractEventEnvelope, ContractEventPayload, EventContract, EventContractEnvelopeError,
};
pub use forum_mention::{
    FORUM_MENTION_EVENT_SCHEMAS, ForumMentionEvent, forum_mention_event_schema,
};
pub use forum_search_projection::{
    FORUM_SEARCH_PROJECTION_EVENT_SCHEMAS, ForumSearchProjectionEvent,
    forum_search_projection_event_schema,
};
pub use marketplace_listing::{
    MARKETPLACE_LISTING_EVENT_SCHEMAS, MarketplaceListingEvent, marketplace_listing_event_schema,
};
pub use marketplace_seller::{
    MARKETPLACE_SELLER_EVENT_SCHEMAS, MarketplaceSellerEvent, marketplace_seller_event_schema,
};
pub use product_index_refresh::{
    MAX_PRODUCT_INDEX_REFRESH_LOCALE_BYTES, PRODUCT_INDEX_LOCALE_REFRESH_REQUESTED_EVENT_TYPE,
    PRODUCT_INDEX_REFRESH_EVENT_SCHEMA_VERSION, PRODUCT_INDEX_REFRESH_EVENT_SCHEMAS,
    PRODUCT_INDEX_VARIANT_REFRESH_REQUESTED_EVENT_TYPE, ProductIndexRefreshEvent,
    product_index_refresh_event_schema,
};
pub use rbac_artifact_permission::{
    RBAC_ARTIFACT_PERMISSION_EVENT_SCHEMAS, RbacArtifactPermissionEvent,
    rbac_artifact_permission_event_schema,
};
pub use rbac_role_mutation::{
    RBAC_EVENT_USER_ROLE_ASSIGNMENT_REPAIRED, RBAC_EVENT_USER_ROLE_REPLACED,
    RBAC_ROLE_MUTATION_EVENT_SCHEMAS, RbacRoleMutationEvent, rbac_role_mutation_event_schema,
};
pub use reactions::{
    MAX_REACTIONS_EVENT_KEYS, REACTIONS_ACTOR_STATE_CHANGED_EVENT_TYPE,
    REACTIONS_EVENT_SCHEMA_VERSION, REACTIONS_EVENT_SCHEMAS,
    REACTIONS_SUBJECT_RECONCILED_EVENT_TYPE, ReactionsEvent, reactions_event_schema,
};
pub use schema::{
    EVENT_SCHEMAS, EventContractDigests, EventSchema, FieldSchema,
    contract_event_envelope_json_schema, contract_event_payload_json_schema,
    domain_event_json_schema, event_contract_digests, event_envelope_json_schema,
};
pub use social_graph::{
    SOCIAL_GRAPH_RELATION_EVENT_SCHEMAS, SocialGraphRelationEvent,
    social_graph_relation_event_schema,
};
pub use translation_workflow::{
    TRANSLATION_WORKFLOW_EVENT_SCHEMAS, TranslationWorkflowEvent, translation_workflow_event_schema,
};
pub use types::{DomainEvent, EventEnvelope, EventEnvelopeError};
pub use validation::{EventValidationError, ValidateEvent};

pub use DomainEvent as RootDomainEvent;
pub use EventEnvelope as RootEventEnvelope;

pub fn event_schema(event_type: &str) -> Option<&'static EventSchema> {
    schema::event_schema(event_type)
        .or_else(|| blog_comments_schedule_audit_event_schema(event_type))
        .or_else(|| forum_mention_event_schema(event_type))
        .or_else(|| forum_search_projection_event_schema(event_type))
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
