/// Error Response Formatting
///
/// Provides standardized JSON error responses for APIs.
///
/// Features:
/// - Consistent error structure across all endpoints
/// - User-friendly messages
/// - Structured field errors (for validation)
/// - Trace IDs for debugging
/// - RFC 7807 Problem Details compatible
///
/// Example:
/// ```rust
/// use rustok_core::error::{ErrorResponse, FieldError};
///
/// let response = ErrorResponse::validation()
///     .with_message("Invalid input data")
///     .with_field_error("email", "must be a valid email address")
///     .with_field_error("age", "must be greater than 0");
/// ```
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::context::RichError;

/// Standardized error response for APIs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// HTTP status code
    pub status: u16,

    /// Error code (e.g., "VALIDATION_ERROR", "NOT_FOUND")
    pub code: String,

    /// Human-readable error message
    pub message: String,

    /// Optional detailed explanation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,

    /// Field-specific errors (for validation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<HashMap<String, Vec<String>>>,

    /// Trace ID for debugging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<Uuid>,

    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

impl ErrorResponse {
    /// Create new error response
    pub fn new(status: u16, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
            detail: None,
            fields: None,
            trace_id: None,
            metadata: None,
        }
    }

    /// Create validation error (400)
    pub fn validation() -> Self {
        Self::new(400, "VALIDATION_ERROR", "Validation failed")
    }

    /// Create not found error (404)
    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::new(404, "NOT_FOUND", format!("{} not found", resource.into()))
    }

    /// Create forbidden error (403)
    pub fn forbidden() -> Self {
        Self::new(403, "FORBIDDEN", "Access denied")
    }

    /// Create unauthenticated error (401)
    pub fn unauthenticated() -> Self {
        Self::new(401, "UNAUTHENTICATED", "Authentication required")
    }

    /// Create conflict error (409)
    pub fn conflict(resource: impl Into<String>) -> Self {
        Self::new(
            409,
            "CONFLICT",
            format!("{} already exists", resource.into()),
        )
    }

    /// Create rate limit error (429)
    pub fn rate_limited() -> Self {
        Self::new(429, "RATE_LIMITED", "Too many requests")
    }

    /// Create internal error (500)
    pub fn internal() -> Self {
        Self::new(500, "INTERNAL_ERROR", "An internal error occurred")
    }

    /// Create timeout error (504)
    pub fn timeout() -> Self {
        Self::new(504, "TIMEOUT", "Request timed out")
    }

    /// Create external service error (503)
    pub fn external_service(service: impl Into<String>) -> Self {
        Self::new(
            503,
            "EXTERNAL_SERVICE_ERROR",
            format!("{} is unavailable", service.into()),
        )
    }

    /// Add custom message
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Add detail
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Add field error
    pub fn with_field_error(mut self, field: impl Into<String>, error: impl Into<String>) -> Self {
        let fields = self.fields.get_or_insert_with(HashMap::new);
        fields.entry(field.into()).or_default().push(error.into());
        self
    }

    /// Add trace ID
    pub fn with_trace_id(mut self, trace_id: Uuid) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    /// Add metadata field
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let metadata = self.metadata.get_or_insert_with(HashMap::new);
        metadata.insert(key.into(), value.into());
        self
    }
}

impl From<RichError> for ErrorResponse {
    fn from(err: RichError) -> Self {
        let message = err.user_message.unwrap_or_else(|| err.message.clone());

        let mut response = Self::new(
            err.status_code,
            err.error_code
                .unwrap_or_else(|| err.kind.error_code().to_string()),
            message,
        );

        if !err.fields.is_empty() {
            response.metadata = Some(err.fields);
        }

        if let Some(trace_id) = err.trace_id {
            response.trace_id = Some(trace_id);
        }

        response
    }
}

/// Field validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

impl FieldError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// Builder for validation errors with multiple field errors
pub struct ValidationErrorBuilder {
    errors: HashMap<String, Vec<String>>,
    trace_id: Option<Uuid>,
}

impl ValidationErrorBuilder {
    pub fn new() -> Self {
        Self {
            errors: HashMap::new(),
            trace_id: None,
        }
    }

    pub fn field(mut self, field: impl Into<String>, error: impl Into<String>) -> Self {
        self.errors
            .entry(field.into())
            .or_default()
            .push(error.into());
        self
    }

    pub fn trace_id(mut self, trace_id: Uuid) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    pub fn build(self) -> ErrorResponse {
        let mut response = ErrorResponse::validation();
        response.fields = Some(self.errors);
        if let Some(trace_id) = self.trace_id {
            response.trace_id = Some(trace_id);
        }
        response
    }
}

impl Default for ValidationErrorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;

    #[test]
    fn test_validation_error_response() {
        let response = ErrorResponse::validation()
            .with_field_error("email", "invalid email format")
            .with_field_error("email", "email already exists")
            .with_field_error("age", "must be greater than 0");

        assert_eq!(response.status, 400);
        assert_eq!(response.code, "VALIDATION_ERROR");

        let fields = response.fields.unwrap();
        assert_eq!(fields.get("email").unwrap().len(), 2);
        assert_eq!(fields.get("age").unwrap().len(), 1);
    }

    #[test]
    fn test_not_found_response() {
        let response = ErrorResponse::not_found("User");

        assert_eq!(response.status, 404);
        assert_eq!(response.code, "NOT_FOUND");
        assert!(response.message.contains("User"));
    }

    #[test]
    fn test_validation_builder() {
        let response = ValidationErrorBuilder::new()
            .field("email", "required")
            .field("password", "too short")
            .field("password", "must contain number")
            .build();

        assert_eq!(response.status, 400);
        let fields = response.fields.unwrap();
        assert_eq!(fields.get("password").unwrap().len(), 2);
    }

    #[test]
    fn test_rich_error_to_response() {
        let rich_error = RichError::new(ErrorKind::NotFound, "User not found")
            .with_user_message("The requested user does not exist")
            .with_field("user_id", "123")
            .with_trace_id(Uuid::new_v4());

        let response: ErrorResponse = rich_error.into();

        assert_eq!(response.status, 404);
        assert_eq!(response.message, "The requested user does not exist");
        assert!(response.trace_id.is_some());
        assert!(response.metadata.is_some());
    }
}
