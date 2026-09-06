mod bounded_compat;
mod category {
    include!("category_import.rs");
    include!("category.rs");
    include!("category_projection_owner.rs");
    include!("category_visibility_list.rs");
}
mod category_audience {
    include!("category_audience.rs");
    include!("category_audience_owner.rs");
}
mod category_audience_read {
    include!("category_audience_read.rs");
    include!("category_audience_read_inline.rs");
    include!("category_audience_read_search.rs");
}
mod category_audience_visibility;
#[allow(clippy::collapsible_if)]
mod category_command {
    include!("category_command_owner.rs");
    include!("category_command.rs");
}
mod category_lifecycle {
    include!("category_lifecycle.rs");
    include!("category_lifecycle_owner.rs");
}
mod category_moderation_audience;
mod category_owner {
    include!("category_owner.rs");
    include!("category_owner_locale_enumeration.rs");
}
mod category_policy;
mod category_reply_create_audience;
mod category_route;
mod category_search_audience_scope;
mod category_search_scope {
    include!("category_search_scope_visible.rs");
    include!("category_search_scope.rs");
}
mod category_topic_create_audience;
mod category_visibility;
mod counter_reconciliation;
pub mod event;
mod solution_reconciliation;
mod import_write {
    include!("import_tombstone_write.rs");
    include!("import_write.rs");
}
#[allow(clippy::collapsible_if, clippy::too_many_arguments)]
mod mention_relation {
    include!("mention_relation_import.rs");
    include!("mention_relation.rs");
}
pub mod mention_reconciliation;
#[cfg(test)]
mod mention_relation_tests {
    include!("mention_relation_tests.rs");
    include!("relation_quote_input_tests.rs");
}
#[path = "moderation.rs"]
mod moderation_legacy;
mod moderation_owner;
mod moderation_public_owner;
pub mod moderation {
    pub use super::moderation_public_owner::ModerationService;
}
mod moderation_audience_authorization;
mod posting_policy;
mod posting_policy_approved_facts;
mod posting_policy_create_window_facts;
mod posting_policy_evaluator;
mod posting_policy_facts;
mod posting_policy_reading_facts;
pub(crate) mod projection_invalidation;
mod public_discovery;
mod quote_command;
mod rbac;
#[path = "read_model.rs"]
mod read_model_legacy;
mod read_model_owner;
pub mod read_model {
    pub use super::read_model_owner::ForumReadModelService;
}
pub mod read_tracking {
    include!("read_tracking.rs");
    include!("read_tracking_audience.rs");
}
mod relation_quote_input;
mod relation_read;
#[allow(clippy::collapsible_if, clippy::items_after_test_module)]
mod reply {
    include!("reply.rs");
    include!("reply_inline.rs");
}
mod reply_audience_read;
mod reply_create_audience_authorization;
mod reply_facade;
mod reply_owner {
    include!("reply_owner_tombstone_import.rs");
    include!("reply_owner_import.rs");
    include!("reply_owner.rs");
    include!("reply_owner_inline.rs");
}
pub mod revision;
mod search_result_eligibility;
pub mod storefront_read_state {
    include!("storefront_read_state.rs");
    include!("storefront_read_state_bulk.rs");
}
pub mod subscription;
#[allow(clippy::collapsible_if)]
mod topic {
    include!("topic_import.rs");
    include!("topic.rs");
    include!("topic_locale_enumeration.rs");
    include!("topic_inline.rs");
    include!("topic_visibility_list.rs");
    include!("topic_widget_preview.rs");
}
mod topic_audience {
    include!("topic_audience.rs");
    include!("topic_audience_owner.rs");
}
mod topic_audience_list;
mod topic_audience_lock;
mod topic_audience_read;
mod topic_audience_visibility;
mod topic_canonical_resolution;
mod topic_create_audience_authorization;
mod topic_route;
mod topic_route_backfill;
mod topic_facade {
    include!("topic_facade.rs");
    include!("topic_facade_locale_enumeration.rs");
}
mod topic_fork;
mod topic_merge;
mod topic_merge_audience_reconciliation;
mod topic_merge_read_state_reconciliation;
mod topic_merge_subscription_reconciliation;
mod topic_merge_tag_reconciliation;
mod topic_merge_vote_reconciliation;
mod topic_move;
mod topic_owner {
    include!("topic_owner.rs");
    include!("topic_owner_inline.rs");
}
mod topic_read_state_lock;
mod topic_reply_create_audience;
mod topic_reply_range_move;
mod topic_solution_lock;
mod topic_split;
mod topic_subscription_lock;
mod topic_tag_lock;
pub mod topic_visibility;
mod topic_vote_lock;
pub mod user_stats;
mod user_trust;
mod user_trust_audience_facts;
pub mod ugc_translation_apply;
pub mod vote;
pub mod widget_contract;
mod widget_preview;

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
pub use event::ForumEventService;
pub use import_write::{
    ForumImportWriteResult, ForumImportWriteService, MAX_FORUM_IMPORT_APPLY_RECORDS_PER_BATCH,
};
#[allow(unused_imports)]
pub(crate) use mention_relation::MentionRelationService;
pub use moderation::ModerationService;
pub use moderation_audience_authorization::{
    ForumModerationAudienceAuthorization, ForumModerationAudienceAuthorizationService,
};
pub use posting_policy::{
    ForumPostingAction, ForumPostingCandidateMetrics, ForumPostingPolicyDecision,
    ForumPostingPolicyDecisionReason, ForumPostingPolicyEvaluationInput,
    ForumPostingPolicyEvidence, ForumPostingPolicyFactKind, ForumPostingPolicyFacts,
    ForumPostingPolicyMeasureUnit, ForumPostingPolicyOutcome, ForumPostingPolicyUnavailableFact,
    ForumPostingWindowCount, MAX_FORUM_POSTING_POLICY_FACTS,
    MAX_FORUM_POSTING_UNAVAILABLE_REASON_CODE_LENGTH,
};
pub use posting_policy_approved_facts::ForumApprovedPostsFactPort;
pub use posting_policy_create_window_facts::{
    ForumReplyCreatesWindowFactPort, ForumTopicCreatesWindowFactPort,
};
pub use posting_policy_evaluator::{
    FORUM_POSTING_POLICY_PRECEDENCE, ForumPostingPolicyEvaluator, ForumPostingPolicyRules,
    ForumPostingWindowLimit,
};
pub use posting_policy_facts::{
    FORUM_POSTING_POLICY_FACTS_CAPABILITY, FORUM_POSTING_POLICY_FACTS_CAPABILITY_UNAVAILABLE,
    ForumPostingPolicyCompositionRequest, ForumPostingPolicyFactsComposer,
    ForumPostingPolicyOwnerFactPort, ForumPostingPolicyOwnerFactRequest,
    ForumPostingPolicyOwnerFactResponse, ForumPostingPolicyOwnerFactValue,
    ForumPostingTrustFactPort, SharedForumPostingPolicyOwnerFactPort,
};
pub use posting_policy_reading_facts::ForumTopicReadPostingFactPort;
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
    ForumSubscriptionReconciliationReport, ForumSubscriptionReconciliationService,
    ForumSubscriptionTargetKind,
};
pub use mention_reconciliation::{
    ForumMentionDrift, ForumMentionDriftKind, ForumMentionReconciliationReport,
    ForumMentionReconciliationService,
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
pub use topic_owner::route_tombstone_visibility::ForumTopicRouteTombstoneVisibilityService;
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
pub use ugc_translation_apply::{
    ApplyExactForumReplyTranslationInput, ApplyExactForumTopicTranslationInput,
    ForumUgcTranslationApplyError, ForumUgcTranslationApplyResult,
};
