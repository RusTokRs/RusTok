use chrono::{DateTime, Utc};
use rustok_newsletter_api::{CampaignStatus, ContentSourceSlug};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Input for creating a new campaign.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateCampaignInput {
    #[schema(max_length = 512)]
    pub title: String,
    #[schema(max_length = 512)]
    pub subject: String,
    /// Optional preheader text for email clients.
    #[schema(max_length = 255)]
    pub preheader: Option<String>,
    /// Content source slugs to aggregate content from.
    pub content_sources: Vec<String>,
    /// Optional segment ID to target specific subscribers.
    pub segment_id: Option<Uuid>,
}

/// Input for updating a draft campaign.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateCampaignInput {
    #[schema(max_length = 512)]
    pub title: Option<String>,
    #[schema(max_length = 512)]
    pub subject: Option<String>,
    #[schema(max_length = 255)]
    pub preheader: Option<String>,
    pub content_sources: Option<Vec<String>>,
    pub segment_id: Option<Uuid>,
}

/// Input for scheduling a campaign.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScheduleCampaignInput {
    pub scheduled_at: DateTime<Utc>,
}

/// Campaign response DTO.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CampaignResponse {
    pub id: Uuid,
    pub title: String,
    pub subject: String,
    pub preheader: Option<String>,
    pub status: CampaignStatus,
    pub content_sources: Vec<String>,
    pub segment_id: Option<Uuid>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub sent_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Campaign list item for paginated listings.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CampaignSummary {
    pub id: Uuid,
    pub title: String,
    pub status: CampaignStatus,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub sent_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Paginated campaign list query.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CampaignListQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub status: Option<CampaignStatus>,
}

/// Paginated campaign list response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CampaignListResponse {
    pub items: Vec<CampaignSummary>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}
