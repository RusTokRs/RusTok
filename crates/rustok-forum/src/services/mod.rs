#![allow(dead_code)]

mod bounded_compat;
mod category;
#[allow(clippy::collapsible_if)]
mod category_command;
mod category_lifecycle;
mod category_owner;
mod category_policy;
mod category_tree;
pub mod event;
#[allow(clippy::collapsible_if, clippy::too_many_arguments)]
mod mention_relation;
#[cfg(test)]
mod mention_relation_tests {
    include!("mention_relation_tests.rs");
    include!("relation_quote_input_tests.rs");
}
pub mod moderation;
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
}
mod topic_facade;
mod topic_owner {
    include!("topic_owner.rs");
    include!("topic_owner_inline.rs");
}
pub mod user_stats;
pub mod vote;
pub mod widget_contract;

pub use category_owner::CategoryService;
pub use event::ForumEventService;
#[allow(unused_imports)]
pub(crate) use mention_relation::MentionRelationService;
pub use moderation::ModerationService;
pub use quote_command::ForumQuoteCommandService;
pub use read_model::ForumReadModelService;
pub use read_tracking::{
    ForumTopicReadState, ForumTopicReadStateService, MarkForumTopicReadInput,
    MarkForumTopicsReadBatchInput, MarkForumTopicsReadBatchResult,
};
pub use relation_read::ForumRelationReadService;
pub use reply_facade::ReplyService;
pub use revision::RevisionService;
pub use storefront_read_state::{ForumStorefrontReadStateService, ForumTopicUnreadSummary};
pub use subscription::SubscriptionService;
pub use topic_facade::TopicService;
pub use user_stats::UserStatsService;
pub use vote::VoteService;
pub use widget_contract::ForumWidgetContractService;
