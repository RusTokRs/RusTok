//! Typed Error System
//!
//! Provides domain-specific errors with standardized error codes,
//! retryability information, and structured context.
//!
//! # Features
//!
//! - **Typed Error Codes**: Machine-readable error codes for programmatic handling
//! - **Retryability**: Automatic determination if an error is retryable
//! - **Structured Context**: Rich error context with fields and tracing IDs
//! - **HTTP Mapping**: Automatic mapping to appropriate HTTP status codes
//!
//! # Example
//!
//! ```ignore
//! use rustok_core::typed_error::{ErrorCode, DomainError, TypedResult};
//!
//! fn process_order(order_id: Uuid) -> TypedResult<Order> {
//!     if !order_exists(order_id) {
//!         return Err(DomainError::not_found("Order", order_id));
//!     }
//!     
//!     if !has_inventory(order_id) {
//!         return Err(DomainError::business(
//!             ErrorCode::INSUFFICIENT_INVENTORY,
//!             "Insufficient inventory for order"
//!         ));
//!     }
//!     
//!     // Process order...
//! }
//! ```

use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Standardized error codes across all domains
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ErrorCode {
    // Authentication (1000-1099)
    #[serde(rename = "AUTH_INVALID_CREDENTIALS")]
    InvalidCredentials = 1000,
    #[serde(rename = "AUTH_TOKEN_EXPIRED")]
    TokenExpired = 1001,
    #[serde(rename = "AUTH_TOKEN_INVALID")]
    TokenInvalid = 1002,
    #[serde(rename = "AUTH_INSUFFICIENT_PERMISSIONS")]
    InsufficientPermissions = 1003,
    #[serde(rename = "AUTH_MFA_REQUIRED")]
    MfaRequired = 1004,
    #[serde(rename = "AUTH_ACCOUNT_LOCKED")]
    AccountLocked = 1005,

    // Validation (1100-1199)
    #[serde(rename = "VAL_INVALID_INPUT")]
    InvalidInput = 1100,
    #[serde(rename = "VAL_MISSING_FIELD")]
    MissingField = 1101,
    #[serde(rename = "VAL_INVALID_FORMAT")]
    InvalidFormat = 1102,
    #[serde(rename = "VAL_OUT_OF_RANGE")]
    OutOfRange = 1103,
    #[serde(rename = "VAL_DUPLICATE")]
    Duplicate = 1104,
    #[serde(rename = "VAL_INVALID_REFERENCE")]
    InvalidReference = 1105,

    // Resource (1200-1299)
    #[serde(rename = "RES_NOT_FOUND")]
    NotFound = 1200,
    #[serde(rename = "RES_ALREADY_EXISTS")]
    AlreadyExists = 1201,
    #[serde(rename = "RES_GONE")]
    Gone = 1202,
    #[serde(rename = "RES_CONFLICT")]
    Conflict = 1203,

    // Business Logic (1300-1399)
    #[serde(rename = "BUS_INSUFFICIENT_FUNDS")]
    InsufficientFunds = 1300,
    #[serde(rename = "BUS_INSUFFICIENT_INVENTORY")]
    InsufficientInventory = 1301,
    #[serde(rename = "BUS_ORDER_ALREADY_PROCESSED")]
    OrderAlreadyProcessed = 1302,
    #[serde(rename = "BUS_PAYMENT_FAILED")]
    PaymentFailed = 1303,
    #[serde(rename = "BUS_OPERATION_NOT_ALLOWED")]
    OperationNotAllowed = 1304,
    #[serde(rename = "BUS_QUOTA_EXCEEDED")]
    QuotaExceeded = 1305,
    #[serde(rename = "BUS_RATE_LIMITED")]
    RateLimited = 1306,

    // System (1500-1599)
    #[serde(rename = "SYS_DATABASE_ERROR")]
    DatabaseError = 1500,
    #[serde(rename = "SYS_CACHE_ERROR")]
    CacheError = 1501,
    #[serde(rename = "SYS_EXTERNAL_SERVICE_ERROR")]
    ExternalServiceError = 1502,
    #[serde(rename = "SYS_EXTERNAL_TIMEOUT")]
    ExternalTimeout = 1503,
    #[serde(rename = "SYS_CIRCUIT_OPEN")]
    CircuitOpen = 1504,
    #[serde(rename = "SYS_INTERNAL_ERROR")]
    InternalError = 1505,
    #[serde(rename = "SYS_NOT_IMPLEMENTED")]
    NotImplemented = 1506,
    #[serde(rename = "SYS_SERVICE_UNAVAILABLE")]
    ServiceUnavailable = 1507,
}

