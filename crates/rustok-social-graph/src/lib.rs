use async_trait::async_trait;
use rustok_core::{MigrationSource, RusToKModule};
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
pub mod maintenance;
pub mod migrations;
pub mod model;
pub mod observability;
pub mod ports;
mod receipts;
pub mod service;

pub use error::{SocialGraphError, SocialGraphResult};
pub use follow_read::{SocialGraphFollowReadPort, SocialGraphFollowState};
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
        &["outbox"]
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
    use rustok_core::{MigrationSource, RusToKModule};

    use super::SocialGraphModule;

    #[test]
    fn module_metadata_and_migrations_are_stable() {
        let module = SocialGraphModule;
        assert_eq!(module.slug(), "social_graph");
        assert_eq!(module.dependencies(), &["outbox"]);
        assert_eq!(module.migrations().len(), 3);
        assert_eq!(module.migration_dependencies().len(), 3);
    }
}
