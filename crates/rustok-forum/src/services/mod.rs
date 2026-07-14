mod bounded_compat;
mod category;
mod category_owner;
mod category_tree;
pub mod event;
pub mod moderation;
mod rbac;
pub mod read_model;
/// Raw persistence service retained temporarily for compatibility.
pub mod reply;
mod reply_owner;
pub mod revision;
pub mod subscription;
/// Raw persistence service retained temporarily for compatibility.
pub mod topic;
mod topic_owner;
pub mod user_stats;
pub mod vote;
pub mod widget_contract;

pub use category_owner::CategoryService;
pub use event::ForumEventService;
pub use moderation::ModerationService;
pub use read_model::ForumReadModelService;
pub use reply_owner::ReplyService;
pub use revision::RevisionService;
pub use subscription::SubscriptionService;
pub use topic_owner::TopicService;
pub use user_stats::UserStatsService;
pub use vote::VoteService;
pub use widget_contract::ForumWidgetContractService;
