//! SeaORM entities for forum-owned persistence.

pub mod forum_audience_mention;
pub mod forum_category;
pub mod forum_category_audience_channel;
pub mod forum_category_audience_group;
pub mod forum_category_audience_policy;
pub mod forum_category_audience_role;
pub mod forum_category_audience_user;
pub mod forum_category_lifecycle;
pub mod forum_category_moderation_audience_channel;
pub mod forum_category_moderation_audience_group;
pub mod forum_category_moderation_audience_policy;
pub mod forum_category_moderation_audience_role;
pub mod forum_category_moderation_audience_user;
pub mod forum_category_policy;
pub mod forum_category_reply_create_audience_channel;
pub mod forum_category_reply_create_audience_group;
pub mod forum_category_reply_create_audience_policy;
pub mod forum_category_reply_create_audience_role;
pub mod forum_category_reply_create_audience_user;
pub mod forum_category_subscription;
pub mod forum_category_taxonomy_binding;
pub mod forum_category_topic_create_audience_channel;
pub mod forum_category_topic_create_audience_group;
pub mod forum_category_topic_create_audience_policy;
pub mod forum_category_topic_create_audience_role;
pub mod forum_category_topic_create_audience_user;
pub mod forum_category_translation;
pub mod forum_domain_event;
pub mod forum_quote;
pub mod forum_relation_revision;
pub mod forum_reply;
pub mod forum_reply_body;
pub mod forum_reply_revision;
pub mod forum_reply_vote;
pub mod forum_solution;
pub mod forum_subscription_policy;
pub mod forum_topic;
pub mod forum_topic_audience_channel;
pub mod forum_topic_audience_group;
pub mod forum_topic_audience_policy;
pub mod forum_topic_audience_role;
pub mod forum_topic_audience_user;
pub mod forum_topic_channel_access;
pub mod forum_topic_merge_audience_reconciliation;
pub mod forum_topic_merge_operation;
pub mod forum_topic_merge_read_state_reconciliation;
pub mod forum_topic_merge_solution_resolution;
pub mod forum_topic_merge_subscription_reconciliation;
pub mod forum_topic_merge_tag_reconciliation;
pub mod forum_topic_merge_vote_reconciliation;
pub mod forum_topic_move_operation;
pub mod forum_topic_read_state;
pub mod forum_topic_reply_create_audience_channel;
pub mod forum_topic_reply_create_audience_group;
pub mod forum_topic_reply_create_audience_policy;
pub mod forum_topic_reply_create_audience_role;
pub mod forum_topic_reply_create_audience_user;
pub mod forum_topic_revision;
pub mod forum_topic_subscription;
pub mod forum_topic_tag;
pub mod forum_topic_translation;
pub mod forum_topic_vote;
pub mod forum_user_mention;
pub mod forum_user_stat;
pub mod forum_user_trust_revision;
pub mod forum_user_trust_state;

pub use forum_category::Entity as ForumCategory;
pub use forum_category_audience_policy::Entity as ForumCategoryAudiencePolicyEntity;
pub use forum_category_lifecycle::Entity as ForumCategoryLifecycle;
pub use forum_category_moderation_audience_policy::Entity as ForumCategoryModerationAudiencePolicyEntity;
pub use forum_category_policy::Entity as ForumCategoryPolicy;
pub use forum_category_reply_create_audience_policy::Entity as ForumCategoryReplyCreateAudiencePolicyEntity;
pub use forum_category_taxonomy_binding::{
    Entity as ForumCategoryTaxonomyBindingEntity, ForumCategoryTaxonomyBinding,
    ForumCategoryTaxonomyBindingService,
};
pub use forum_category_topic_create_audience_policy::Entity as ForumCategoryTopicCreateAudiencePolicyEntity;
pub use forum_domain_event::Entity as ForumDomainEvent;
pub use forum_relation_revision::Entity as ForumRelationRevision;
pub use forum_reply::Entity as ForumReply;
pub use forum_reply_revision::Entity as ForumReplyRevision;
pub use forum_topic::Entity as ForumTopic;
pub use forum_topic_audience_policy::Entity as ForumTopicAudiencePolicyEntity;
pub use forum_topic_merge_audience_reconciliation::{
    Entity as ForumTopicMergeAudienceReconciliationEntity, ForumTopicMergeAudienceOutcome,
};
pub use forum_topic_merge_operation::Entity as ForumTopicMergeOperationEntity;
pub use forum_topic_merge_read_state_reconciliation::Entity as ForumTopicMergeReadStateReconciliationEntity;
pub use forum_topic_merge_solution_resolution::Entity as ForumTopicMergeSolutionResolutionEntity;
pub use forum_topic_merge_subscription_reconciliation::Entity as ForumTopicMergeSubscriptionReconciliationEntity;
pub use forum_topic_merge_tag_reconciliation::Entity as ForumTopicMergeTagReconciliationEntity;
pub use forum_topic_merge_vote_reconciliation::Entity as ForumTopicMergeVoteReconciliationEntity;
pub use forum_topic_move_operation::Entity as ForumTopicMoveOperationEntity;
pub use forum_topic_read_state::Entity as ForumTopicReadStateEntity;
pub use forum_topic_reply_create_audience_policy::Entity as ForumTopicReplyCreateAudiencePolicyEntity;
pub use forum_topic_revision::Entity as ForumTopicRevision;
pub use forum_user_trust_revision::{
    Entity as ForumUserTrustRevisionEntity, ForumUserTrustChangeKind,
};
pub use forum_user_trust_state::Entity as ForumUserTrustStateEntity;
