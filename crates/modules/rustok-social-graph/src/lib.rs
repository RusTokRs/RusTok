//! # rustok-social-graph
//!
//! Tenant-scoped social relation owner for blocks, mutes, follows, and friendship policy.
//!
//! Exposes neutral command, privacy read, revision-bearing follow-state read,
//! and maintenance ports for idempotency receipts and relation event replay.

pub mod entities;
pub mod error;
mod external_events;
pub mod follow_read;
#[cfg(feature = "graphql")]
pub mod graphql;
#[cfg(feature = "index")]
pub mod index;
#[cfg(feature = "index-consumer")]
pub mod index_consumer;
#[cfg(feature = "index-consumer")]
mod index_dlq_message_id;
#[cfg(feature = "index-consumer")]
pub mod index_dlq_receipt;
#[cfg(feature = "index")]
pub mod index_privacy;
#[cfg(feature = "index")]
pub mod index_privacy_shadow;
#[cfg(feature = "index")]
pub mod index_source;
pub mod maintenance;
pub mod migrations;
pub mod model;
pub mod module;
pub mod observability;
pub mod ports;
mod receipts;
pub mod service;

pub use error::{SocialGraphError, SocialGraphResult};
pub use follow_read::{SocialGraphFollowReadPort, SocialGraphFollowState};
#[cfg(feature = "index")]
pub use index_privacy::IndexSocialGraphPrivacyReadPort;
#[cfg(feature = "index")]
pub use index_privacy_shadow::{
    IndexPrivacyShadowFailureCode, IndexPrivacyShadowObservation, IndexPrivacyShadowObserver,
    IndexPrivacyShadowOperation, IndexPrivacyShadowOutcome, IndexShadowSocialGraphPrivacyReadPort,
    SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_TARGET,
};
pub use maintenance::{
    SocialGraphReceiptMaintenanceService, SocialGraphRelationEventMaintenanceService,
};
pub use model::SocialRelationKind;
pub use observability::{
    SOCIAL_GRAPH_OPERATION_TARGET, SocialGraphCommandOperation, SocialGraphCommandTimer,
};
pub use ports::{
    MAX_SOCIAL_GRAPH_FOLLOW_TARGETS, MAX_SOCIAL_GRAPH_RECEIPT_CLEANUP_BATCH,
    MAX_SOCIAL_GRAPH_RELATION_EVENT_REPLAY_BATCH, SetSocialRelationCommand, SocialGraphCommandPort,
    SocialGraphFollowBatchRequest, SocialGraphFollowBatchResult, SocialGraphPairRequest,
    SocialGraphPrivacyReadPort, SocialGraphPrivacyRuntime, SocialGraphReceiptCleanupCommand,
    SocialGraphReceiptCleanupResult, SocialGraphReceiptMaintenancePort,
    SocialGraphRelationEventMaintenancePort, SocialGraphRelationEventReplayCommand,
    SocialGraphRelationEventReplayResult,
};
pub use service::SocialGraphService;

pub use module::SocialGraphModule;

#[cfg(test)]
mod contract_tests;


#[cfg(test)]
mod tests {
    #[cfg(feature = "index")]
    use rustok_core::ModuleRuntimeExtensions;
    use rustok_core::{MigrationSource, RusToKModule};

    use super::SocialGraphModule;

    #[test]
    fn module_metadata_and_migrations_are_stable() {
        let module = SocialGraphModule;
        assert_eq!(module.slug(), "social_graph");
        #[cfg(feature = "index")]
        assert_eq!(module.dependencies(), &["index", "outbox"]);
        #[cfg(not(feature = "index"))]
        assert_eq!(module.dependencies(), &["outbox"]);
        assert_eq!(module.migrations().len(), 4);
        assert_eq!(module.migration_dependencies().len(), 4);
    }

    #[cfg(feature = "index")]
    #[test]
    fn module_publishes_complete_index_source_contracts() {
        let mut extensions = ModuleRuntimeExtensions::default();
        SocialGraphModule
            .register_runtime_extensions(&mut extensions)
            .expect("Social Graph Index contracts should register");

        let catalog = extensions
            .get::<rustok_index::IndexSchemaSourceCatalog>()
            .expect("Index schema source catalog should be present");
        let schema = crate::index::social_graph_relation_index_schema().unwrap();
        let descriptor = catalog
            .get(&schema.reference)
            .expect("Social Graph schema should be source-published");
        assert_eq!(descriptor.owner_module, "social_graph");
        assert_eq!(descriptor.schema, schema);

        let factories = extensions
            .get::<rustok_index::PostgresIndexSourceFactoryCatalog>()
            .expect("Social Graph replay source factory should be present");
        assert!(factories.iter().any(|factory| {
            factory.owner_module() == "social_graph"
                && factory.factory_name()
                    == crate::index_source::SOCIAL_GRAPH_RELATION_INDEX_SOURCE_FACTORY
        }));

        let routes = extensions
            .get::<rustok_index::IndexMutationEventCatalog>()
            .expect("Social Graph mutation event catalog should be present");
        let route = routes
            .get(crate::index_source::SOCIAL_GRAPH_RELATION_INDEX_EVENT_DOMAIN)
            .expect("Social Graph relation event route should be present");
        assert_eq!(
            route.source_name(),
            crate::index_source::SOCIAL_GRAPH_RELATION_INDEX_SOURCE
        );
        assert_eq!(route.schema(), &schema.reference);
    }
}
