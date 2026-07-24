use sea_orm::DbErr;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type SeoResult<T> = Result<T, SeoError>;

const SEO_FAILURE_MESSAGE_PREFIX: &str = "[seo_failure class=";

/// Stable operational category for every SEO failure.
///
/// Classification is deliberately separate from retry policy. `Retryable` means the underlying
/// failure may succeed on a later attempt; callers must still apply an explicit bounded retry
/// policy before rescheduling work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeoFailureClass {
    Retryable,
    Terminal,
    Validation,
    Configuration,
}

impl SeoFailureClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Retryable => "retryable",
            Self::Terminal => "terminal",
            Self::Validation => "validation",
            Self::Configuration => "configuration",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "retryable" => Some(Self::Retryable),
            "terminal" => Some(Self::Terminal),
            "validation" => Some(Self::Validation),
            "configuration" => Some(Self::Configuration),
            _ => None,
        }
    }

    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::Retryable)
    }
}

/// Machine-readable failure envelope suitable for logs, job `last_error` fields, and operator
/// diagnostics without coupling storage to the concrete [`SeoError`] representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeoFailure {
    pub class: SeoFailureClass,
    pub code: String,
    pub message: String,
}

impl SeoFailure {
    pub fn new(
        class: SeoFailureClass,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            class,
            code: code.into(),
            message: message.into(),
        }
    }

    /// Encode the class and stable code while retaining a readable message.
    pub fn durable_message(&self) -> String {
        format!(
            "{SEO_FAILURE_MESSAGE_PREFIX}{} code={}] {}",
            self.class.as_str(),
            self.code,
            self.message
        )
    }

    /// Parse a value produced by [`SeoFailure::durable_message`].
    pub fn parse_durable_message(value: &str) -> Option<Self> {
        let payload = value.strip_prefix(SEO_FAILURE_MESSAGE_PREFIX)?;
        let (class, payload) = payload.split_once(" code=")?;
        let (code, message) = payload.split_once("] ")?;
        Some(Self::new(
            SeoFailureClass::parse(class)?,
            code,
            message,
        ))
    }
}

#[derive(Debug, Error)]
pub enum SeoError {
    #[error("{0}")]
    Validation(String),
    #[error("SEO runtime configuration error: {0}")]
    Configuration(String),
    #[error("SEO target not found")]
    NotFound,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
}

impl SeoError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration(message.into())
    }

    /// Return the explicit operational class for this failure.
    pub const fn failure_class(&self) -> SeoFailureClass {
        match self {
            Self::Validation(_) => SeoFailureClass::Validation,
            Self::Configuration(_) => SeoFailureClass::Configuration,
            Self::NotFound | Self::PermissionDenied => SeoFailureClass::Terminal,
            Self::Database(_) => SeoFailureClass::Retryable,
        }
    }

    /// Return a stable code that does not depend on display text.
    pub const fn stable_code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "validation",
            Self::Configuration(_) => "configuration",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::Database(_) => "database",
        }
    }

    pub const fn is_retryable(&self) -> bool {
        self.failure_class().is_retryable()
    }

    pub fn failure(&self) -> SeoFailure {
        SeoFailure::new(self.failure_class(), self.stable_code(), self.to_string())
    }

    pub fn durable_message(&self) -> String {
        self.failure().durable_message()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_variant_has_an_explicit_failure_class() {
        let cases = [
            (
                SeoError::validation("invalid locale"),
                SeoFailureClass::Validation,
                "validation",
            ),
            (
                SeoError::configuration("missing public origin"),
                SeoFailureClass::Configuration,
                "configuration",
            ),
            (SeoError::NotFound, SeoFailureClass::Terminal, "not_found"),
            (
                SeoError::PermissionDenied,
                SeoFailureClass::Terminal,
                "permission_denied",
            ),
            (
                SeoError::Database(DbErr::Custom("temporarily unavailable".to_string())),
                SeoFailureClass::Retryable,
                "database",
            ),
        ];

        for (error, class, code) in cases {
            assert_eq!(error.failure_class(), class);
            assert_eq!(error.stable_code(), code);
            assert_eq!(error.is_retryable(), class == SeoFailureClass::Retryable);
        }
    }

    #[test]
    fn durable_failure_message_round_trips_class_code_and_message() {
        let error = SeoError::Database(DbErr::Custom("connection reset".to_string()));
        let encoded = error.durable_message();
        let decoded = SeoFailure::parse_durable_message(encoded.as_str())
            .expect("classified SEO failure should parse");

        assert_eq!(decoded.class, SeoFailureClass::Retryable);
        assert_eq!(decoded.code, "database");
        assert!(decoded.message.contains("connection reset"));
    }
}
