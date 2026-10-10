use chrono::{DateTime, Utc};
use rustok_newsletter_api::SubscriberStatus;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Input for subscribing a new email address.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubscribeInput {
    /// Email address to subscribe.
    #[schema(max_length = 255)]
    pub email: String,
    /// Optional display name.
    #[schema(max_length = 255)]
    pub name: Option<String>,
    /// Optional locale preference for newsletter content.
    pub locale: Option<String>,
}

/// Input for updating a subscriber.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateSubscriberInput {
    #[schema(max_length = 255)]
    pub name: Option<String>,
    pub locale: Option<String>,
}

/// Subscriber response DTO.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubscriberResponse {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub status: SubscriberStatus,
    pub locale: Option<String>,
    pub subscribed_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub unsubscribed_at: Option<DateTime<Utc>>,
}

/// Subscriber list item for paginated listings.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubscriberSummary {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub status: SubscriberStatus,
    pub subscribed_at: DateTime<Utc>,
}

/// Paginated subscriber list query.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubscriberListQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub status: Option<SubscriberStatus>,
    pub search: Option<String>,
}

/// Paginated subscriber list response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubscriberListResponse {
    pub items: Vec<SubscriberSummary>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}
