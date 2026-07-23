mod candidate;
pub mod entities;
pub mod error;
mod fanout;
pub mod migrations;
pub mod model;
mod outbox_intake;
mod recipient_policy;
mod service;
mod worker;

use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use rustok_notifications_api::ensure_notification_source_registry;
use sea_orm_migration::MigrationTrait;

pub use candidate::{
    NotificationCandidateProcessResult, NotificationCandidateService, NotificationRecipientPolicy,
    NotificationRecipientPolicyDecision, NotificationRecipientPolicyError,
    NotificationRecipientPolicyRequest, NotificationRecipientSuppression,
};
pub use error::{NotificationError, NotificationResult};
pub use fanout::{
    NotificationFanoutPageResult, NotificationFanoutService, NotificationSourceInboxReceipt,
};
pub use outbox_intake::{
    DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE, MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE,
    NotificationOutboxEnvelopeDecoder, NotificationOutboxEnvelopeRecord,
    NotificationOutboxIntakeBatchResult, NotificationOutboxIntakeFailure,
    NotificationOutboxIntakeOutcome, NotificationOutboxIntakeRejection,
    NotificationOutboxIntakeResult, NotificationOutboxIntakeWorker,
};
pub use recipient_policy::{
    NotificationBlockReadPort, NotificationBlockReadRuntime, NotificationMuteReadPort,
    NotificationMuteReadRuntime, NotificationRecipientPolicyRuntime,
    NotificationRelationPolicyRequest,
};
pub use rustok_notifications_api as api;
pub use service::NotificationsService;
pub use worker::{
    DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE, MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE,
    NotificationCandidateBatchResult, NotificationCandidateWorker,
    NotificationCandidateWorkerFailure,
};

pub struct NotificationsModule;

#[async_trait]
impl RusToKModule for NotificationsModule {
    fn slug(&self) -> &'static str {
        "notifications"
    }

    fn name(&self) -> &'static str {
        "Notifications"
    }

    fn description(&self) -> &'static str {
        "Notification inbox, preferences, bounded fan-out, grouping, digests, and delivery orchestration"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["outbox"]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        let _ = ensure_notification_source_registry(extensions);
        Ok(())
    }
}

impl MigrationSource for NotificationsModule {
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
    use rustok_notifications_api::notification_source_registry_from_extensions;

    use super::{NotificationsModule, NotificationsService};

    #[test]
    fn module_initializes_source_registry_and_persistence_migrations() {
        let module = NotificationsModule;
        assert_eq!(module.slug(), "notifications");
        assert_eq!(module.dependencies(), &["outbox"]);
        assert_eq!(module.migrations().len(), 5);
        assert_eq!(module.migration_dependencies().len(), 5);

        let mut extensions = ModuleRuntimeExtensions::default();
        module
            .register_runtime_extensions(&mut extensions)
            .expect("notification runtime extensions should initialize");
        assert!(notification_source_registry_from_extensions(&extensions).is_some());

        let service = NotificationsService::from_runtime_extensions(&extensions);
        assert_eq!(service.source_count(), 0);
        assert!(!service.has_sources());
    }
}
