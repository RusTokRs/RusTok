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

    #[error("invalid locale identifier: {value}")]
    InvalidLocale { value: String },

    #[error("schema must define at least one field")]
    EmptySchema,

    #[error("schema version must be greater than zero")]
    ZeroSchemaVersion,

    #[error("schema contains duplicate field: {0}")]
    DuplicateField(String),

    #[error("schema contains duplicate link: {0}")]
    DuplicateLink(String),

    #[error("schema link {link} must define source and target fields")]
    EmptyLinkFields { link: String },

    #[error("schema link {link} has {source_count} source fields and {target_count} target fields")]
    LinkFieldArityMismatch {
        link: String,
        source_count: usize,
        target_count: usize,
    },

    #[error("schema link references an unknown source field: {0}")]
    UnknownLinkSourceField(String),

    #[error("multi-value field cannot be sortable: {0}")]
    SortableManyField(String),

    #[error("query must select at least one field")]
    EmptySelection,

    #[error("page size must be greater than zero")]
    EmptyPage,

    #[error("page size exceeds the supported maximum")]
    PageTooLarge,

    #[error("offset pagination exceeds the bounded compatibility limit")]
    OffsetLimitExceeded,

    #[error("offset pagination is too deep")]
    OffsetTooDeep,
}