impl ErrorCode {
    /// Get the HTTP status code associated with this error
    pub fn http_status(&self) -> u16 {
        match self {
            // 401 Unauthorized
            Self::InvalidCredentials
            | Self::TokenExpired
            | Self::TokenInvalid
            | Self::MfaRequired => 401,

            // 403 Forbidden
            Self::InsufficientPermissions | Self::AccountLocked | Self::OperationNotAllowed => 403,

            // 404 Not Found
            Self::NotFound | Self::Gone => 404,

            // 409 Conflict
            Self::AlreadyExists | Self::Conflict => 409,

            // 400 Bad Request
            Self::InvalidInput
            | Self::MissingField
            | Self::InvalidFormat
            | Self::OutOfRange
            | Self::Duplicate
            | Self::InvalidReference => 400,

            // 422 Unprocessable Entity (business logic errors)
            Self::InsufficientFunds
            | Self::InsufficientInventory
            | Self::OrderAlreadyProcessed
            | Self::PaymentFailed
            | Self::QuotaExceeded => 422,

            // 429 Too Many Requests
            Self::RateLimited => 429,

            // 500 Internal Server Error
            Self::DatabaseError | Self::CacheError | Self::InternalError | Self::NotImplemented => {
                500
            }

            // 503 Service Unavailable
            Self::ExternalServiceError
            | Self::ExternalTimeout
            | Self::CircuitOpen
            | Self::ServiceUnavailable => 503,
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::ExternalTimeout
                | Self::ExternalServiceError
                | Self::CircuitOpen
                | Self::ServiceUnavailable
                | Self::DatabaseError
                | Self::CacheError
        )
    }

    /// Get the error category
    pub fn category(&self) -> ErrorCategory {
        let code = *self as u16;
        match code {
            1000..=1099 => ErrorCategory::Authentication,
            1100..=1199 => ErrorCategory::Validation,
            1200..=1299 => ErrorCategory::Resource,
            1300..=1399 => ErrorCategory::Business,
            1500..=1599 => ErrorCategory::System,
            _ => ErrorCategory::Unknown,
        }
    }

    /// Get the default error message
    pub fn default_message(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "Invalid credentials provided",
            Self::TokenExpired => "Authentication token has expired",
            Self::TokenInvalid => "Invalid authentication token",
            Self::InsufficientPermissions => "Insufficient permissions for this operation",
            Self::MfaRequired => "Multi-factor authentication required",
            Self::AccountLocked => "Account is locked",
            Self::InvalidInput => "Invalid input provided",
            Self::MissingField => "Required field is missing",
            Self::InvalidFormat => "Invalid format",
            Self::OutOfRange => "Value is out of allowed range",
            Self::Duplicate => "Duplicate entry",
            Self::InvalidReference => "Invalid reference",
            Self::NotFound => "Resource not found",
            Self::AlreadyExists => "Resource already exists",
            Self::Gone => "Resource is no longer available",
            Self::Conflict => "Resource conflict",
            Self::InsufficientFunds => "Insufficient funds",
            Self::InsufficientInventory => "Insufficient inventory",
            Self::OrderAlreadyProcessed => "Order has already been processed",
            Self::PaymentFailed => "Payment processing failed",
            Self::OperationNotAllowed => "Operation not allowed",
            Self::QuotaExceeded => "Quota exceeded",
            Self::RateLimited => "Rate limit exceeded",
            Self::DatabaseError => "Database error",
            Self::CacheError => "Cache error",
            Self::ExternalServiceError => "External service error",
            Self::ExternalTimeout => "External service timeout",
            Self::CircuitOpen => "Circuit breaker is open",
            Self::InternalError => "Internal server error",
            Self::NotImplemented => "Operation not implemented",
            Self::ServiceUnavailable => "Service temporarily unavailable",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(ToOwned::to_owned))
            .unwrap_or_else(|| format!("{:?}", self));
        write!(f, "{code}")
    }
}

