use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::search_projection::register_search_projection_source;
use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use rustok_moderation_api::register_moderation_subject_adapter_factory;
use rustok_notifications_api::register_notification_source_provider_factory;
use rustok_reactions_api::register_reaction_subject_provider_factory;
use rustok_seo_targets::register_seo_target_provider;
use sea_orm_migration::MigrationTrait;

pub mod audience;
pub mod category_presentation;
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
pub mod import_write_preparation;
pub mod locale;
pub mod mentions {
    include!("mentions_import.rs");
    include!("mentions.rs");
}
pub mod migrations;
mod moderation_subject;
mod moderation_transport;
pub mod notification_recipient;
mod notification_source;
pub mod openapi;
mod reaction_subject;
mod reply_create_transport;
pub mod reply_read_transport;
pub mod richtext;
mod search_projection;
mod search_projection_author;
mod seo_audience_targets;
mod seo_targets;
pub mod services;
pub mod state_machine;
pub mod subscription;
mod topic_create_transport;
pub mod topic_read_transport;
pub mod visibility;

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
pub use constants::*;
pub use dto::*;
pub use entities::*;
pub use error::{ForumError, ForumResult};
pub use export_mapping::*;
pub use graphql::{ForumMutation, ForumQuery};
pub use import_inspection::*;
pub use import_mapping::*;
pub use import_relation_preparation::*;
pub use import_resolution::*;
pub use import_write_preparation::*;
pub use mentions::*;
pub use moderation_subject::{
    FORUM_MODERATION_MODULE, ForumModerationSubjectAdapterFactory,
};
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
pub use reply_read_transport::{
    ForumReplyReadOperation, ForumReplyReadTransport, reply_read_audience_port_context,
};
pub use search_projection::ForumSearchProjectionSourceFactory;
pub use services::{
    CategoryService, DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT,
    FORUM_POSTING_POLICY_FACTS_CAPABILITY, FORUM_POSTING_POLICY_FACTS_CAPABILITY_UNAVAILABLE,
    FORUM_POSTING_POLICY_PRECEDENCE, ForkForumReplyBranchInput, ForumApprovedPostsFactPort,
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
    ForumModerationAudienceAuthorization, ForumModerationAudienceAuthorizationService,
    ForumPostingAction, ForumPostingCandidateMetrics, ForumPostingPolicyCompositionRequest,
    ForumPostingPolicyDecision, ForumPostingPolicyDecisionReason,
    ForumPostingPolicyEvaluationInput, ForumPostingPolicyEvaluator, ForumPostingPolicyEvidence,
    ForumPostingPolicyFactKind, ForumPostingPolicyFacts, ForumPostingPolicyFactsComposer,
    ForumPostingPolicyMeasureUnit, ForumPostingPolicyOutcome, ForumPostingPolicyOwnerFactPort,
    ForumPostingPolicyOwnerFactRequest, ForumPostingPolicyOwnerFactResponse,
    ForumPostingPolicyOwnerFactValue, ForumPostingPolicyRules, ForumPostingPolicyUnavailableFact,
    ForumPostingTrustFactPort, ForumPostingWindowCount, ForumPostingWindowLimit,
    ForumPublicDiscoveryService, ForumQuoteCommandService, ForumReadModelService,
    ForumRelationReadService, ForumReplyAudienceReadService, ForumReplyCreateAudienceAuthorization,
    ForumReplyCreateAudienceAuthorizationService, ForumReplyCreatesWindowFactPort,
    ForumReplyRangeMoveResult, ForumReplyRangeMoveService, ForumSearchCategoryAudienceScopeService,
    ForumSearchResultCandidate, ForumSearchResultCandidateKind,
    ForumSearchResultEligibilityService, ForumStorefrontReadStateService,
    ForumStorefrontUnreadTopic, ForumStorefrontUnreadTopicPage, ForumTopicAudienceListService,
    ForumTopicAudiencePage, ForumTopicAudiencePolicy, ForumTopicAudiencePolicyService,
    ForumTopicAudienceReadService, ForumTopicAudienceViewer,
    ForumTopicAudienceVisibilityService, ForumTopicCanonicalResolution,
    ForumTopicCreateAudienceAuthorization, ForumTopicCreateAudienceAuthorizationService,
    ForumTopicCreatesWindowFactPort, ForumTopicForkResult, ForumTopicForkService,
    ForumTopicMergeAudienceReconciliationResult, ForumTopicMergeAudienceReconciliationService,
    ForumTopicMergeReadStateReconciliationResult,
    ForumTopicMergeReadStateReconciliationService, ForumTopicMergeResult,
    ForumTopicMergeService, ForumTopicMergeSubscriptionReconciliationResult,
    ForumTopicMergeSubscriptionReconciliationService, ForumTopicMergeTagReconciliationResult,
    ForumTopicMergeTagReconciliationService, ForumTopicMergeVoteReconciliationResult,
    ForumTopicMergeVoteReconciliationService, ForumTopicMoveResult, ForumTopicMoveService,
    ForumTopicReadPostingFactPort, ForumTopicReadState, ForumTopicReadStateService,
    ForumTopicReplyCreateAudiencePolicy, ForumTopicReplyCreateAudiencePolicyService,
    ForumTopicSplitResult, ForumTopicSplitService, ForumTopicUnreadSummary,
    ForumTopicVisibilityScope, ForumTopicVisibilityService, ForumUserTrustAudienceFactsPort,
    ForumUserTrustChange, ForumUserTrustRevision, ForumUserTrustRevisionPage,
    ForumUserTrustService, ForumUserTrustState, ForumVisibilityScopedReadStateService,
    ForumWidgetContractService, ForumWidgetPreviewService, MAX_FORUM_COUNTER_RECONCILIATION_LIMIT,
    MAX_FORUM_POSTING_POLICY_FACTS, MAX_FORUM_POSTING_UNAVAILABLE_REASON_CODE_LENGTH,
    MAX_FORUM_REPLY_RANGE_MOVE_REASON_LEN, MAX_FORUM_REPLY_RANGE_MOVE_REPLIES,
    MAX_FORUM_SEARCH_RESULT_ELIGIBILITY_CANDIDATES,
    MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS, MAX_FORUM_TOPIC_FORK_BODY_ROWS,
    MAX_FORUM_TOPIC_FORK_MENTIONS, MAX_FORUM_TOPIC_FORK_QUOTES,
    MAX_FORUM_TOPIC_FORK_REASON_LEN, MAX_FORUM_TOPIC_FORK_RELATION_REVISIONS,
    MAX_FORUM_TOPIC_FORK_REPLIES, MAX_FORUM_TOPIC_FORK_REPLY_REVISIONS,
    MAX_FORUM_TOPIC_FORK_TITLE_LEN, MAX_FORUM_TOPIC_MERGE_AUDIENCE_REASON_LEN,
    MAX_FORUM_TOPIC_MERGE_READ_STATE_REASON_LEN, MAX_FORUM_TOPIC_MERGE_READ_STATES,
    MAX_FORUM_TOPIC_MERGE_REASON_LEN, MAX_FORUM_TOPIC_MERGE_REPLIES,
    MAX_FORUM_TOPIC_MERGE_SUBSCRIPTION_REASON_LEN, MAX_FORUM_TOPIC_MERGE_SUBSCRIPTIONS,
    MAX_FORUM_TOPIC_MERGE_TAG_REASON_LEN, MAX_FORUM_TOPIC_MERGE_TAGS,
    MAX_FORUM_TOPIC_MERGE_VOTE_REASON_LEN, MAX_FORUM_TOPIC_MERGE_VOTES,
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
    MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN, MAX_FORUM_TOPIC_ROUTE_SLUG_LEN,
    RenameForumTopicSlugInput,
};
pub use state_machine::{ReplyStatus, TopicStatus};
pub use subscription::{ForumDigestMode, ForumSubscriptionLevel, ForumSubscriptionPreferences};
pub use topic_read_transport::{
    ForumTopicReadOperation, ForumTopicReadTransport, topic_read_audience_port_context,
};
pub use visibility::ForumCategoryVisibility;

