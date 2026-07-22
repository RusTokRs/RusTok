use rustok_notifications_api::NotificationProviderError;
use thiserror::Error;

pub type NotificationResult<T> = Result<T, NotificationError>;

#[derive(Debug, Error)]
pub enum NotificationError {
    #[error("notification source is not registered")]
    SourceUnavailable,
    #[error("notification source does not support this event type")]
    UnsupportedEvent,
    #[error("notification source event is invalid")]
    InvalidEvent,
    #[error("notification source provider rejected the event")]
    ProviderRejected,
    #[error("notification source provider failed")]
    ProviderFailure { retryable: bool },
    #[error("notification recipient policy is unavailable")]
    RecipientPolicyFailure { retryable: bool },
    #[error("notification source event identity conflicts with an existing inbox record")]
    SourceIdentityConflict,
    #[error("notification fan-out lease is unavailable")]
    LeaseUnavailable,
    #[error("notification fan-out cursor did not advance")]
    CursorDidNotAdvance,
    #[error("notification fan-out descriptor is invalid")]
    InvalidDescriptor,
    #[error("notification fan-out input is invalid: {0}")]
    Validation(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

impl NotificationError {
    pub const fn stable_code(&self) -> &'static str {
        match self {
            Self::SourceUnavailable => "NOTIFICATION_SOURCE_UNAVAILABLE",
            Self::UnsupportedEvent => "NOTIFICATION_SOURCE_EVENT_UNSUPPORTED",
            Self::InvalidEvent => "NOTIFICATION_SOURCE_EVENT_INVALID",
            Self::ProviderRejected => "NOTIFICATION_SOURCE_EVENT_REJECTED",
            Self::ProviderFailure { .. } => "NOTIFICATION_SOURCE_PROVIDER_FAILURE",
            Self::RecipientPolicyFailure { .. } => "NOTIFICATION_RECIPIENT_POLICY_FAILURE",
            Self::SourceIdentityConflict => "NOTIFICATION_SOURCE_IDENTITY_CONFLICT",
            Self::LeaseUnavailable => "NOTIFICATION_FANOUT_LEASE_UNAVAILABLE",
            Self::CursorDidNotAdvance => "NOTIFICATION_FANOUT_CURSOR_STALLED",
            Self::InvalidDescriptor => "NOTIFICATION_FANOUT_DESCRIPTOR_INVALID",
            Self::Validation(_) => "NOTIFICATION_VALIDATION_ERROR",
            Self::Database(_) => "NOTIFICATION_DATABASE_ERROR",
            Self::Serialization(_) => "NOTIFICATION_SERIALIZATION_ERROR",
        }
    }

    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::SourceUnavailable | Self::LeaseUnavailable | Self::Database(_) => true,
            Self::ProviderFailure { retryable } | Self::RecipientPolicyFailure { retryable } => {
                *retryable
            }
            Self::UnsupportedEvent
            | Self::InvalidEvent
            | Self::ProviderRejected
            | Self::SourceIdentityConflict
            | Self::CursorDidNotAdvance
            | Self::InvalidDescriptor
            | Self::Validation(_)
            | Self::Serialization(_) => false,
        }
    }
}

impl From<NotificationProviderError> for NotificationError {
    fn from(error: NotificationProviderError) -> Self {
        match error {
            NotificationProviderError::CapabilityUnavailable { retryable }
            | NotificationProviderError::Internal { retryable } => {
                Self::ProviderFailure { retryable }
            }
            NotificationProviderError::InvalidEvent => Self::InvalidEvent,
            NotificationProviderError::Rejected => Self::ProviderRejected,
        }
    }
}
