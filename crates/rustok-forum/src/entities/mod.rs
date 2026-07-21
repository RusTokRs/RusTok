//! SeaORM entities for forum-owned persistence.

pub mod forum_audience_mention;
pub mod forum_category;
pub mod forum_category_lifecycle;
pub mod forum_category_policy;
pub mod forum_category_subscription;
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
pub mod forum_topic_channel_access;
pub mod forum_topic_revision;
pub mod forum_topic_subscription;
pub mod forum_topic_tag;
pub mod forum_topic_translation;
pub mod forum_topic_vote;
pub mod forum_user_mention;
pub mod forum_user_stat;

pub use forum_category::Entity as ForumCategory;
pub use forum_category_lifecycle::Entity as ForumCategoryLifecycle;
pub use forum_category_policy::Entity as ForumCategoryPolicy;
pub use forum_domain_event::Entity as ForumDomainEvent;
pub use forum_relation_revision::Entity as ForumRelationRevision;
pub use forum_reply::Entity as ForumReply;
pub use forum_reply_revision::Entity as ForumReplyRevision;
pub use forum_topic::Entity as ForumTopic;
pub use forum_topic_revision::Entity as ForumTopicRevision;
