/// Node kinds for forum content (backed by rustok-content nodes).
pub const KIND_CATEGORY: &str = "forum_category";
pub const KIND_TOPIC: &str = "forum_topic";
pub const KIND_REPLY: &str = "forum_reply";

/// Topic statuses.
pub mod topic_status {
    pub const OPEN: &str = "open";
    pub const CLOSED: &str = "closed";
    pub const ARCHIVED: &str = "archived";
}

/// Reply statuses.
pub mod reply_status {
    pub const PENDING: &str = "pending";
    pub const APPROVED: &str = "approved";
    pub const REJECTED: &str = "rejected";
    pub const HIDDEN: &str = "hidden";
    pub const FLAGGED: &str = "flagged";
    pub const DELETED: &str = "deleted";
}
