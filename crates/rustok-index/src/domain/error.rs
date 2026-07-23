use thiserror::Error;

/// Errors produced by the database-independent Index Engine domain model.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    #[error("{kind} identifier must not be empty")]
    EmptyIdentifier { kind: &'static str },

    #[error("{kind} identifier contains an invalid character: {value}")]
    InvalidIdentifier {
        kind: &'static str,
        value: String,
    },

    #[error("schema contains duplicate field: {0}")]
    DuplicateField(String),

    #[error("schema contains duplicate link: {0}")]
    DuplicateLink(String),

    #[error("schema link references an unknown source field: {0}")]
    UnknownLinkSourceField(String),

    #[error("query must select at least one field")]
    EmptySelection,

    #[error("page size must be greater than zero")]
    EmptyPage,

    #[error("offset pagination exceeds the bounded compatibility limit")]
    OffsetLimitExceeded,
}
