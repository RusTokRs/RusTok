/// Error Context Support
///
/// Provides rich context for errors including:
/// - Error chains (cause tracking)
/// - Structured metadata (key-value pairs)
/// - Request tracing IDs
/// - User-facing vs internal messages
///
/// Example:
/// ```ignore
/// use rustok_core::error::{ErrorContext, RichError};
///
/// fn process_order(order_id: Uuid) -> Result<Order, RichError> {
///     let order = fetch_order(order_id)
///         .context("Failed to fetch order")
///         .with_field("order_id", order_id.to_string())
///         .with_field("operation", "process_order")?;
///     
///     Ok(order)
/// }
/// ```
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

/// Rich error with context
#[derive(Debug)]
pub struct RichError {
    /// Error message
    pub message: String,

    /// Error kind/category
    pub kind: ErrorKind,

    /// HTTP status code (if applicable)
    pub status_code: u16,

    /// User-facing message (safe to show to end users)
    pub user_message: Option<String>,

    /// Error code for API responses
    pub error_code: Option<String>,

    /// Structured metadata
    pub fields: HashMap<String, String>,

    /// Request/trace ID
    pub trace_id: Option<Uuid>,

    /// Source error (cause chain)
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

/// Error kind/category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Validation errors (400)
    Validation,

    /// Authentication errors (401)
    Unauthenticated,

    /// Authorization errors (403)
    Forbidden,

    /// Resource not found (404)
    NotFound,

    /// Conflict (409) - duplicate, race condition
    Conflict,

    /// Rate limiting (429)
    RateLimited,

    /// Database errors (500)
    Database,

    /// External service errors (502/503)
    ExternalService,

    /// Internal errors (500)
    Internal,

    /// Timeout (504)
    Timeout,

    /// Business logic errors (400)
    BusinessLogic,
}

impl ErrorKind {
    /// Get default HTTP status code for this error kind
    pub fn status_code(&self) -> u16 {
        match self {
            ErrorKind::Validation => 400,
            ErrorKind::Unauthenticated => 401,
            ErrorKind::Forbidden => 403,
            ErrorKind::NotFound => 404,
            ErrorKind::Conflict => 409,
            ErrorKind::RateLimited => 429,
            ErrorKind::Database => 500,
            ErrorKind::ExternalService => 503,
            ErrorKind::Internal => 500,
            ErrorKind::Timeout => 504,
            ErrorKind::BusinessLogic => 400,
        }
    }

    /// Get default error code string
    pub fn error_code(&self) -> &'static str {
        match self {
            ErrorKind::Validation => "VALIDATION_ERROR",
            ErrorKind::Unauthenticated => "UNAUTHENTICATED",
            ErrorKind::Forbidden => "FORBIDDEN",
            ErrorKind::NotFound => "NOT_FOUND",
            ErrorKind::Conflict => "CONFLICT",
            ErrorKind::RateLimited => "RATE_LIMITED",
            ErrorKind::Database => "DATABASE_ERROR",
            ErrorKind::ExternalService => "EXTERNAL_SERVICE_ERROR",
            ErrorKind::Internal => "INTERNAL_ERROR",
            ErrorKind::Timeout => "TIMEOUT",
            ErrorKind::BusinessLogic => "BUSINESS_LOGIC_ERROR",
        }
    }
}

impl RichError {
    /// Create new rich error
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind,
            status_code: kind.status_code(),
            user_message: None,
            error_code: Some(kind.error_code().to_string()),
            fields: HashMap::new(),
            trace_id: None,
            source: None,
        }
    }

    /// Add user-facing message
    pub fn with_user_message(mut self, message: impl Into<String>) -> Self {
        self.user_message = Some(message.into());
        self
    }

    /// Add custom error code
    pub fn with_error_code(mut self, code: impl Into<String>) -> Self {
        self.error_code = Some(code.into());
        self
    }

    /// Add metadata field
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }

    /// Add trace ID
    pub fn with_trace_id(mut self, trace_id: Uuid) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    /// Add source error
    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// Set custom status code
    pub fn with_status_code(mut self, status_code: u16) -> Self {
        self.status_code = status_code;
        self
    }
}

impl fmt::Display for RichError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;

        if !self.fields.is_empty() {
            write!(f, " [")?;
            for (i, (k, v)) in self.fields.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}={}", k, v)?;
            }
            write!(f, "]")?;
        }

        Ok(())
    }
}

impl std::error::Error for RichError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref() as _)
    }
}

/// Extension trait for adding context to results
#[allow(clippy::result_large_err)]
pub trait ErrorContext<T> {
    /// Add context message to error
    fn context(self, message: impl Into<String>) -> Result<T, RichError>;

    /// Add context with structured fields
    fn with_context<F>(self, f: F) -> Result<T, RichError>
    where
        F: FnOnce() -> (String, Vec<(String, String)>);
}

impl<T, E> ErrorContext<T> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context(self, message: impl Into<String>) -> Result<T, RichError> {
        self.map_err(|e| RichError::new(ErrorKind::Internal, message).with_source(e))
    }

    fn with_context<F>(self, f: F) -> Result<T, RichError>
    where
        F: FnOnce() -> (String, Vec<(String, String)>),
    {
        self.map_err(|e| {
            let (message, fields) = f();
            let mut error = RichError::new(ErrorKind::Internal, message).with_source(e);

            for (key, value) in fields {
                error = error.with_field(key, value);
            }

            error
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rich_error_creation() {
        let error = RichError::new(ErrorKind::NotFound, "User not found")
            .with_field("user_id", "123")
            .with_user_message("The requested user does not exist");

        assert_eq!(error.kind, ErrorKind::NotFound);
        assert_eq!(error.status_code, 404);
        assert_eq!(error.fields.get("user_id"), Some(&"123".to_string()));
        assert!(error.user_message.is_some());
    }

    #[test]
    fn test_error_context() {
        fn inner_fn() -> Result<(), std::io::Error> {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "file not found",
            ))
        }

        let result = inner_fn().context("Failed to read config");

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.message, "Failed to read config");
        assert!(err.source.is_some());
    }

    #[test]
    fn test_error_kind_status_codes() {
        assert_eq!(ErrorKind::Validation.status_code(), 400);
        assert_eq!(ErrorKind::Unauthenticated.status_code(), 401);
        assert_eq!(ErrorKind::Forbidden.status_code(), 403);
        assert_eq!(ErrorKind::NotFound.status_code(), 404);
        assert_eq!(ErrorKind::Conflict.status_code(), 409);
    }
}
