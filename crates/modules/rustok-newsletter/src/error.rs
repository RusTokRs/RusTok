use rustok_newsletter_api::NewsletterApiError;
use thiserror::Error;

/// Domain errors for the newsletter module.
#[derive(Debug, Error)]
pub enum NewsletterError {
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("subscriber already exists: {0}")]
    DuplicateSubscriber(String),

    #[error("campaign status invalid for operation: current={current}, required={required}")]
    InvalidCampaignStatus { current: String, required: String },

    #[error("delivery error: {0}")]
    Delivery(String),

    #[error("api error: {0}")]
    Api(#[from] NewsletterApiError),
}

pub type NewsletterResult<T> = Result<T, NewsletterError>;

impl NewsletterError {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn invalid_campaign_status(
        current: impl Into<String>,
        required: impl Into<String>,
    ) -> Self {
        Self::InvalidCampaignStatus {
            current: current.into(),
            required: required.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages_are_actionable() {
        let err = NewsletterError::not_found("subscriber 123");
        assert!(err.to_string().contains("subscriber 123"));

        let err = NewsletterError::invalid_campaign_status("draft", "scheduled");
        assert!(err.to_string().contains("draft"));
        assert!(err.to_string().contains("scheduled"));
    }
}