/// Error category for grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Authentication,
    Validation,
    Resource,
    Business,
    System,
    Unknown,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authentication => write!(f, "authentication"),
            Self::Validation => write!(f, "validation"),
            Self::Resource => write!(f, "resource"),
            Self::Business => write!(f, "business"),
            Self::System => write!(f, "system"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Structured error with context
#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct DomainError {
    /// Error code
    pub code: ErrorCode,

    /// Human-readable error message
    pub message: String,

    /// Additional structured context
    pub context: HashMap<String, String>,

    /// Trace ID for request correlation
    pub trace_id: Option<Uuid>,

    /// Timestamp when the error occurred
    pub timestamp: DateTime<Utc>,

    /// Source error (if any)
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl DomainError {
    /// Create a new domain error
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            context: HashMap::new(),
            trace_id: None,
            timestamp: Utc::now(),
            source: None,
        }
    }

    /// Create from an error code with default message
    pub fn from_code(code: ErrorCode) -> Self {
        Self::new(code, code.default_message())
    }

    /// Create a "not found" error
    pub fn not_found(resource: impl fmt::Display, id: impl fmt::Display) -> Self {
        Self::new(
            ErrorCode::NotFound,
            format!("{} with id '{}' not found", resource, id),
        )
        .with_field("resource", resource.to_string())
        .with_field("id", id.to_string())
    }

    /// Create a "conflict" error
    pub fn conflict(resource: impl fmt::Display, reason: impl fmt::Display) -> Self {
        Self::new(ErrorCode::Conflict, format!("{}: {}", resource, reason))
            .with_field("resource", resource.to_string())
    }

    /// Create a validation error
    pub fn validation(field: impl fmt::Display, reason: impl fmt::Display) -> Self {
        Self::new(
            ErrorCode::InvalidInput,
            format!("Validation failed for field '{}': {}", field, reason),
        )
        .with_field("field", field.to_string())
    }

    /// Create a business logic error
    pub fn business(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }

    /// Add context field
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// Add multiple context fields
    pub fn with_fields(
        mut self,
        fields: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        for (key, value) in fields {
            self.context.insert(key.into(), value.into());
        }
        self
    }

    /// Set trace ID
    pub fn with_trace_id(mut self, trace_id: Uuid) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    /// Set source error
    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        self.code.is_retryable()
    }

    /// Get HTTP status code
    pub fn http_status(&self) -> u16 {
        self.code.http_status()
    }

    /// Get error category
    pub fn category(&self) -> ErrorCategory {
        self.code.category()
    }

    /// Convert to JSON representation
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "error": {
                "code": self.code,
                "message": &self.message,
                "category": self.category().to_string(),
                "retryable": self.is_retryable(),
                "context": self.context,
                "trace_id": self.trace_id,
                "timestamp": self.timestamp.to_rfc3339(),
            }
        })
    }
}

/// Result type alias with DomainError
pub type TypedResult<T> = std::result::Result<T, DomainError>;

/// Extension trait for converting Results to TypedResults
pub trait IntoTypedResult<T, E> {
    /// Convert to TypedResult with a mapping function
    fn into_typed<F>(self, f: F) -> TypedResult<T>
    where
        F: FnOnce(E) -> DomainError;

