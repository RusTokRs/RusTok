use thiserror::Error;
use uuid::Uuid;

/// Domain error exposed through Forum transports.
///
/// `Display` is intentionally safe for public boundaries because GraphQL and
/// compatibility adapters may convert an error through `to_string()`. Use the
/// `Debug` representation and the wrapped source error for server-side
/// diagnostics; never add database, provider, or internal details to these
/// public messages.
#[derive(Debug, Error)]
pub enum ForumError {
    #[error("Forum persistence operation failed")]
    Database(#[source] sea_orm::DbErr),

    #[error("Forum content operation failed")]
    Content(#[from] rustok_content::ContentError),

    #[error("Forum internal operation failed")]
    Internal(#[from] rustok_core::Error),

    #[error("Category not found: {0}")]
    CategoryNotFound(Uuid),

    #[error("Forum category route was not found")]
    CategoryRouteNotFound,

    #[error("Forum category route resolution is inconsistent")]
    CategoryRouteResolutionConflict,

    #[error("Topic not found: {0}")]
    TopicNotFound(Uuid),

    #[error("Reply not found: {0}")]
    ReplyNotFound(Uuid),

    #[error("Topic solution not found for topic: {0}")]
    SolutionNotFound(Uuid),

    #[error("Topic is closed")]
    TopicClosed,

    #[error("Topic is archived")]
    TopicArchived,

    #[error("Topic is awaiting moderation")]
    TopicAwaitingModeration,

    #[error("Topic is locked")]
    TopicLocked,

    #[error("Topic is deleted")]
    TopicDeleted,

    #[error("Topic cannot be restored")]
    TopicRestoreUnavailable(Uuid),

    #[error("Reply is deleted")]
    ReplyDeleted,

    #[error("Forum internal voting is disabled while reactions are selected")]
    InternalVotingDisabled,

    #[error("Reply cannot be restored")]
    ReplyRestoreUnavailable(Uuid),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Forum posting rate limit reached; retry after {retry_after_seconds} seconds")]
    RateLimited { retry_after_seconds: u64 },

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Forum mention target is unavailable")]
    MentionTargetUnavailable,

    #[error("Forum quote target is unavailable")]
    QuoteTargetUnavailable,

    #[error("Forum relation revision is unavailable")]
    RelationRevisionUnavailable,

    #[error("Forum relation revision changed concurrently")]
    RelationRevisionConflict,

    #[error("Forum attachment relation invariant was violated")]
    AttachmentRelationInvariant,

    #[error("Forum attachment relation revision counter is exhausted")]
    AttachmentRelationRevisionExhausted,

    #[error("Forum subscription revision counter is exhausted")]
    SubscriptionRevisionExhausted,

    #[error("Forum subscription revision changed concurrently")]
    SubscriptionRevisionConflict,

    #[error("Forum attachment source revision conflict: expected {expected}, current {current}")]
    AttachmentSourceRevisionConflict { expected: u64, current: u64 },

    #[error("Forum topic changed concurrently: {0}")]
    TopicUpdateConflict(Uuid),

    #[error("Forum topic move operation conflicts with an existing command: {0}")]
    TopicMoveOperationConflict(Uuid),

    #[error("Forum topic merge operation conflicts with an existing command: {0}")]
    TopicMergeOperationConflict(Uuid),

    #[error("Forum topic fork operation conflicts with an existing command: {0}")]
    TopicForkOperationConflict(Uuid),

    #[error("Forum reply range move operation conflicts with an existing command: {0}")]
    TopicReplyRangeMoveOperationConflict(Uuid),

    #[error("Forum reply range move accepted solutions conflict: {0}")]
    TopicReplyRangeMoveSolutionConflict(Uuid),

    #[error("Forum topic merge accepted solutions require explicit resolution: {0}")]
    TopicMergeSolutionConflict(Uuid),

    #[error("Forum topic canonical resolution is inconsistent: {0}")]
    TopicCanonicalResolutionConflict(Uuid),

    #[error("Forum topic route was not found")]
    TopicRouteNotFound,

    #[error("Forum topic route resolution is inconsistent")]
    TopicRouteResolutionConflict,

    #[error("Forum topic merge audience reconciliation conflicts with an existing command: {0}")]
    TopicMergeAudienceReconciliationConflict(Uuid),

    #[error("Forum topic merge audience layers require explicit resolution")]
    TopicMergeAudiencePolicyConflict(Uuid),

    #[error("Forum topic merge read-state reconciliation conflicts with an existing command: {0}")]
    TopicMergeReadStateReconciliationConflict(Uuid),

    #[error(
        "Forum topic merge subscription reconciliation conflicts with an existing command: {0}"
    )]
    TopicMergeSubscriptionReconciliationConflict(Uuid),

    #[error("Forum topic merge tag reconciliation conflicts with an existing command: {0}")]
    TopicMergeTagReconciliationConflict(Uuid),

    #[error("Forum topic merge vote reconciliation conflicts with an existing command: {0}")]
    TopicMergeVoteReconciliationConflict(Uuid),

    #[error("Required capability `{capability}` is unavailable")]
    CapabilityUnavailable {
        capability: &'static str,
        code: &'static str,
    },

    #[error("Forum capability operation failed")]
    CapabilityFailure {
        capability: &'static str,
        source_code: String,
        message: String,
        retryable: bool,
    },

    #[error("{0}")]
    InvalidTopicTransition(#[from] crate::state_machine::InvalidTopicTransition),

    #[error("{0}")]
    InvalidReplyTransition(#[from] crate::state_machine::InvalidReplyTransition),
}

pub type ForumResult<T> = Result<T, ForumError>;

impl ForumError {
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden(message.into())
    }

    pub const fn mention_target_unavailable() -> Self {
        Self::MentionTargetUnavailable
    }

    pub const fn quote_target_unavailable() -> Self {
        Self::QuoteTargetUnavailable
    }

    pub const fn relation_revision_unavailable() -> Self {
        Self::RelationRevisionUnavailable
    }

    pub const fn capability_unavailable(capability: &'static str, code: &'static str) -> Self {
        Self::CapabilityUnavailable { capability, code }
    }

    pub fn capability_failure(
        capability: &'static str,
        source_code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self::CapabilityFailure {
            capability,
            source_code: source_code.into(),
            message: message.into(),
            retryable,
        }
    }

    pub const fn stable_code(&self) -> &'static str {
        match self {
            Self::CapabilityUnavailable { code, .. } => code,
            Self::CapabilityFailure { .. } => "FORUM_CAPABILITY_FAILURE",
            Self::MentionTargetUnavailable => "FORUM_MENTION_TARGET_UNAVAILABLE",
            Self::QuoteTargetUnavailable => "FORUM_QUOTE_TARGET_UNAVAILABLE",
            Self::RelationRevisionUnavailable => "FORUM_RELATION_REVISION_UNAVAILABLE",
            Self::RelationRevisionConflict => "FORUM_RELATION_REVISION_CONFLICT",
            Self::AttachmentRelationInvariant => "FORUM_ATTACHMENT_RELATION_INVARIANT",
            Self::AttachmentRelationRevisionExhausted => {
                "FORUM_ATTACHMENT_RELATION_REVISION_EXHAUSTED"
            }
            Self::SubscriptionRevisionExhausted => "FORUM_SUBSCRIPTION_REVISION_EXHAUSTED",
            Self::SubscriptionRevisionConflict => "FORUM_SUBSCRIPTION_REVISION_CONFLICT",
            Self::AttachmentSourceRevisionConflict { .. } => {
                "FORUM_ATTACHMENT_SOURCE_REVISION_CONFLICT"
            }
            Self::TopicUpdateConflict(_) => "FORUM_TOPIC_UPDATE_CONFLICT",
            Self::TopicMoveOperationConflict(_) => "FORUM_TOPIC_MOVE_OPERATION_CONFLICT",
            Self::TopicMergeOperationConflict(_) => "FORUM_TOPIC_MERGE_OPERATION_CONFLICT",
            Self::TopicForkOperationConflict(_) => "FORUM_TOPIC_FORK_OPERATION_CONFLICT",
            Self::TopicReplyRangeMoveOperationConflict(_) => {
                "FORUM_TOPIC_REPLY_RANGE_MOVE_OPERATION_CONFLICT"
            }
            Self::TopicReplyRangeMoveSolutionConflict(_) => {
                "FORUM_TOPIC_REPLY_RANGE_MOVE_SOLUTION_CONFLICT"
            }
            Self::TopicMergeSolutionConflict(_) => "FORUM_TOPIC_MERGE_SOLUTION_CONFLICT",
            Self::TopicCanonicalResolutionConflict(_) => {
                "FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT"
            }
            Self::TopicRouteNotFound => "FORUM_TOPIC_ROUTE_NOT_FOUND",
            Self::TopicRouteResolutionConflict => "FORUM_TOPIC_ROUTE_RESOLUTION_CONFLICT",
            Self::CategoryRouteNotFound => "FORUM_CATEGORY_ROUTE_NOT_FOUND",
            Self::CategoryRouteResolutionConflict => "FORUM_CATEGORY_ROUTE_RESOLUTION_CONFLICT",
            Self::TopicMergeAudienceReconciliationConflict(_) => {
                "FORUM_TOPIC_MERGE_AUDIENCE_RECONCILIATION_CONFLICT"
            }
            Self::TopicMergeAudiencePolicyConflict(_) => {
                "FORUM_TOPIC_MERGE_AUDIENCE_POLICY_CONFLICT"
            }
            Self::TopicMergeReadStateReconciliationConflict(_) => {
                "FORUM_TOPIC_MERGE_READ_STATE_RECONCILIATION_CONFLICT"
            }
            Self::TopicMergeSubscriptionReconciliationConflict(_) => {
                "FORUM_TOPIC_MERGE_SUBSCRIPTION_RECONCILIATION_CONFLICT"
            }
            Self::TopicMergeTagReconciliationConflict(_) => {
                "FORUM_TOPIC_MERGE_TAG_RECONCILIATION_CONFLICT"
            }
            Self::TopicMergeVoteReconciliationConflict(_) => {
                "FORUM_TOPIC_MERGE_VOTE_RECONCILIATION_CONFLICT"
            }
            Self::CategoryNotFound(_) => "FORUM_CATEGORY_NOT_FOUND",
            Self::TopicNotFound(_) => "FORUM_TOPIC_NOT_FOUND",
            Self::ReplyNotFound(_) => "FORUM_REPLY_NOT_FOUND",
            Self::SolutionNotFound(_) => "FORUM_SOLUTION_NOT_FOUND",
            Self::TopicClosed => "FORUM_TOPIC_CLOSED",
            Self::TopicArchived => "FORUM_TOPIC_ARCHIVED",
            Self::TopicAwaitingModeration => "FORUM_TOPIC_AWAITING_MODERATION",
            Self::TopicLocked => "FORUM_TOPIC_LOCKED",
            Self::TopicDeleted => "FORUM_TOPIC_DELETED",
            Self::TopicRestoreUnavailable(_) => "FORUM_TOPIC_RESTORE_UNAVAILABLE",
            Self::ReplyDeleted => "FORUM_REPLY_DELETED",
            Self::InternalVotingDisabled => "FORUM_INTERNAL_VOTING_DISABLED",
            Self::ReplyRestoreUnavailable(_) => "FORUM_REPLY_RESTORE_UNAVAILABLE",
            Self::Validation(_) => "FORUM_VALIDATION_FAILED",
            Self::RateLimited { .. } => "FORUM_RATE_LIMITED",
            Self::Forbidden(_) => "FORUM_FORBIDDEN",
            Self::Database(_) | Self::Content(_) | Self::Internal(_) => "FORUM_INTERNAL_ERROR",
            Self::InvalidTopicTransition(_) => "FORUM_TOPIC_TRANSITION_INVALID",
            Self::InvalidReplyTransition(_) => "FORUM_REPLY_TRANSITION_INVALID",
        }
    }

    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::CapabilityFailure { retryable, .. } => *retryable,
            Self::Database(_) | Self::Internal(_) | Self::RelationRevisionConflict => true,
            Self::AttachmentRelationInvariant
            | Self::AttachmentRelationRevisionExhausted
            | Self::SubscriptionRevisionExhausted => false,
            Self::SubscriptionRevisionConflict => true,
            Self::AttachmentSourceRevisionConflict { .. } => true,
            Self::TopicUpdateConflict(_) => true,
            _ => false,
        }
    }
}

