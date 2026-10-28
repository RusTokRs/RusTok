//! # RusToK Forum
//!
//! Forum domain module for the RusToK platform.
//!
//! Provides categories, topics, replies, moderation workflows, user trust tiers,
//! audience policies, and rich-text discussion state.

pub mod attachment_relation;
pub mod audience;
pub mod category_read_transport;
pub mod constants;
pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod export_mapping;
pub mod graphql;
pub mod import_inspection;
pub mod import_mapping;
pub mod import_relation_preparation;
pub mod import_resolution;
pub mod import_tombstone_preparation;
pub mod import_write_preparation;
pub mod mentions {
    include!("mentions_import.rs");
    include!("mentions.rs");
}
pub mod migrations;
pub mod moderation_report;
mod moderation_subject;
mod moderation_transport;
pub mod module;
pub mod notification_recipient;
mod notification_source;
pub mod openapi;
mod reaction_subject;
pub mod reply_create_transport;
pub mod reply_read_transport;
pub mod richtext;
mod search_projection;
mod search_projection_author;
mod seo_audience_targets;
mod seo_targets;
pub mod services;
pub mod state_machine;
pub mod subscription;
pub mod topic_create_transport;
pub mod topic_read_transport;
pub mod visibility;

pub use attachment_relation::{
    ForumAttachmentRelationAdmissionError, ForumAttachmentRelationAdmissionRequest,
    ForumAttachmentRelationInput, ForumAttachmentRelationPreparer, ForumAttachmentRelationRecord,
    ForumAttachmentRelationRevision, ForumAttachmentRelationRevisionError,
    ForumAttachmentRelationSet, ForumAttachmentRelationWriteKind, ForumAttachmentSourceRevision,
    ForumAttachmentUsage, ForumPreparedAttachmentRelation, ForumPreparedAttachmentRelationBatch,
    MAX_FORUM_ATTACHMENT_CAPTION_BYTES, MAX_FORUM_ATTACHMENTS_PER_SET,
};
pub use audience::{
    FORUM_AUDIENCE_FACTS_CAPABILITY, FORUM_AUDIENCE_FACTS_CAPABILITY_UNAVAILABLE,
    ForumAudienceConstraints, ForumAudienceDecision, ForumAudienceDecisionReason,
    ForumAudienceEvaluator, ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    ForumAudienceFactsResolver, MAX_FORUM_AUDIENCE_CHANNELS, MAX_FORUM_AUDIENCE_EXPLICIT_USERS,
    MAX_FORUM_AUDIENCE_GROUPS, MAX_FORUM_AUDIENCE_ROLES, MAX_FORUM_AUDIENCE_TRUST_LEVEL,
    SharedForumAudienceFactsPort,
};
pub use category_read_transport::{
    ForumCategoryReadOperation, ForumCategoryReadTransport, category_read_audience_port_context,
};
pub use constants::{
    FORUM_MODULE_SLUG, KIND_CATEGORY, KIND_REPLY, KIND_TOPIC, reply_status, topic_status,
};
pub use dto::{
    CategoryBreadcrumb, CategoryCursorPage, CategoryCursorQuery, CategoryListItem,
    CategoryPlacementResponse, CategoryReadModel, CategoryResponse,
    CategorySubtreeLifecycleResponse, CategoryTopicPolicyResponse, CategoryTreeNode,
    CategoryTreeQuery, CategoryTreeResponse, CreateCategoryInput, CreateReplyCommandInput,
    CreateReplyInput, CreateTopicCommandInput, CreateTopicInput, DEFAULT_FORUM_READ_LIMIT,
    ForumDomainEventQuery, ForumDomainEventResponse, ForumModuleSettings,
    ForumProjectionOwnerRevisionImpact, ForumProjectionOwnerRevisionResponse,
    ForumProjectionOwnerTenantHeadResponse, ForumQuoteReferenceInput, ForumQuoteTargetKindInput,
    ForumRelationQuoteResponse, ForumRelationSnapshotQuery, ForumRelationSnapshotResponse,
    ForumReplyStreamWidgetPreview, ForumSubscriptionPolicyResponse, ForumSubscriptionResponse,
    ForumSubscriptionTargetType, ForumTopicDetailWidgetPreview, ForumTopicListOrder,
    ForumTopicListWidgetPreview, ForumUserStatsResponse, ForumWidgetCapabilityRequirements,
    ForumWidgetCatalogItem, ForumWidgetCatalogResponse, ForumWidgetCompatibilityEntry,
    ForumWidgetErrorMapping, ForumWidgetPreviewPayload, ForumWidgetPreviewResponse,
    ForumWidgetPropsValidationResponse, ForumWidgetValidationIssue, ListRepliesFilter,
    ListTopicsFilter, MAX_FORUM_CATEGORY_TREE_DEPTH, MAX_FORUM_CATEGORY_TREE_NODES,
    MAX_FORUM_READ_LIMIT, MoveCategoryInput, MoveCategoryResponse, PreviewForumWidgetInput,
    ReorderCategorySiblingsInput, ReorderCategorySiblingsResponse, ReplyCursor, ReplyCursorPage,
    ReplyCursorQuery, ReplyListItem, ReplyListItemPage, ReplyPage, ReplyReadModel, ReplyResponse,
    ReplyRevisionResponse, SetForumQuotesInput, TopicCursorPage, TopicCursorQuery, TopicListCursor,
    TopicListItem, TopicListItemPage, TopicPage, TopicReadModel, TopicResponse,
    TopicRevisionResponse, TopicUnreadCursorPage, TopicUnreadCursorQuery, TopicUnreadReadModel,
    TopicUnreadSummaryReadModel, UpdateCategoryInput, UpdateCategoryTopicPolicyInput,
    UpdateForumSubscriptionInput, UpdateForumSubscriptionPolicyInput, UpdateReplyCommandInput,
    UpdateReplyInput, UpdateTopicCommandInput, UpdateTopicInput, ValidateForumWidgetPropsInput,
    bounded_forum_read_limit, deserialize_forum_read_limit,
};
pub use entities::{
    ForumAttachmentRelation, ForumAttachmentRelationHead, ForumCategory,
    ForumCategoryAudiencePolicyEntity, ForumCategoryLifecycle,
    ForumCategoryModerationAudiencePolicyEntity, ForumCategoryPolicy,
    ForumCategoryReplyCreateAudiencePolicyEntity, ForumCategoryTopicCreateAudiencePolicyEntity,
    ForumDomainEvent, ForumRelationRevision, ForumReply, ForumReplyRevision, ForumTopic,
    ForumTopicAudiencePolicyEntity, ForumTopicMergeAudienceOutcome,
    ForumTopicMergeAudienceReconciliationEntity, ForumTopicMergeOperationEntity,
    ForumTopicMergeReadStateReconciliationEntity, ForumTopicMergeSolutionResolutionEntity,
    ForumTopicMergeSubscriptionReconciliationEntity, ForumTopicMergeTagReconciliationEntity,
    ForumTopicMergeVoteReconciliationEntity, ForumTopicMoveOperationEntity,
    ForumTopicReadStateEntity, ForumTopicReplyCreateAudiencePolicyEntity, ForumTopicRevision,
    ForumUserTrustChangeKind, ForumUserTrustRevisionEntity, ForumUserTrustStateEntity,
};
pub use entities::{
    forum_category, forum_reply, forum_reply_body, forum_topic, forum_topic_tag,
    forum_topic_translation,
};
pub use error::{ForumError, ForumResult};
pub use export_mapping::{
    FORUM_EXPORT_SCHEMA_V1, ForumExportCategoryRecord, ForumExportFragment,
    ForumExportMappingError, ForumExportOwnerViewBatch, ForumExportReplyRecord,
    ForumExportTopicRecord, ForumExportUserRef, ForumOwnerExportMapper,
    MAX_FORUM_EXPORT_OWNER_VIEWS_PER_FRAGMENT,
};
pub use graphql::{ForumMutation, ForumQuery};
pub use import_inspection::{
    ForumImportDependencyDisposition, ForumImportDependencyIssue, ForumImportDependencyRelation,
    MAX_FORUM_IMPORT_DEPENDENCY_ISSUES_PER_BATCH, NodebbForumImportInspection,
    NodebbForumImportInspector,
};
pub use import_mapping::{
    FORUM_IMPORT_SOURCE_NODEBB, ForumImportCandidateBatch, ForumImportCategoryCandidate,
    ForumImportEntityKind, ForumImportExternalRef, ForumImportMappingError,
    ForumImportPostCandidate, ForumImportPostRole, ForumImportTopicCandidate,
    MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH, NodebbCategoryRecord, NodebbExportBatch,
    NodebbForumImportMapper, NodebbPostRecord, NodebbTopicRecord,
};
pub use import_relation_preparation::{
    ForumImportContentRelationDecision, ForumImportMentionBinding, ForumImportRelationEventMode,
    ForumImportRelationMode, ForumImportRelationPreparationError,
    ForumImportRelationPreparationRequest, ForumImportRelationPreparer,
    ForumPreparedImportContentRelations, ForumPreparedImportMention,
    ForumPreparedImportRelationBatch, MAX_FORUM_IMPORT_RELATION_TARGETS_PER_BATCH,
};
pub use import_resolution::{
    ForumImportApplicationResolutionRequest, ForumImportApplicationResolver,
    ForumImportIdentityBinding, ForumImportResolutionError, ForumImportTargetIdentityKind,
    ForumResolvedImportApplicationBatch, ForumResolvedImportAuthor, ForumResolvedImportCategory,
    ForumResolvedImportReply, ForumResolvedImportTopic,
    MAX_FORUM_IMPORT_RESOLUTION_BINDINGS_PER_BATCH,
};
pub use import_tombstone_preparation::{
    ForumImportReplyTombstoneFact, ForumImportTombstonePreparationError,
    ForumImportTombstonePreparationRequest, ForumImportTombstonePreparer,
    ForumPreparedDeletedReplyTombstone, ForumPreparedImportTombstoneBatch,
    MAX_FORUM_IMPORT_REPLY_TOMBSTONES_PER_BATCH, NodebbForumReplyTombstoneMapper,
    NodebbReplyTombstoneRecord,
};
pub use import_write_preparation::{
    ForumImportCategoryWriteDecision, ForumImportReplyWriteDecision, ForumImportTopicWriteDecision,
    ForumImportWriteEventMode, ForumImportWritePreparationError,
    ForumImportWritePreparationRequest, ForumImportWritePreparer, ForumPreparedImportCategory,
    ForumPreparedImportReply, ForumPreparedImportTopic, ForumPreparedImportWriteBatch,
    MAX_FORUM_IMPORT_WRITE_RECORDS_PER_BATCH,
};
pub use mentions::{
    FORUM_MAX_MENTION_TARGETS_PER_REVISION, FORUM_MAX_QUOTE_REFERENCES_PER_REVISION,
    FORUM_MENTION_PROFILES_CAPABILITY, ForumContentTarget, ForumContentTargetKind,
    ForumMentionAudience, ForumMentionCandidates, ForumMentionDiff, ForumMentionEventCandidate,
    ForumMentionEventTarget, ForumMentionPolicy, ForumMentionRevisionProjection,
    ForumQuoteReference, ForumResolvedMentions, ForumRevisionIdentity, ResolvedForumMention,
    diff_forum_mentions, extract_forum_mention_candidates, resolve_forum_mentions,
    validate_forum_quote_references,
};
pub use moderation_report::{
    FORUM_MODERATION_REPORT_CAPABILITY, ForumModerationReportCommand, ForumModerationReportPort,
    SharedForumModerationReportPort,
};
pub use moderation_subject::{FORUM_MODERATION_MODULE, ForumModerationSubjectAdapterFactory};
pub use module::ForumModule;
pub use notification_recipient::{
    FORUM_NOTIFICATION_RECIPIENT_CONTEXT_CAPABILITY,
    FORUM_NOTIFICATION_RECIPIENT_CONTEXT_CAPABILITY_UNAVAILABLE, ForumNotificationRecipientContext,
    ForumNotificationRecipientContextPort, ForumNotificationRecipientContextRequest,
    ForumNotificationRecipientContextResolver, SharedForumNotificationRecipientContextPort,
};
pub use reaction_subject::{
    FORUM_REACTION_SOURCE, FORUM_REACTION_V1_KEY, FORUM_REPLY_REACTION_KIND,
    FORUM_TOPIC_REACTION_KIND, ForumReactionSubjectProviderFactory,
};
pub use reply_create_transport::{ForumReplyCreateTransport, reply_create_audience_port_context};
pub use reply_read_transport::{
    ForumReplyReadOperation, ForumReplyReadTransport, reply_read_audience_port_context,
};
pub use search_projection::ForumSearchProjectionSourceFactory;
pub use services::moderation_report::ForumModerationReportService;
pub use services::{
    ApplyExactForumReplyTranslationInput, ApplyExactForumTopicTranslationInput, CategoryService,
    DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT, FORUM_POSTING_POLICY_FACTS_CAPABILITY,
    FORUM_POSTING_POLICY_FACTS_CAPABILITY_UNAVAILABLE, FORUM_POSTING_POLICY_PRECEDENCE,
    ForkForumReplyBranchInput, ForumApprovedPostsFactPort, ForumAttachmentHoldDrift,
    ForumAttachmentHoldDriftKind, ForumAttachmentHoldReconciliationReport,
    ForumAttachmentHoldReconciliationService, ForumAttachmentRelationService,
    ForumCategoryAudiencePage, ForumCategoryAudiencePolicy, ForumCategoryAudiencePolicyLayer,
    ForumCategoryAudiencePolicyService, ForumCategoryAudienceReadService,
    ForumCategoryAudienceViewer, ForumCategoryAudienceVisibilityService,
    ForumCategoryModerationAudiencePolicy, ForumCategoryModerationAudiencePolicyLayer,
    ForumCategoryModerationAudiencePolicyService, ForumCategoryReplyCreateAudiencePolicy,
    ForumCategoryReplyCreateAudiencePolicyLayer, ForumCategoryReplyCreateAudiencePolicyService,
    ForumCategoryTopicCreateAudiencePolicy, ForumCategoryTopicCreateAudiencePolicyLayer,
    ForumCategoryTopicCreateAudiencePolicyService, ForumCategoryVisibilityPolicy,
    ForumCategoryVisibilityPolicyService, ForumCounterDrift, ForumCounterDriftKind,
    ForumCounterReconciliationReport, ForumCounterReconciliationService, ForumEventService,
    ForumMentionDrift, ForumMentionDriftKind, ForumMentionReconciliationReport,
    ForumMentionReconciliationService, ForumModerationAudienceAuthorization,
    ForumModerationAudienceAuthorizationService, ForumPostingAction, ForumPostingCandidateMetrics,
    ForumPostingPolicyCompositionRequest, ForumPostingPolicyDecision,
    ForumPostingPolicyDecisionReason, ForumPostingPolicyEvaluationInput,
    ForumPostingPolicyEvaluator, ForumPostingPolicyEvidence, ForumPostingPolicyFactKind,
    ForumPostingPolicyFacts, ForumPostingPolicyFactsComposer, ForumPostingPolicyMeasureUnit,
    ForumPostingPolicyOutcome, ForumPostingPolicyOwnerFactPort, ForumPostingPolicyOwnerFactRequest,
    ForumPostingPolicyOwnerFactResponse, ForumPostingPolicyOwnerFactValue, ForumPostingPolicyRules,
    ForumPostingPolicyUnavailableFact, ForumPostingTrustFactPort, ForumPostingWindowCount,
    ForumPostingWindowLimit, ForumPublicDiscoveryService, ForumQuoteCommandService,
    ForumReadModelService, ForumRelationReadService, ForumReplyAudienceReadService,
    ForumReplyCreateAudienceAuthorization, ForumReplyCreateAudienceAuthorizationService,
    ForumReplyCreatesWindowFactPort, ForumReplyRangeMoveResult, ForumReplyRangeMoveService,
    ForumSearchCategoryAudienceScopeService, ForumSearchResultCandidate,
    ForumSearchResultCandidateKind, ForumSearchResultEligibilityService, ForumSettingsProviders,
    ForumSolutionDrift, ForumSolutionDriftKind, ForumSolutionReconciliationReport,
    ForumSolutionReconciliationService, ForumStorefrontReadStateService,
    ForumStorefrontUnreadTopic, ForumStorefrontUnreadTopicPage, ForumSubscriptionCursor,
    ForumSubscriptionDrift, ForumSubscriptionDriftKind, ForumSubscriptionReconciliationCursors,
    ForumSubscriptionReconciliationReport, ForumSubscriptionReconciliationService,
    ForumSubscriptionTargetKind, ForumTopicAudienceListService, ForumTopicAudiencePage,
    ForumTopicAudiencePolicy, ForumTopicAudiencePolicyService, ForumTopicAudienceReadService,
    ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService, ForumTopicCanonicalResolution,
    ForumTopicCreateAudienceAuthorization, ForumTopicCreateAudienceAuthorizationService,
    ForumTopicCreatesWindowFactPort, ForumTopicForkResult, ForumTopicForkService,
    ForumTopicMergeAudienceReconciliationResult, ForumTopicMergeAudienceReconciliationService,
    ForumTopicMergeReadStateReconciliationResult, ForumTopicMergeReadStateReconciliationService,
    ForumTopicMergeResult, ForumTopicMergeService, ForumTopicMergeSubscriptionReconciliationResult,
    ForumTopicMergeSubscriptionReconciliationService, ForumTopicMergeTagReconciliationResult,
    ForumTopicMergeTagReconciliationService, ForumTopicMergeVoteReconciliationResult,
    ForumTopicMergeVoteReconciliationService, ForumTopicMoveResult, ForumTopicMoveService,
    ForumTopicReadPostingFactPort, ForumTopicReadState, ForumTopicReadStateService,
    ForumTopicReplyCreateAudiencePolicy, ForumTopicReplyCreateAudiencePolicyService,
    ForumTopicSplitResult, ForumTopicSplitService, ForumTopicUnreadSummary,
    ForumTopicVisibilityScope, ForumTopicVisibilityService, ForumUgcTranslationApplyError,
    ForumUgcTranslationApplyResult, ForumUserTrustAudienceFactsPort, ForumUserTrustChange,
    ForumUserTrustRevision, ForumUserTrustRevisionPage, ForumUserTrustService, ForumUserTrustState,
    ForumVisibilityScopedReadStateService, ForumWidgetContractService, ForumWidgetPreviewService,
    MAX_FORUM_COUNTER_RECONCILIATION_LIMIT, MAX_FORUM_POSTING_POLICY_FACTS,
    MAX_FORUM_POSTING_UNAVAILABLE_REASON_CODE_LENGTH, MAX_FORUM_REPLY_RANGE_MOVE_REASON_LEN,
    MAX_FORUM_REPLY_RANGE_MOVE_REPLIES, MAX_FORUM_SEARCH_RESULT_ELIGIBILITY_CANDIDATES,
    MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS, MAX_FORUM_TOPIC_FORK_BODY_ROWS,
    MAX_FORUM_TOPIC_FORK_MENTIONS, MAX_FORUM_TOPIC_FORK_QUOTES, MAX_FORUM_TOPIC_FORK_REASON_LEN,
    MAX_FORUM_TOPIC_FORK_RELATION_REVISIONS, MAX_FORUM_TOPIC_FORK_REPLIES,
    MAX_FORUM_TOPIC_FORK_REPLY_REVISIONS, MAX_FORUM_TOPIC_FORK_TITLE_LEN,
    MAX_FORUM_TOPIC_MERGE_AUDIENCE_REASON_LEN, MAX_FORUM_TOPIC_MERGE_READ_STATE_REASON_LEN,
    MAX_FORUM_TOPIC_MERGE_READ_STATES, MAX_FORUM_TOPIC_MERGE_REASON_LEN,
    MAX_FORUM_TOPIC_MERGE_REPLIES, MAX_FORUM_TOPIC_MERGE_SUBSCRIPTION_REASON_LEN,
    MAX_FORUM_TOPIC_MERGE_SUBSCRIPTIONS, MAX_FORUM_TOPIC_MERGE_TAG_REASON_LEN,
    MAX_FORUM_TOPIC_MERGE_TAGS, MAX_FORUM_TOPIC_MERGE_VOTE_REASON_LEN, MAX_FORUM_TOPIC_MERGE_VOTES,
    MAX_FORUM_TOPIC_MOVE_REASON_LEN, MAX_FORUM_TOPIC_SPLIT_REASON_LEN,
    MAX_FORUM_TOPIC_SPLIT_REPLIES, MAX_FORUM_TOPIC_SPLIT_TITLE_LEN, MAX_FORUM_TOPIC_TAGS,
    MAX_FORUM_TOPIC_VISIBILITY_CANDIDATES, MAX_FORUM_USER_TRUST_HISTORY_PAGE,
    MAX_FORUM_USER_TRUST_LEVEL, MarkForumTopicReadInput, MarkForumTopicsReadBatchInput,
    MarkForumTopicsReadBatchResult, MergeForumTopicInput, ModerationService,
    MoveForumReplyRangeInput, MoveForumTopicInput, ReconcileForumTopicMergeAudienceInput,
    ReconcileForumTopicMergeReadStatesInput, ReconcileForumTopicMergeSubscriptionsInput,
    ReconcileForumTopicMergeTagsInput, ReconcileForumTopicMergeVotesInput, ReplyService,
    RevisionService, SetForumCategoryAudiencePolicyInput,
    SetForumCategoryModerationAudiencePolicyInput, SetForumCategoryReplyCreateAudiencePolicyInput,
    SetForumCategoryTopicCreateAudiencePolicyInput, SetForumCategoryVisibilityPolicyInput,
    SetForumTopicAudiencePolicyInput, SetForumTopicReplyCreateAudiencePolicyInput,
    SetForumUserTrustInput, SharedForumPostingPolicyOwnerFactPort, SplitForumTopicRepliesInput,
    SubscriptionService, TopicService, UserStatsService, VoteService,
};
pub use services::{
    BackfillForumTopicMergeRouteAliasesInput, FORUM_TOPIC_RENAMED_ROUTE_REASON,
    FORUM_TOPIC_ROUTE_SHORT_ID_LEN, ForumCategoryRouteDescriptor, ForumCategoryRouteDisposition,
    ForumCategoryRouteResolution, ForumCategoryRouteService, ForumTopicMergeRouteBackfillCursor,
    ForumTopicMergeRouteBackfillResult, ForumTopicMergeRouteBackfillService,
    ForumTopicRouteDescriptor, ForumTopicRouteDisposition, ForumTopicRouteResolution,
    ForumTopicRouteService, ForumTopicSlugRenameResult,
    MAX_FORUM_TOPIC_MERGE_ROUTE_BACKFILL_OPERATIONS, MAX_FORUM_TOPIC_ROUTE_ALIAS_REASON_LEN,
    MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN, MAX_FORUM_TOPIC_ROUTE_SLUG_LEN, RenameForumTopicSlugInput,
};
pub use state_machine::{ReplyStatus, TopicStatus};
pub use subscription::{ForumDigestMode, ForumSubscriptionLevel, ForumSubscriptionPreferences};
pub use topic_create_transport::{ForumTopicCreateTransport, topic_create_audience_port_context};
pub use topic_read_transport::{
    ForumTopicReadOperation, ForumTopicReadTransport, topic_read_audience_port_context,
};
pub use visibility::ForumCategoryVisibility;

#[cfg(test)]
mod contract_tests;