    /// Convert to TypedResult with a specific error code
    fn into_typed_with_code(self, code: ErrorCode, message: impl Into<String>) -> TypedResult<T>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> IntoTypedResult<T, E>
    for std::result::Result<T, E>
{
    fn into_typed<F>(self, f: F) -> TypedResult<T>
    where
        F: FnOnce(E) -> DomainError,
    {
        self.map_err(f)
    }

    fn into_typed_with_code(self, code: ErrorCode, message: impl Into<String>) -> TypedResult<T> {
        self.map_err(|e| DomainError::new(code, message).with_source(e))
    }
}

/// Macro for creating domain errors
#[macro_export]
macro_rules! domain_err {
    // With explicit message
    ($code:expr, $msg:expr) => {
        Err($crate::typed_error::DomainError::new($code, $msg))
    };

    // With format string
    ($code:expr, $fmt:expr, $($arg:tt)*) => {
        Err($crate::typed_error::DomainError::new($code, format!($fmt, $($arg)*)))
    };

    // Not found helper
    (not_found: $resource:expr, $id:expr) => {
        Err($crate::typed_error::DomainError::not_found($resource, $id))
    };

    // Validation helper
    (validation: $field:expr, $reason:expr) => {
        Err($crate::typed_error::DomainError::validation($field, $reason))
    };
}

/// Error response for APIs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponseBody {
    /// Error code
    pub code: String,

    /// Human-readable message
    pub message: String,

    /// Error category
    pub category: String,

    /// Whether the operation can be retried
    pub retryable: bool,

    /// Additional context
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub context: HashMap<String, String>,

    /// Trace ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,

    /// Timestamp
    pub timestamp: String,
}

impl From<&DomainError> for ErrorResponseBody {
    fn from(error: &DomainError) -> Self {
        Self {
            code: format!("{:?}", error.code),
            message: error.message.clone(),
            category: error.category().to_string(),
            retryable: error.is_retryable(),
            context: error.context.clone(),
            trace_id: error.trace_id.map(|id| id.to_string()),
            timestamp: error.timestamp.to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_http_status() {
        assert_eq!(ErrorCode::NotFound.http_status(), 404);
        assert_eq!(ErrorCode::InvalidCredentials.http_status(), 401);
        assert_eq!(ErrorCode::InsufficientPermissions.http_status(), 403);
        assert_eq!(ErrorCode::InvalidInput.http_status(), 400);
        assert_eq!(ErrorCode::InternalError.http_status(), 500);
    }

    #[test]
    fn test_error_code_retryable() {
        assert!(ErrorCode::ExternalTimeout.is_retryable());
        assert!(ErrorCode::CircuitOpen.is_retryable());
        assert!(!ErrorCode::NotFound.is_retryable());
        assert!(!ErrorCode::InvalidCredentials.is_retryable());
    }

    #[test]
    fn test_domain_error_creation() {
        let error = DomainError::not_found("User", "123");
        assert_eq!(error.code, ErrorCode::NotFound);
        assert_eq!(error.http_status(), 404);
        assert!(!error.is_retryable());
        assert_eq!(error.context.get("resource"), Some(&"User".to_string()));
        assert_eq!(error.context.get("id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_domain_error_with_fields() {
        let error =
            DomainError::validation("email", "invalid format").with_field("value", "test@invalid");

        assert_eq!(error.code, ErrorCode::InvalidInput);
        assert_eq!(error.context.get("field"), Some(&"email".to_string()));
        assert_eq!(
            error.context.get("value"),
            Some(&"test@invalid".to_string())
        );
    }

    #[test]
    fn test_error_response_body() {
        let error = DomainError::business(ErrorCode::InsufficientInventory, "Not enough stock");

        let response: ErrorResponseBody = (&error).into();
        assert_eq!(response.category, "business");
        assert!(!response.retryable);
        assert_eq!(response.message, "Not enough stock");
    }
}
