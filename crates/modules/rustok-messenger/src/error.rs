use thiserror::Error;
use uuid::Uuid;

/// Result alias for messenger operations.
pub type MessengerResult<T> = Result<T, MessengerError>;

/// Module-specific error contract for highload messenger.
#[derive(Debug, Error)]
pub enum MessengerError {
    #[error("Conversation not found: {0}")]
    ConversationNotFound(Uuid),

    #[error("Access denied to conversation: {0}")]
    ConversationAccessDenied(Uuid),

    #[error("Member '{user_id}' not found in conversation '{conversation_id}'")]
    MemberNotFound {
        conversation_id: Uuid,
        user_id: Uuid,
    },

    #[error("Member '{user_id}' already exists in conversation '{conversation_id}'")]
    MemberAlreadyExists {
        conversation_id: Uuid,
        user_id: Uuid,
    },

    #[error("Message not found: {0}")]
    MessageNotFound(i64),

    #[error("Invalid message payload: {0}")]
    InvalidMessagePayload(String),

    #[error("Invalid sequence cursor: requested {requested}, current max {current_max}")]
    InvalidSequence {
        requested: i64,
        current_max: i64,
    },

    #[error("Sync cursor regression: target sequence {target_seq} is lower than current {current_seq}")]
    CursorRegression {
        target_seq: i64,
        current_seq: i64,
    },

    #[error("Invalid channel locale '{locale}': must be valid BCP 47 tag up to 32 characters")]
    InvalidLocale { locale: String },

    #[error("Message content exceeds maximum allowed size ({actual} bytes > {max} bytes)")]
    PayloadTooLarge { actual: usize, max: usize },

    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Unauthorized: valid authenticated context required")]
    Unauthorized,

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for MessengerError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<MessengerError> for rustok_web::HttpError {
    fn from(err: MessengerError) -> Self {
        match err {
            MessengerError::ConversationNotFound(id) => rustok_web::HttpError::not_found(
                "conversation_not_found",
                format!("Conversation '{id}' not found"),
            ),
            MessengerError::ConversationAccessDenied(id) => rustok_web::HttpError::new(
                axum::http::StatusCode::FORBIDDEN,
                "conversation_access_denied",
                format!("Access denied to conversation '{id}'"),
            ),
            MessengerError::MemberNotFound {
                conversation_id,
                user_id,
            } => rustok_web::HttpError::not_found(
                "member_not_found",
                format!("Member '{user_id}' not in conversation '{conversation_id}'"),
            ),
            MessengerError::MemberAlreadyExists {
                conversation_id,
                user_id,
            } => rustok_web::HttpError::new(
                axum::http::StatusCode::CONFLICT,
                "member_already_exists",
                format!("Member '{user_id}' already in conversation '{conversation_id}'"),
            ),
            MessengerError::MessageNotFound(id) => rustok_web::HttpError::not_found(
                "message_not_found",
                format!("Message '{id}' not found"),
            ),
            MessengerError::InvalidMessagePayload(msg) => {
                rustok_web::HttpError::bad_request("invalid_message_payload", msg)
            }
            MessengerError::InvalidSequence {
                requested,
                current_max,
            } => rustok_web::HttpError::bad_request(
                "invalid_sequence",
                format!("Requested seq {requested} exceeds max {current_max}"),
            ),
            MessengerError::CursorRegression {
                target_seq,
                current_seq,
            } => rustok_web::HttpError::bad_request(
                "cursor_regression",
                format!("Cannot regress cursor from {current_seq} to {target_seq}"),
            ),
            MessengerError::InvalidLocale { locale } => rustok_web::HttpError::bad_request(
                "invalid_locale",
                format!("Invalid locale '{locale}'"),
            ),
            MessengerError::PayloadTooLarge { actual, max } => rustok_web::HttpError::new(
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                format!("Payload size {actual} exceeds limit {max}"),
            ),
            MessengerError::Unauthorized => {
                rustok_web::HttpError::unauthorized("unauthorized", "Authentication required")
            }
            MessengerError::Database(err) => {
                tracing::error!("Database error in messenger module: {err}");
                rustok_web::HttpError::internal("A database error occurred")
            }
            MessengerError::Serialization(msg) => {
                rustok_web::HttpError::bad_request("serialization_error", msg)
            }
            MessengerError::Validation(msg) => {
                rustok_web::HttpError::bad_request("validation_error", msg)
            }
            MessengerError::NotFound(msg) => {
                rustok_web::HttpError::not_found("not_found", msg)
            }
            MessengerError::Internal(msg) => {
                tracing::error!("Internal error in messenger module: {msg}");
                rustok_web::HttpError::internal("An internal error occurred")
            }
        }
    }
}

impl From<MessengerError> for rustok_api::PortError {
    fn from(err: MessengerError) -> Self {
        match err {
            MessengerError::ConversationNotFound(id) => rustok_api::PortError::not_found(
                "conversation",
                id.to_string(),
            ),
            MessengerError::ConversationAccessDenied(id) => rustok_api::PortError::forbidden(
                "conversation_access_denied",
                format!("Access denied to conversation '{id}'"),
            ),
            MessengerError::Unauthorized => {
                rustok_api::PortError::forbidden("unauthorized", "Authentication required")
            }
            MessengerError::Database(err) => {
                rustok_api::PortError::unavailable("database_error", err.to_string())
            }
            other => rustok_api::PortError::validation("messenger_error", other.to_string()),
        }
    }
}

impl From<rustok_api::PortError> for MessengerError {
    fn from(err: rustok_api::PortError) -> Self {
        match err.kind {
            rustok_api::PortErrorKind::NotFound => Self::NotFound(err.message),
            rustok_api::PortErrorKind::Forbidden => Self::Unauthorized,
            rustok_api::PortErrorKind::Validation | rustok_api::PortErrorKind::Conflict => {
                Self::Validation(err.message)
            }
            rustok_api::PortErrorKind::Unavailable
            | rustok_api::PortErrorKind::Timeout
            | rustok_api::PortErrorKind::InvariantViolation => Self::Internal(err.message),
        }
    }
}
