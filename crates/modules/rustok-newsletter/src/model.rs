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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentFetchRequest {
    pub tenant_id: Uuid,
    pub source_slug: ContentSourceSlug,
    pub locale: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub limit: usize,
}

/// Subscriber status in the newsletter system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
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

impl SubscriberStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Unsubscribed => "unsubscribed",
            Self::Suppressed => "suppressed",
        }
    }
}

impl std::fmt::Display for SubscriberStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SubscriberStatus {
    type Err = NewsletterApiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "unsubscribed" => Ok(Self::Unsubscribed),
            "suppressed" => Ok(Self::Suppressed),
            other => Err(NewsletterApiError::InvalidStatus(format!(
                "unknown subscriber status: {other}"
            ))),
        }
    }
}

/// Campaign lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CampaignStatus {
    Draft,
    Scheduled,
    Sending,
    Sent,
    Cancelled,
}

impl CampaignStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Scheduled => "scheduled",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for CampaignStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for CampaignStatus {
    type Err = NewsletterApiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(Self::Draft),
            "scheduled" => Ok(Self::Scheduled),
            "sending" => Ok(Self::Sending),
            "sent" => Ok(Self::Sent),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(NewsletterApiError::InvalidStatus(format!(
                "unknown campaign status: {other}"
            ))),
        }
    }
}

/// API-level errors for newsletter contracts.
#[derive(Debug, Clone, thiserror::Error)]
pub enum NewsletterApiError {
    #[error("invalid slug: {0}")]
    InvalidSlug(String),
    #[error("invalid status: {0}")]
    InvalidStatus(String),
    #[error("provider not found: {0}")]
    ProviderNotFound(String),
    #[error("content fetch failed: {0}")]
    ContentFetchFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_source_slug_accepts_valid_values() {
        assert!(ContentSourceSlug::new("blog").is_ok());
        assert!(ContentSourceSlug::new("forum").is_ok());
        assert!(ContentSourceSlug::new("commerce").is_ok());
    }

    #[test]
    fn content_source_slug_rejects_invalid_values() {
        assert!(ContentSourceSlug::new("").is_err());
        assert!(ContentSourceSlug::new("Blog").is_err());
        assert!(ContentSourceSlug::new(" blog").is_err());
        assert!(ContentSourceSlug::new("blog/extra").is_err());
    }

    #[test]
    fn subscriber_status_display_and_parse_roundtrip() {
        for status in [
            SubscriberStatus::Pending,
            SubscriberStatus::Active,
            SubscriberStatus::Unsubscribed,
            SubscriberStatus::Suppressed,
        ] {
            let str_val = status.as_str();
            assert_eq!(status.to_string(), str_val);
            let parsed: SubscriberStatus = str_val.parse().expect("valid subscriber status");
            assert_eq!(parsed, status);
        }
        assert!("invalid_status".parse::<SubscriberStatus>().is_err());
    }

    #[test]
    fn campaign_status_display_and_parse_roundtrip() {
        for status in [
            CampaignStatus::Draft,
            CampaignStatus::Scheduled,
            CampaignStatus::Sending,
            CampaignStatus::Sent,
            CampaignStatus::Cancelled,
        ] {
            let str_val = status.as_str();
            assert_eq!(status.to_string(), str_val);
            let parsed: CampaignStatus = str_val.parse().expect("valid campaign status");
            assert_eq!(parsed, status);
        }
        assert!("unknown".parse::<CampaignStatus>().is_err());
    }

    #[test]
    fn subscriber_and_campaign_status_serde_snake_case() {
        let json = serde_json::to_string(&SubscriberStatus::Active).unwrap();
        assert_eq!(json, "\"active\"");
        let deserialized: SubscriberStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, SubscriberStatus::Active);

        let json = serde_json::to_string(&CampaignStatus::Draft).unwrap();
        assert_eq!(json, "\"draft\"");
        let deserialized: CampaignStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, CampaignStatus::Draft);
    }
}
