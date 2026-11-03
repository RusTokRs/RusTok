/// Module slug the host, the audience registry and Search all key Forum by.
pub const FORUM_MODULE_SLUG: &str = "forum";

/// Node kinds for forum content (backed by rustok-content nodes).
pub const KIND_CATEGORY: &str = "forum_category";
pub const KIND_TOPIC: &str = "forum_topic";
pub const KIND_REPLY: &str = "forum_reply";

/// Topic statuses.
pub mod topic_status {
    pub const OPEN: &str = "open";
    pub const CLOSED: &str = "closed";
    pub const ARCHIVED: &str = "archived";
    pub const PENDING: &str = "pending";
}

/// Maximum reply nesting depth. The create path walks the ancestor chain
/// inside the write transaction and rejects deeper threads, so clients
/// never render unbounded recursion and a cyclic parent chain terminates
/// at this bound instead of looping.
pub const MAX_FORUM_REPLY_NESTING_DEPTH: u32 = 10;

/// Reply statuses.
pub mod reply_status {
    pub const PENDING: &str = "pending";
    pub const APPROVED: &str = "approved";
    pub const REJECTED: &str = "rejected";
    pub const HIDDEN: &str = "hidden";
    pub const FLAGGED: &str = "flagged";
    pub const DELETED: &str = "deleted";
}
