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
pub mod module;
mod outbox_intake;
mod recipient_policy;
mod service;
mod worker;

#[cfg(test)]
mod contract_tests;

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

pub use module::NotificationsModule;

