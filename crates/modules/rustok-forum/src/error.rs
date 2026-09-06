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

    #[error("Topic is locked")]
    TopicLocked,

    #[error("Topic is deleted")]
    TopicDeleted,

    #[error("Reply is deleted")]
    ReplyDeleted,

    #[error("Validation error: {0}")]
    Validation(String),

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
            Self::TopicLocked => "FORUM_TOPIC_LOCKED",
            Self::TopicDeleted => "FORUM_TOPIC_DELETED",
            Self::ReplyDeleted => "FORUM_REPLY_DELETED",
            Self::Validation(_) => "FORUM_VALIDATION_FAILED",
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
            _ => false,
        }
    }
}

impl From<sea_orm::DbErr> for ForumError {
    fn from(error: sea_orm::DbErr) -> Self {
        let message = error.to_string();
        if message.contains("forum topic merge audience policy conflict") {
            return Self::TopicMergeAudiencePolicyConflict(Uuid::nil());
        }
        if message.contains("forum category does not allow topic creation") {
            return Self::Validation("Forum category does not allow topic creation".to_string());
        }
        if message.contains("active forum category cannot have archived parent")
            || message.contains("archived forum category cannot have active child")
            || message.contains("forum category lifecycle")
        {
            return Self::Validation("Forum category archive hierarchy violation".to_string());
        }
        if message.contains("Forum category icon") {
            return Self::Validation(
                "Forum category icon must be a bounded kebab-case design token".to_string(),
            );
        }
        if message.contains("Forum category color") {
            return Self::Validation(
                "Forum category color must use #RGB, #RGBA, #RRGGBB, or #RRGGBBAA".to_string(),
            );
        }
        Self::Database(error)
    }
}

impl From<rustok_taxonomy::TaxonomyError> for ForumError {
    fn from(value: rustok_taxonomy::TaxonomyError) -> Self {
        match value {
            rustok_taxonomy::TaxonomyError::Database(err) => Self::from(err),
            rustok_taxonomy::TaxonomyError::Forbidden(message) => Self::Forbidden(message),
            rustok_taxonomy::TaxonomyError::Validation(message)
            | rustok_taxonomy::TaxonomyError::DuplicateCanonicalKey(message)
            | rustok_taxonomy::TaxonomyError::DuplicateSlug(message)
            | rustok_taxonomy::TaxonomyError::DuplicateAlias(message)
            | rustok_taxonomy::TaxonomyError::Conflict(message) => Self::Validation(message),
            rustok_taxonomy::TaxonomyError::TermNotFound(term_id) => {
                Self::Validation(format!("Taxonomy term not found: {term_id}"))
            }
            rustok_taxonomy::TaxonomyError::TranslationRevisionExhausted { term_id, locale } => {
                Self::Validation(format!(
                    "Taxonomy translation revision is exhausted for term {term_id} and locale {locale}"
                ))
            }
        }
    }
}
