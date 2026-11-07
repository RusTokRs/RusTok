mod audience_policy;
mod audience_storage;
mod attachment_hold_reconciliation;
mod attachment_relation;
pub mod topic_routes;
mod category;
pub mod category_audience;
mod category_audience_read;
mod category_audience_visibility;
mod category_command;
mod category_lifecycle;
mod category_moderation_audience;
mod category_owner;
mod category_policy;
mod category_reply_create_audience;
mod category_route;
mod category_search_audience_scope;
mod category_search_scope;
mod category_topic_create_audience;
mod category_visibility;
mod content_limits;
mod counter_reconciliation;
mod merge_reconciliation_executor;
mod reconciliation_executor;
mod edit_window;
mod engagement_mode;
pub mod event;
mod solution_reconciliation;
mod import_write;
mod mention_relation;
pub mod mention_reconciliation;
#[cfg(test)]
mod mention_relation_tests;
mod moderation_owner;
pub mod moderation_report;
pub mod moderation {
    pub use super::moderation_owner::ModerationService;
}
mod moderation_audience_authorization;
mod pending_visibility;
mod posting_rate;
pub(crate) mod projection_invalidation;
mod public_discovery;
mod quote_command;
mod rbac;
mod read_model_owner;
pub mod read_model {
    pub use super::read_model_owner::ForumReadModelService;
}
pub mod read_tracking;
mod relation_quote_input;
mod relation_read;
mod reply;
mod reply_audience_read;
mod reply_create_audience_authorization;
mod reply_facade;
mod reply_owner;
pub mod revision;
mod search_result_eligibility;
pub mod storefront_read_state;
pub mod subscription;
pub(crate) mod topic;
mod topic_audience;
mod topic_audience_list;
mod topic_audience_lock;
mod topic_audience_read;
mod topic_audience_visibility;
mod topic_canonical_resolution;
mod topic_create_audience_authorization;
mod topic_route;
mod topic_route_backfill;
mod topic_write_audience;
mod topic_facade;
mod topic_fork;
mod topic_merge;
mod topic_merge_audience_reconciliation;
mod topic_merge_follow_up_reconciliation;
mod topic_merge_read_state_reconciliation;
mod topic_merge_subscription_reconciliation;
mod topic_merge_tag_reconciliation;
mod topic_merge_vote_reconciliation;
mod topic_move;
mod topic_owner;
mod topic_read_state_lock;
mod topic_reply_create_audience;
mod topic_reply_range_move;
mod topic_solution_lock;
mod topic_split;
mod topic_subscription_lock;
mod topic_tag_lock;
pub mod topic_visibility;
mod topic_vote_lock;
pub mod ugc_translation_apply;
pub mod user_stats;
mod user_trust;
mod user_trust_audience_facts;
pub mod vote;
pub mod widget_contract;
mod widget_preview;

