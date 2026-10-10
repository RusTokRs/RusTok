use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Validated slug for a newsletter content source (e.g., "blog", "forum", "commerce").
///
/// Rules: non-empty, lowercase ASCII alphanumeric + hyphens, 1..63 chars.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentSourceSlug(String);

impl ContentSourceSlug {
    pub fn new(value: impl Into<String>) -> Result<Self, NewsletterApiError> {
        let value = value.into();
        if value.is_empty() || value.len() > 63 {
            return Err(NewsletterApiError::InvalidSlug(format!(
                "content source slug must be 1-63 characters, got {value:?}"
            )));
        }
        if !value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        {
            return Err(NewsletterApiError::InvalidSlug(format!(
                "content source slug must be lowercase ASCII alphanumeric + hyphens, got {value:?}"
            )));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ContentSourceSlug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A single content item that a source module provides for newsletter inclusion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsletterContentItem {
    /// Stable identifier within the source (e.g., blog post UUID).
    pub source_id: Uuid,
    /// Which module produced this content.
    pub source_slug: ContentSourceSlug,
    /// Human-readable title.
    pub title: String,
    /// Short summary or excerpt suitable for newsletter display.
    pub summary: Option<String>,
    /// Canonical URL for the content item.
    pub url: Option<String>,
    /// Optional featured image URL.
    pub image_url: Option<String>,
    /// When the content was published or last updated.
    pub published_at: DateTime<Utc>,
    /// Locale of the content item.
    pub locale: String,
    /// Optional structured metadata for template rendering.
    pub metadata: Option<serde_json::Value>,
}

/// Request to fetch recent content items from a source module.
#[derive(Debug, Clone)]
pub struct ContentFetchRequest {
    pub tenant_id: Uuid,
    pub source_slug: ContentSourceSlug,
    pub locale: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub limit: usize,
}

/// Subscriber status in the newsletter system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriberStatus {
    /// Registered but email not yet confirmed.
    Pending,
    /// Email confirmed, actively receiving newsletters.
    Active,
    /// Explicitly unsubscribed.
    Unsubscribed,
    /// Bounced or suppressed by delivery system.
    Suppressed,
}

/// Campaign lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignStatus {
    Draft,
    Scheduled,
    Sending,
    Sent,
    Cancelled,
}

/// API-level errors for newsletter contracts.
#[derive(Debug, Clone, thiserror::Error)]
pub enum NewsletterApiError {
    #[error("invalid slug: {0}")]
    InvalidSlug(String),
    #[error("provider not found: {0}")]
    ProviderNotFound(String),
    #[error("content fetch failed: {0}")]
    ContentFetchFailed(String),
}