pub struct ForumModule;

#[async_trait]
impl RusToKModule for ForumModule {
    fn slug(&self) -> &'static str {
        "forum"
    }

    fn name(&self) -> &'static str {
        "Forum"
    }

    fn description(&self) -> &'static str {
        "Forum categories, topics, replies, and moderation workflows"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["content", "taxonomy"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::FORUM_CATEGORIES_CREATE,
            Permission::FORUM_CATEGORIES_READ,
            Permission::FORUM_CATEGORIES_UPDATE,
            Permission::FORUM_CATEGORIES_DELETE,
            Permission::FORUM_CATEGORIES_LIST,
            Permission::FORUM_CATEGORIES_MANAGE,
            Permission::FORUM_TOPICS_CREATE,
            Permission::FORUM_TOPICS_READ,
            Permission::FORUM_TOPICS_UPDATE,
            Permission::FORUM_TOPICS_DELETE,
            Permission::FORUM_TOPICS_LIST,
            Permission::FORUM_TOPICS_MODERATE,
            Permission::FORUM_TOPICS_MANAGE,
            Permission::FORUM_REPLIES_CREATE,
            Permission::FORUM_REPLIES_READ,
            Permission::FORUM_REPLIES_UPDATE,
            Permission::FORUM_REPLIES_DELETE,
            Permission::FORUM_REPLIES_LIST,
            Permission::FORUM_REPLIES_MODERATE,
            Permission::FORUM_REPLIES_MANAGE,
        ]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        register_search_projection_source(extensions, ForumSearchProjectionSourceFactory).map_err(
            |error| {
                rustok_core::Error::Validation(format!(
                    "forum Search projection source registration failed: {error}"
                ))
            },
        )?;
        register_seo_target_provider(
            extensions,
            seo_audience_targets::ForumCategorySeoTargetProvider,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "forum category SEO target registration failed: {error}"
            ))
        })?;
        register_seo_target_provider(
            extensions,
            seo_audience_targets::ForumTopicSeoTargetProvider,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "forum topic SEO target registration failed: {error}"
            ))
        })?;
        register_reaction_subject_provider_factory(
            extensions,
            reaction_subject::ForumReactionSubjectProviderFactory,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "forum reaction subject factory registration failed: {error}"
            ))
        })?;
        register_moderation_subject_adapter_factory(
            extensions,
            moderation_subject::ForumModerationSubjectAdapterFactory::topic(),
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "forum moderation topic adapter factory registration failed: {error}"
            ))
        })?;
        register_moderation_subject_adapter_factory(
            extensions,
            moderation_subject::ForumModerationSubjectAdapterFactory::reply(),
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "forum moderation reply adapter factory registration failed: {error}"
            ))
        })?;
        register_notification_source_provider_factory(
            extensions,
            notification_source::ForumNotificationSourceProviderFactory,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "forum notification source factory registration failed: {error}"
            ))
        })?;
        Ok(())
    }
}

impl MigrationSource for ForumModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod contract_tests;
