use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use sea_orm_migration::MigrationTrait;

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

pub struct SocialGraphModule;

#[async_trait]
impl RusToKModule for SocialGraphModule {
    fn slug(&self) -> &'static str {
        "social_graph"
    }

    fn name(&self) -> &'static str {
        "Social Graph"
    }

    fn description(&self) -> &'static str {
        "Tenant-scoped social relation owner for blocks, mutes, follows, and friendship policy"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        #[cfg(feature = "index")]
        {
            &["index", "outbox"]
        }
        #[cfg(not(feature = "index"))]
        {
            &["outbox"]
        }
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        #[cfg(feature = "index")]
        {
            let schema = index::social_graph_relation_index_schema().map_err(|error| {
                rustok_core::Error::Validation(format!(
                    "Social Graph Index schema construction failed: {error}"
                ))
            })?;
            rustok_index::register_index_schema_source(extensions, self.slug(), schema).map_err(
                |error| {
                    rustok_core::Error::Validation(format!(
                        "Social Graph Index schema source registration failed: {error}"
                    ))
                },
            )?;
            index_source::register_social_graph_index_source_contracts(extensions).map_err(
                |error| {
                    rustok_core::Error::Validation(format!(
                        "Social Graph Index source contract registration failed: {error}"
                    ))
                },
            )?;
        }
        #[cfg(not(feature = "index"))]
        let _ = extensions;
        Ok(())
    }
}

impl MigrationSource for SocialGraphModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};

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