pub use attachment_hold_reconciliation::{
    DEFAULT_FORUM_ATTACHMENT_HOLD_RECONCILIATION_LIMIT, ForumAttachmentHoldDrift,
    ForumAttachmentHoldDriftKind, ForumAttachmentHoldReconciliationReport,
    ForumAttachmentHoldReconciliationService, MAX_FORUM_ATTACHMENT_HOLD_RECONCILIATION_LIMIT,
};
pub use attachment_relation::ForumAttachmentRelationService;
pub use category_audience::{
    ForumCategoryAudiencePolicy, ForumCategoryAudiencePolicyLayer,
    ForumCategoryAudiencePolicyOwnerService as ForumCategoryAudiencePolicyService,
    SetForumCategoryAudiencePolicyInput,
};
pub use category_audience_read::{ForumCategoryAudiencePage, ForumCategoryAudienceReadService};
pub use category_audience_visibility::{
    ForumCategoryAudienceViewer, ForumCategoryAudienceVisibilityService,
};
pub use category_moderation_audience::{
    ForumCategoryModerationAudiencePolicy, ForumCategoryModerationAudiencePolicyLayer,
    ForumCategoryModerationAudiencePolicyService, SetForumCategoryModerationAudiencePolicyInput,
};
pub use category_owner::CategoryService;
pub use category_reply_create_audience::{
    ForumCategoryReplyCreateAudiencePolicy, ForumCategoryReplyCreateAudiencePolicyLayer,
    ForumCategoryReplyCreateAudiencePolicyService, SetForumCategoryReplyCreateAudiencePolicyInput,
};
pub use category_route::{
    ForumCategoryRouteDescriptor, ForumCategoryRouteDisposition, ForumCategoryRouteResolution,
    ForumCategoryRouteService, MAX_FORUM_CATEGORY_ROUTE_CANDIDATES,
    MAX_FORUM_CATEGORY_ROUTE_LOCALE_LEN, MAX_FORUM_CATEGORY_ROUTE_SLUG_LEN,
};
pub use category_search_audience_scope::ForumSearchCategoryAudienceScopeService;
pub use category_search_scope::{
    ForumSearchCategoryScope, ForumSearchCategoryScopeService, MAX_FORUM_SEARCH_CATEGORY_ROOTS,
};
pub use category_topic_create_audience::{
    ForumCategoryTopicCreateAudiencePolicy, ForumCategoryTopicCreateAudiencePolicyLayer,
    ForumCategoryTopicCreateAudiencePolicyService, SetForumCategoryTopicCreateAudiencePolicyInput,
};
pub use category_visibility::{
    ForumCategoryVisibilityPolicy, ForumCategoryVisibilityPolicyService,
    SetForumCategoryVisibilityPolicyInput,
};
pub use counter_reconciliation::{
    DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT, ForumCounterDrift, ForumCounterDriftKind,
    ForumCounterReconciliationReport, ForumCounterReconciliationService,
    MAX_FORUM_COUNTER_RECONCILIATION_LIMIT,
};
pub use engagement_mode::{
    FORUM_MODULE_SLUG, FORUM_REACTIONS_MODULE_SLUG, ForumEngagementMode, ForumSettingsProviders,
};
pub use event::ForumEventService;
pub use import_write::{
    ForumImportWriteResult, ForumImportWriteService, MAX_FORUM_IMPORT_APPLY_RECORDS_PER_BATCH,
};
pub use mention_reconciliation::{
    ForumMentionDrift, ForumMentionDriftKind, ForumMentionReconciliationReport,
    ForumMentionReconciliationService,
};
pub use moderation::ModerationService;
pub use moderation_audience_authorization::{
    ForumModerationAudienceAuthorization, ForumModerationAudienceAuthorizationService,
};
pub use public_discovery::ForumPublicDiscoveryService;
pub use quote_command::ForumQuoteCommandService;
pub use read_model::ForumReadModelService;
pub use read_tracking::{
    ForumTopicReadState, ForumTopicReadStateService, ForumVisibilityScopedReadStateService,
    MarkForumTopicReadInput, MarkForumTopicsReadBatchInput, MarkForumTopicsReadBatchResult,
};
pub use relation_read::ForumRelationReadService;
pub use reply_audience_read::ForumReplyAudienceReadService;
pub use reply_create_audience_authorization::{
    ForumReplyCreateAudienceAuthorization, ForumReplyCreateAudienceAuthorizationService,
};
pub use reply_facade::ReplyService;
pub use revision::RevisionService;
pub use search_result_eligibility::{
    ForumSearchResultCandidate, ForumSearchResultCandidateKind,
    ForumSearchResultEligibilityService, MAX_FORUM_SEARCH_RESULT_ELIGIBILITY_CANDIDATES,
};
pub use solution_reconciliation::{
    ForumSolutionDrift, ForumSolutionDriftKind, ForumSolutionReconciliationReport,
    ForumSolutionReconciliationService,
};
pub use storefront_read_state::{
    ForumStorefrontReadStateService, ForumStorefrontUnreadTopic, ForumStorefrontUnreadTopicPage,
    ForumTopicUnreadSummary,
};
pub use subscription::SubscriptionService;
pub use subscription::reconciliation::{
    ForumSubscriptionCursor, ForumSubscriptionDrift, ForumSubscriptionDriftKind,
    ForumSubscriptionReconciliationCursors, ForumSubscriptionReconciliationReport,
    ForumSubscriptionReconciliationService, ForumSubscriptionTargetKind,
};
pub use topic::MAX_FORUM_TOPIC_TAGS;
pub use topic_audience::{
    ForumTopicAudiencePolicy,
    ForumTopicAudiencePolicyOwnerService as ForumTopicAudiencePolicyService,
    SetForumTopicAudiencePolicyInput,
};
pub use topic_audience_list::{ForumTopicAudienceListService, ForumTopicAudiencePage};
pub use topic_audience_read::ForumTopicAudienceReadService;
pub use topic_audience_visibility::{
    ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService,
};
pub use topic_canonical_resolution::{
    ForumTopicCanonicalResolution, MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS,
};
pub use topic_create_audience_authorization::{
    ForumTopicCreateAudienceAuthorization, ForumTopicCreateAudienceAuthorizationService,
};
pub use topic_facade::TopicService;
pub use topic_fork::{
    ForkForumReplyBranchInput, ForumTopicForkResult, ForumTopicForkService,
    MAX_FORUM_TOPIC_FORK_BODY_ROWS, MAX_FORUM_TOPIC_FORK_MENTIONS, MAX_FORUM_TOPIC_FORK_QUOTES,
    MAX_FORUM_TOPIC_FORK_REASON_LEN, MAX_FORUM_TOPIC_FORK_RELATION_REVISIONS,
    MAX_FORUM_TOPIC_FORK_REPLIES, MAX_FORUM_TOPIC_FORK_REPLY_REVISIONS,
    MAX_FORUM_TOPIC_FORK_TITLE_LEN,
};
pub use topic_merge::{
    ForumTopicMergeResult, ForumTopicMergeService, MAX_FORUM_TOPIC_MERGE_REASON_LEN,
    MAX_FORUM_TOPIC_MERGE_REPLIES, MergeForumTopicInput,
};
pub use topic_merge_audience_reconciliation::{
    ForumTopicMergeAudienceReconciliationResult, ForumTopicMergeAudienceReconciliationService,
    MAX_FORUM_TOPIC_MERGE_AUDIENCE_REASON_LEN, ReconcileForumTopicMergeAudienceInput,
};
pub use topic_merge_read_state_reconciliation::{
    ForumTopicMergeReadStateReconciliationResult, ForumTopicMergeReadStateReconciliationService,
    MAX_FORUM_TOPIC_MERGE_READ_STATE_REASON_LEN, MAX_FORUM_TOPIC_MERGE_READ_STATES,
    ReconcileForumTopicMergeReadStatesInput,
};
pub use topic_merge_subscription_reconciliation::{
    ForumTopicMergeSubscriptionReconciliationResult,
    ForumTopicMergeSubscriptionReconciliationService,
    MAX_FORUM_TOPIC_MERGE_SUBSCRIPTION_REASON_LEN, MAX_FORUM_TOPIC_MERGE_SUBSCRIPTIONS,
    ReconcileForumTopicMergeSubscriptionsInput,
};
pub use topic_merge_tag_reconciliation::{
    ForumTopicMergeTagReconciliationResult, ForumTopicMergeTagReconciliationService,
    MAX_FORUM_TOPIC_MERGE_TAG_REASON_LEN, MAX_FORUM_TOPIC_MERGE_TAGS,
    ReconcileForumTopicMergeTagsInput,
};
pub use topic_merge_vote_reconciliation::{
    ForumTopicMergeVoteReconciliationResult, ForumTopicMergeVoteReconciliationService,
    MAX_FORUM_TOPIC_MERGE_VOTE_REASON_LEN, MAX_FORUM_TOPIC_MERGE_VOTES,
    ReconcileForumTopicMergeVotesInput,
};
pub use topic_move::{
    ForumTopicMoveResult, ForumTopicMoveService, MAX_FORUM_TOPIC_MOVE_REASON_LEN,
    MoveForumTopicInput,
};
// The owner implementation stays internal (`mod topic_owner`); the service is exposed through a
// named seam, the same shape `moderation` and `read_model` use above. Re-exporting it straight from
// the owner module made an internal owner path part of the public surface.
pub mod route_tombstone_visibility {
    pub use super::topic_owner::route_tombstone_visibility::ForumTopicRouteTombstoneVisibilityService;
}
pub use route_tombstone_visibility::ForumTopicRouteTombstoneVisibilityService;
pub use topic_reply_create_audience::{
    ForumTopicReplyCreateAudiencePolicy, ForumTopicReplyCreateAudiencePolicyService,
    SetForumTopicReplyCreateAudiencePolicyInput,
};
pub use topic_reply_range_move::{
    ForumReplyRangeMoveResult, ForumReplyRangeMoveService, MAX_FORUM_REPLY_RANGE_MOVE_REASON_LEN,
    MAX_FORUM_REPLY_RANGE_MOVE_REPLIES, MoveForumReplyRangeInput,
};
pub use topic_route::{
    FORUM_TOPIC_RENAMED_ROUTE_REASON, FORUM_TOPIC_ROUTE_SHORT_ID_LEN, ForumTopicRouteDescriptor,
    ForumTopicRouteDisposition, ForumTopicRouteResolution, ForumTopicRouteService,
    ForumTopicSlugRenameResult, MAX_FORUM_TOPIC_ROUTE_ALIAS_REASON_LEN,
    MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN, MAX_FORUM_TOPIC_ROUTE_SLUG_LEN, RenameForumTopicSlugInput,
};
pub use topic_route_backfill::{
    BackfillForumTopicMergeRouteAliasesInput, ForumTopicMergeRouteBackfillCursor,
    ForumTopicMergeRouteBackfillResult, ForumTopicMergeRouteBackfillService,
    MAX_FORUM_TOPIC_MERGE_ROUTE_BACKFILL_OPERATIONS,
};
pub use topic_split::{
    ForumTopicSplitResult, ForumTopicSplitService, MAX_FORUM_TOPIC_SPLIT_REASON_LEN,
    MAX_FORUM_TOPIC_SPLIT_REPLIES, MAX_FORUM_TOPIC_SPLIT_TITLE_LEN, SplitForumTopicRepliesInput,
};
pub use topic_visibility::{
    ForumTopicVisibilityScope, ForumTopicVisibilityService, MAX_FORUM_TOPIC_VISIBILITY_CANDIDATES,
};
pub use ugc_translation_apply::{
    ApplyExactForumReplyTranslationInput, ApplyExactForumTopicTranslationInput,
    ForumUgcTranslationApplyError, ForumUgcTranslationApplyResult,
};
pub use user_stats::UserStatsService;
pub use user_trust::{
    ForumUserTrustChange, ForumUserTrustRevision, ForumUserTrustRevisionPage,
    ForumUserTrustService, ForumUserTrustState, MAX_FORUM_USER_TRUST_HISTORY_PAGE,
    MAX_FORUM_USER_TRUST_LEVEL, SetForumUserTrustInput,
};
pub use user_trust_audience_facts::ForumUserTrustAudienceFactsPort;
pub use vote::VoteService;
pub use widget_contract::ForumWidgetContractService;
pub use widget_preview::ForumWidgetPreviewService;
