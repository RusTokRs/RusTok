pub mod keys;
pub mod model;
pub mod provider;
pub mod reconciliation;

#[cfg(feature = "server")]
pub mod domain;
#[cfg(feature = "server")]
mod candidate;
#[cfg(feature = "server")]
pub mod entities;
#[cfg(feature = "server")]
pub mod error;
#[cfg(feature = "server")]
mod fanout;
#[cfg(feature = "server")]
mod fanout_worker;
#[cfg(feature = "server")]
pub mod graphql;
#[cfg(feature = "server")]
mod inbox;
#[cfg(feature = "server")]
mod inbox_bulk;
#[cfg(feature = "server")]
mod inbox_count;
#[cfg(feature = "server")]
mod inbox_group;
#[cfg(feature = "server")]
mod inbox_group_state;
#[cfg(feature = "server")]
mod inbox_group_summary;
#[cfg(feature = "server")]
mod inbox_reconcile;
#[cfg(feature = "server")]
mod inbox_selected;
#[cfg(feature = "server")]
mod inbox_state;
#[cfg(feature = "server")]
mod inbox_storefront_port;
#[cfg(feature = "server")]
pub mod migrations;
#[cfg(feature = "server")]
pub mod module;
#[cfg(feature = "server")]
mod outbox_intake;
#[cfg(feature = "server")]
mod recipient_policy;
#[cfg(feature = "server")]
mod service;
#[cfg(feature = "server")]
mod worker;

#[cfg(test)]
mod contract_tests;

pub use keys::{
    NotificationAudienceCursor, NotificationKeyError, NotificationSourceSlug,
    NotificationTargetKind, NotificationTargetRoute, NotificationTemplateKey, NotificationTypeKey,
};
pub use model::*;
pub use provider::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest, NotificationProviderError,
    NotificationProviderResult, NotificationSourceProvider, NotificationSourceProviderFactory,
    NotificationSourceRegistry, NotificationSourceRegistryEntry, ResolveNotificationAudienceRequest,
    ensure_notification_source_registry, notification_source_registry_from_extensions,
};
pub use provider::*;
pub use reconciliation::*;

pub mod api {
    pub use crate::keys::*;
    pub use crate::model::*;
    pub use crate::provider::*;
    pub use crate::reconciliation::*;
    pub use rustok_api::{PortContext, PortError};
}

#[cfg(feature = "server")]
pub use candidate::{
    NotificationCandidateProcessResult, NotificationCandidateService, NotificationRecipientPolicy,
    NotificationRecipientPolicyDecision, NotificationRecipientPolicyError,
    NotificationRecipientPolicyRequest, NotificationRecipientSuppression,
    NotificationTenantCapabilityCommitDecision, NotificationTenantCapabilityCommitError,
    NotificationTenantCapabilityCommitGuard, NotificationTenantCapabilityCommitRequest,
};
#[cfg(feature = "server")]
pub use error::{NotificationError, NotificationResult};
#[cfg(feature = "server")]
pub use fanout::{
    NotificationFanoutPageResult, NotificationFanoutService, NotificationSourceInboxReceipt,
};
#[cfg(feature = "server")]
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
#[cfg(feature = "server")]
pub use inbox::{
    DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE, MAX_NOTIFICATION_INBOX_CURSOR_BYTES,
    MAX_NOTIFICATION_INBOX_PAGE_SIZE, NotificationInboxItem, NotificationInboxListRequest,
    NotificationInboxListService, NotificationInboxOpenDecision, NotificationInboxOpenRequest,
    NotificationInboxOpenService, NotificationInboxPage,
};
#[cfg(feature = "server")]
pub use inbox_bulk::{
    NotificationInboxMarkAllArchivePage, NotificationInboxMarkAllArchiveRequest,
    NotificationInboxMarkAllArchiveService, NotificationInboxMarkAllReadPage,
    NotificationInboxMarkAllReadRequest, NotificationInboxMarkAllReadService,
    NotificationInboxMarkAllUnreadPage, NotificationInboxMarkAllUnreadRequest,
    NotificationInboxMarkAllUnreadService,
};
#[cfg(feature = "server")]
pub use inbox_count::{
    NotificationInboxUnreadCount, NotificationInboxUnreadCountRequest,
    NotificationInboxUnreadCountService,
};
#[cfg(feature = "server")]
pub use inbox_group::{
    MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES, NotificationInboxGroupListRequest,
    NotificationInboxGroupListService,
};
#[cfg(feature = "server")]
pub use inbox_group_state::{
    NotificationInboxGroupStateAction, NotificationInboxGroupStatePage,
    NotificationInboxGroupStateRequest, NotificationInboxGroupStateService,
};
#[cfg(feature = "server")]
pub use inbox_group_summary::{
    NotificationInboxGroupSummary, NotificationInboxGroupSummaryPage,
    NotificationInboxGroupSummaryRequest, NotificationInboxGroupSummaryService,
};
#[cfg(feature = "server")]
pub use inbox_reconcile::{
    NotificationInboxReconcileInspectionPage, NotificationInboxReconcilePage,
    NotificationInboxReconcileRequest, NotificationInboxReconcileService,
};
#[cfg(feature = "server")]
pub use inbox_selected::{
    MAX_NOTIFICATION_INBOX_SELECTED_IDS, NotificationInboxSelectedAction,
    NotificationInboxSelectedStateRequest, NotificationInboxSelectedStateResult,
    NotificationInboxSelectedStateService,
};
#[cfg(feature = "server")]
pub use inbox_state::{
    NotificationInboxStateDecision, NotificationInboxStateRequest, NotificationInboxStateService,
    NotificationInboxStateSnapshot,
};
#[cfg(feature = "server")]
pub use inbox_storefront_port::{
    NotificationInboxStorefrontGroupItemsRequest, NotificationInboxStorefrontGroupStateRequest,
    NotificationInboxStorefrontGroupSummaryRequest, NotificationInboxStorefrontOpenDecision,
    NotificationInboxStorefrontOpenRequest, NotificationInboxStorefrontPort,
    NotificationInboxStorefrontService, in_process_notification_inbox_storefront_port,
};
#[cfg(feature = "server")]
pub use outbox_intake::{
    DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE, MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE,
    NotificationOutboxEnvelopeDecoder, NotificationOutboxEnvelopeRecord,
    NotificationOutboxIntakeBatchResult, NotificationOutboxIntakeFailure,
    NotificationOutboxIntakeOutcome, NotificationOutboxIntakeRejection,
    NotificationOutboxIntakeResult, NotificationOutboxIntakeWorker,
};
#[cfg(feature = "server")]
pub use recipient_policy::{
    NotificationBlockReadPort, NotificationBlockReadRuntime, NotificationMuteReadPort,
    NotificationMuteReadRuntime, NotificationRecipientPolicyRuntime,
    NotificationRelationPolicyRequest,
};
#[cfg(feature = "server")]
pub use service::NotificationsService;
#[cfg(feature = "server")]
pub use worker::{
    DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE, MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE,
    NotificationCandidateBatchResult, NotificationCandidatePolicyDeferral,
    NotificationCandidateWorkItem, NotificationCandidateWorker, NotificationCandidateWorkerFailure,
};
#[cfg(feature = "server")]
pub use module::NotificationsModule;
