#![allow(dead_code)]

mod bounded_compat;
mod category {
    include!("category.rs");
    include!("category_visibility_list.rs");
}
mod category_audience;
mod category_audience_read {
    include!("category_audience_read.rs");
    include!("category_audience_read_inline.rs");
}
mod category_audience_visibility;
#[allow(clippy::collapsible_if)]
mod category_command;
mod category_lifecycle;
mod category_moderation_audience;
mod category_owner;
mod category_policy;
mod category_reply_create_audience;
mod category_topic_create_audience;
mod category_tree {
    include!("category_tree.rs");
    include!("category_tree_visibility.rs");
}
mod category_visibility;
pub mod event;
#[allow(clippy::collapsible_if, clippy::too_many_arguments)]
mod mention_relation;
#[cfg(test)]
mod mention_relation_tests {
    include!("mention_relation_tests.rs");
    include!("relation_quote_input_tests.rs");
}
pub mod moderation;
mod moderation_audience_authorization;
mod posting_policy;
mod posting_policy_approved_facts;
mod posting_policy_create_window_facts;
mod posting_policy_evaluator;
mod posting_policy_facts;
mod posting_policy_reading_facts;
mod quote_command;
mod rbac;
pub mod read_model;
pub mod read_tracking;
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
    include!("reply_owner.rs");
    include!("reply_owner_inline.rs");
}
pub mod revision;
pub mod storefront_read_state;
pub mod subscription;
#[allow(clippy::collapsible_if)]
mod topic {
    include!("topic.rs");
    include!("topic_inline.rs");
    include!("topic_visibility_list.rs");
}
mod topic_audience {
    include!("topic_audience.rs");
    include!("topic_audience_visibility.rs");
}
mod topic_audience_list;
mod topic_audience_read;
mod topic_create_audience_authorization;
mod topic_facade;
mod topic_owner {
    include!("topic_owner.rs");
    include!("topic_owner_inline.rs");
}
mod topic_reply_create_audience;
pub mod topic_visibility;
pub mod user_stats;
mod user_trust;
mod user_trust_audience_facts;
pub mod vote;
pub mod widget_contract;

pub use category_audience::{
    ForumCategoryAudiencePolicy, ForumCategoryAudiencePolicyLayer,
    ForumCategoryAudiencePolicyService, SetForumCategoryAudiencePolicyInput,
};
pub use category_audience_read::{
    ForumCategoryAudiencePage, ForumCategoryAudienceReadService,
};
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
pub use category_topic_create_audience::{
    ForumCategoryTopicCreateAudiencePolicy, ForumCategoryTopicCreateAudiencePolicyLayer,
    ForumCategoryTopicCreateAudiencePolicyService, SetForumCategoryTopicCreateAudiencePolicyInput,
};
pub use category_visibility::{
    ForumCategoryVisibilityPolicy, ForumCategoryVisibilityPolicyService,
    SetForumCategoryVisibilityPolicyInput,
};
pub use event::ForumEventService;
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
pub use quote_command::ForumQuoteCommandService;
pub use read_model::ForumReadModelService;
pub use read_tracking::{
    ForumTopicReadState, ForumTopicReadStateService, MarkForumTopicReadInput,
    MarkForumTopicsReadBatchInput, MarkForumTopicsReadBatchResult,
};
pub use relation_read::ForumRelationReadService;
pub use reply_audience_read::ForumReplyAudienceReadService;
pub use reply_create_audience_authorization::{
    ForumReplyCreateAudienceAuthorization, ForumReplyCreateAudienceAuthorizationService,
};
pub use reply_facade::ReplyService;
pub use revision::RevisionService;
pub use storefront_read_state::{
    ForumStorefrontReadStateService, ForumStorefrontUnreadTopic, ForumStorefrontUnreadTopicPage,
    ForumTopicUnreadSummary,
};
pub use subscription::SubscriptionService;
pub use topic_audience::{
    ForumTopicAudiencePolicy, ForumTopicAudiencePolicyService, ForumTopicAudienceViewer,
    ForumTopicAudienceVisibilityService, SetForumTopicAudiencePolicyInput,
};
pub use topic_audience_list::{ForumTopicAudienceListService, ForumTopicAudiencePage};
pub use topic_audience_read::ForumTopicAudienceReadService;
pub use topic_create_audience_authorization::{
    ForumTopicCreateAudienceAuthorization, ForumTopicCreateAudienceAuthorizationService,
};
pub use topic_facade::TopicService;
pub use topic_reply_create_audience::{
    ForumTopicReplyCreateAudiencePolicy, ForumTopicReplyCreateAudiencePolicyService,
    SetForumTopicReplyCreateAudiencePolicyInput,
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
