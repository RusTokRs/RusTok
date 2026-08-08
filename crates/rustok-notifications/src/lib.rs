mod candidate;
pub mod entities;
pub mod error;
mod fanout;
mod fanout_worker;
#[cfg(feature = "server")]
pub mod graphql;
mod inbox;
mod inbox_bulk;
mod inbox_count;
mod inbox_group;
mod inbox_group_state;
mod inbox_group_summary;
mod inbox_reconcile;
mod inbox_selected;
mod inbox_state;
mod inbox_storefront_port;
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
    NotificationTenantCapabilityCommitDecision, NotificationTenantCapabilityCommitError,
    NotificationTenantCapabilityCommitGuard, NotificationTenantCapabilityCommitRequest,
};
pub use error::{NotificationError, NotificationResult};
pub use fanout::{
    NotificationFanoutPageResult, NotificationFanoutService, NotificationSourceInboxReceipt,
};
pub use fanout_worker::{
    DEFAULT_NOTIFICATION_FANOUT_BATCH_SIZE, DEFAULT_NOTIFICATION_FANOUT_PAGE_SIZE,
    MAX_NOTIFICATION_FANOUT_BATCH_SIZE, MAX_NOTIFICATION_FANOUT_PAGE_SIZE,
    NotificationFanoutJobWorkItem, NotificationFanoutPolicyDeferral,
    NotificationFanoutSourceWorkItem, NotificationFanoutWorker,
    NotificationFanoutWorkerBatchResult, NotificationFanoutWorkerFailure,
    NotificationFanoutWorkerStage,
};
#[cfg(feature = "server")]
pub use graphql::{
    GqlNotificationInboxGroupStateAction, GqlNotificationInboxGroupStatePage,
    GqlNotificationInboxUnreadCount, NotificationsMutation, NotificationsQuery,
};
pub use inbox::{
    DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE, MAX_NOTIFICATION_INBOX_CURSOR_BYTES,
    MAX_NOTIFICATION_INBOX_PAGE_SIZE, NotificationInboxItem, NotificationInboxListRequest,
    NotificationInboxListService, NotificationInboxOpenDecision, NotificationInboxOpenRequest,
    NotificationInboxOpenService, NotificationInboxPage,
};
pub use inbox_bulk::{
    NotificationInboxMarkAllArchivePage, NotificationInboxMarkAllArchiveRequest,
    NotificationInboxMarkAllArchiveService, NotificationInboxMarkAllReadPage,
    NotificationInboxMarkAllReadRequest, NotificationInboxMarkAllReadService,
    NotificationInboxMarkAllUnreadPage, NotificationInboxMarkAllUnreadRequest,
    NotificationInboxMarkAllUnreadService,
};
pub use inbox_count::{
    NotificationInboxUnreadCount, NotificationInboxUnreadCountRequest,
    NotificationInboxUnreadCountService,
};
pub use inbox_group::{
    MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES, NotificationInboxGroupListRequest,
    NotificationInboxGroupListService,
};
pub use inbox_group_state::{
    NotificationInboxGroupStateAction, NotificationInboxGroupStatePage,
    NotificationInboxGroupStateRequest, NotificationInboxGroupStateService,
};
pub use inbox_group_summary::{
    NotificationInboxGroupSummary, NotificationInboxGroupSummaryPage,
    NotificationInboxGroupSummaryRequest, NotificationInboxGroupSummaryService,
};
pub use inbox_reconcile::{
    NotificationInboxReconcileInspectionPage, NotificationInboxReconcilePage,
    NotificationInboxReconcileRequest, NotificationInboxReconcileService,
};
pub use inbox_selected::{
    MAX_NOTIFICATION_INBOX_SELECTED_IDS, NotificationInboxSelectedAction,
    NotificationInboxSelectedStateRequest, NotificationInboxSelectedStateResult,
    NotificationInboxSelectedStateService,
};
pub use inbox_state::{
    NotificationInboxStateDecision, NotificationInboxStateRequest, NotificationInboxStateService,
    NotificationInboxStateSnapshot,
};
pub use inbox_storefront_port::{
    NotificationInboxStorefrontGroupItemsRequest, NotificationInboxStorefrontGroupStateRequest,
    NotificationInboxStorefrontGroupSummaryRequest, NotificationInboxStorefrontOpenDecision,
    NotificationInboxStorefrontOpenRequest, NotificationInboxStorefrontPort,
    NotificationInboxStorefrontService, in_process_notification_inbox_storefront_port,
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
    NotificationCandidateBatchResult, NotificationCandidatePolicyDeferral,
    NotificationCandidateWorkItem, NotificationCandidateWorker, NotificationCandidateWorkerFailure,
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
        assert_eq!(module.migrations().len(), 7);
        assert_eq!(module.migration_dependencies().len(), 7);

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