const TRIGGER_MSG_AUDIENCE_POLICY_CONFLICT: &str = "forum topic merge audience policy conflict";
const TRIGGER_MSG_CATEGORY_TOPIC_CREATE_DENIED: &str =
    "forum category does not allow topic creation";
const TRIGGER_MSG_CATEGORY_LIFECYCLE_ACTIVE_PARENT: &str =
    "active forum category cannot have archived parent";
const TRIGGER_MSG_CATEGORY_LIFECYCLE_ARCHIVED_CHILD: &str =
    "archived forum category cannot have active child";
const TRIGGER_MSG_CATEGORY_LIFECYCLE_GENERIC: &str = "forum category lifecycle";
const TRIGGER_MSG_CATEGORY_ICON: &str = "Forum category icon";
const TRIGGER_MSG_CATEGORY_COLOR: &str = "Forum category color";

const SQLSTATE_RAISE_EXCEPTION: &str = "P0001";
const SQLSTATE_UNIQUE_VIOLATION: &str = "23505";

impl From<sea_orm::DbErr> for ForumError {
    fn from(error: sea_orm::DbErr) -> Self {
        // Prefer SQLSTATE over free-form message: Postgres RAISE EXCEPTION
        // is P0001 and SeaORM surfaces it as `code: Some("P0001")` in the debug
        // representation. TRIGGER_MSG_* remain the second-level discriminator so a
        // trigger wording change is caught in one place.
        let debug = format!("{error:?}");
        let message = error.to_string();
        let is_raise =
            debug.contains(SQLSTATE_RAISE_EXCEPTION) || message.contains(SQLSTATE_RAISE_EXCEPTION);

        if is_raise && message.contains(TRIGGER_MSG_AUDIENCE_POLICY_CONFLICT) {
            return Self::TopicMergeAudiencePolicyConflict(Uuid::nil());
        }
        if is_raise && message.contains(TRIGGER_MSG_CATEGORY_TOPIC_CREATE_DENIED) {
            return Self::Validation("Forum category does not allow topic creation".to_string());
        }
        if is_raise
            && (message.contains(TRIGGER_MSG_CATEGORY_LIFECYCLE_ACTIVE_PARENT)
                || message.contains(TRIGGER_MSG_CATEGORY_LIFECYCLE_ARCHIVED_CHILD)
                || message.contains(TRIGGER_MSG_CATEGORY_LIFECYCLE_GENERIC))
        {
            return Self::Validation("Forum category archive hierarchy violation".to_string());
        }
        if is_raise && message.contains(TRIGGER_MSG_CATEGORY_ICON) {
            return Self::Validation(
                "Forum category icon must be a bounded kebab-case design token".to_string(),
            );
        }
        if is_raise && message.contains(TRIGGER_MSG_CATEGORY_COLOR) {
            return Self::Validation(
                "Forum category color must use #RGB, #RGBA, #RRGGBB, or #RRGGBBAA".to_string(),
            );
        }
        // Fallback for drivers that flatten code into message only (e.g. SQLite
        // in tests) — keep the original contains checks.
        if message.contains(TRIGGER_MSG_AUDIENCE_POLICY_CONFLICT) {
            return Self::TopicMergeAudiencePolicyConflict(Uuid::nil());
        }
        if message.contains(TRIGGER_MSG_CATEGORY_TOPIC_CREATE_DENIED) {
            return Self::Validation("Forum category does not allow topic creation".to_string());
        }
        if message.contains(TRIGGER_MSG_CATEGORY_LIFECYCLE_ACTIVE_PARENT)
            || message.contains(TRIGGER_MSG_CATEGORY_LIFECYCLE_ARCHIVED_CHILD)
            || message.contains(TRIGGER_MSG_CATEGORY_LIFECYCLE_GENERIC)
        {
            return Self::Validation("Forum category archive hierarchy violation".to_string());
        }
        if message.contains(TRIGGER_MSG_CATEGORY_ICON) {
            return Self::Validation(
                "Forum category icon must be a bounded kebab-case design token".to_string(),
            );
        }
        if message.contains(TRIGGER_MSG_CATEGORY_COLOR) {
            return Self::Validation(
                "Forum category color must use #RGB, #RGBA, #RRGGBB, or #RRGGBBAA".to_string(),
            );
        }
        // Unique violation (23505) from concurrent inserts should surface as a
        // retryable conflict rather than a generic 500 — the owner will retry
        // with idempotency key. Keep it as Database so the caller can decide,
        // but preserve the code for observability.
        if debug.contains(SQLSTATE_UNIQUE_VIOLATION) || message.contains(SQLSTATE_UNIQUE_VIOLATION)
        {
            return Self::Database(error);
        }
        Self::Database(error)
    }
}


impl From<ForumError> for rustok_core::Error {
    fn from(error: ForumError) -> Self {
        match error {
            ForumError::CategoryNotFound(id) => {
                rustok_core::Error::NotFound(format!("category {id}"))
            }
            ForumError::TopicNotFound(id) => rustok_core::Error::NotFound(format!("topic {id}")),
            ForumError::ReplyNotFound(id) => rustok_core::Error::NotFound(format!("reply {id}")),
            ForumError::SolutionNotFound(id) => {
                rustok_core::Error::NotFound(format!("solution {id}"))
            }
            ForumError::TopicRouteNotFound | ForumError::CategoryRouteNotFound => {
                rustok_core::Error::NotFound("forum route".into())
            }
            ForumError::Forbidden(msg) => rustok_core::Error::Forbidden(msg),
            ForumError::Validation(msg) => rustok_core::Error::Validation(msg),
            ForumError::Internal(err) => err,
            other => rustok_core::Error::External(other.to_string()),
        }
    }
}
