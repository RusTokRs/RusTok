use async_graphql::*;
use chrono::{DateTime, Utc};
use rustok_newsletter_api::{
    CampaignStatus as ApiCampaignStatus, SubscriberStatus as ApiSubscriberStatus,
};
use uuid::Uuid;

use crate::dto;

/// Subscriber status enum for GraphQL.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlSubscriberStatus {
    Pending,
    Active,
    Unsubscribed,
    Suppressed,
}

impl From<ApiSubscriberStatus> for GqlSubscriberStatus {
    fn from(value: ApiSubscriberStatus) -> Self {
        match value {
            ApiSubscriberStatus::Pending => Self::Pending,
            ApiSubscriberStatus::Active => Self::Active,
            ApiSubscriberStatus::Unsubscribed => Self::Unsubscribed,
            ApiSubscriberStatus::Suppressed => Self::Suppressed,
        }
    }
}

impl From<GqlSubscriberStatus> for ApiSubscriberStatus {
    fn from(value: GqlSubscriberStatus) -> Self {
        match value {
            GqlSubscriberStatus::Pending => Self::Pending,
            GqlSubscriberStatus::Active => Self::Active,
            GqlSubscriberStatus::Unsubscribed => Self::Unsubscribed,
            GqlSubscriberStatus::Suppressed => Self::Suppressed,
        }
    }
}

/// Campaign status enum for GraphQL.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlCampaignStatus {
    Draft,
    Scheduled,
    Sending,
    Sent,
    Cancelled,
}

impl From<ApiCampaignStatus> for GqlCampaignStatus {
    fn from(value: ApiCampaignStatus) -> Self {
        match value {
            ApiCampaignStatus::Draft => Self::Draft,
            ApiCampaignStatus::Scheduled => Self::Scheduled,
            ApiCampaignStatus::Sending => Self::Sending,
            ApiCampaignStatus::Sent => Self::Sent,
            ApiCampaignStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<GqlCampaignStatus> for ApiCampaignStatus {
    fn from(value: GqlCampaignStatus) -> Self {
        match value {
            GqlCampaignStatus::Draft => Self::Draft,
            GqlCampaignStatus::Scheduled => Self::Scheduled,
            GqlCampaignStatus::Sending => Self::Sending,
            GqlCampaignStatus::Sent => Self::Sent,
            GqlCampaignStatus::Cancelled => Self::Cancelled,
        }
    }
}

/// Newsletter subscriber object.
#[derive(SimpleObject)]
pub struct GqlSubscriber {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub status: GqlSubscriberStatus,
    pub locale: Option<String>,
    pub subscribed_at: String,
    pub confirmed_at: Option<String>,
    pub unsubscribed_at: Option<String>,
}

impl From<dto::SubscriberResponse> for GqlSubscriber {
    fn from(value: dto::SubscriberResponse) -> Self {
        Self {
            id: value.id,
            email: value.email,
            name: value.name,
            status: value.status.into(),
            locale: value.locale,
            subscribed_at: value.subscribed_at.to_rfc3339(),
            confirmed_at: value.confirmed_at.map(|dt| dt.to_rfc3339()),
            unsubscribed_at: value.unsubscribed_at.map(|dt| dt.to_rfc3339()),
        }
    }
}

/// Newsletter subscriber list item.
#[derive(SimpleObject)]
pub struct GqlSubscriberListItem {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub status: GqlSubscriberStatus,
    pub subscribed_at: String,
}

impl From<dto::SubscriberSummary> for GqlSubscriberListItem {
    fn from(value: dto::SubscriberSummary) -> Self {
        Self {
            id: value.id,
            email: value.email,
            name: value.name,
            status: value.status.into(),
            subscribed_at: value.subscribed_at.to_rfc3339(),
        }
    }
}

/// Paginated subscriber list.
#[derive(SimpleObject)]
pub struct GqlSubscriberList {
    pub items: Vec<GqlSubscriberListItem>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// Newsletter campaign object.
#[derive(SimpleObject)]
pub struct GqlCampaign {
    pub id: Uuid,
    pub title: String,
    pub subject: String,
    pub preheader: Option<String>,
    pub status: GqlCampaignStatus,
    pub content_sources: Vec<String>,
    pub segment_id: Option<Uuid>,
    pub scheduled_at: Option<String>,
    pub sent_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<dto::CampaignResponse> for GqlCampaign {
    fn from(value: dto::CampaignResponse) -> Self {
        Self {
            id: value.id,
            title: value.title,
            subject: value.subject,
            preheader: value.preheader,
            status: value.status.into(),
            content_sources: value.content_sources,
            segment_id: value.segment_id,
            scheduled_at: value.scheduled_at.map(|dt| dt.to_rfc3339()),
            sent_at: value.sent_at.map(|dt| dt.to_rfc3339()),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

/// Newsletter campaign list item.
#[derive(SimpleObject)]
pub struct GqlCampaignListItem {
    pub id: Uuid,
    pub title: String,
    pub status: GqlCampaignStatus,
    pub scheduled_at: Option<String>,
    pub sent_at: Option<String>,
    pub created_at: String,
}

impl From<dto::CampaignSummary> for GqlCampaignListItem {
    fn from(value: dto::CampaignSummary) -> Self {
        Self {
            id: value.id,
            title: value.title,
            status: value.status.into(),
            scheduled_at: value.scheduled_at.map(|dt| dt.to_rfc3339()),
            sent_at: value.sent_at.map(|dt| dt.to_rfc3339()),
            created_at: value.created_at.to_rfc3339(),
        }
    }
}

/// Paginated campaign list.
#[derive(SimpleObject)]
pub struct GqlCampaignList {
    pub items: Vec<GqlCampaignListItem>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

// --- Input types ---

/// Input for subscribing a new email address.
#[derive(InputObject)]
pub struct GqlSubscribeInput {
    pub email: String,
    pub name: Option<String>,
    pub locale: Option<String>,
}

impl From<GqlSubscribeInput> for dto::SubscribeInput {
    fn from(value: GqlSubscribeInput) -> Self {
        Self {
            email: value.email,
            name: value.name,
            locale: value.locale,
        }
    }
}

/// Input for updating a subscriber.
#[derive(InputObject)]
pub struct GqlUpdateSubscriberInput {
    pub name: Option<String>,
    pub locale: Option<String>,
}

impl From<GqlUpdateSubscriberInput> for dto::UpdateSubscriberInput {
    fn from(value: GqlUpdateSubscriberInput) -> Self {
        Self {
            name: value.name,
            locale: value.locale,
        }
    }
}

/// Input for creating a new campaign.
#[derive(InputObject)]
pub struct GqlCreateCampaignInput {
    pub title: String,
    pub subject: String,
    pub preheader: Option<String>,
    pub content_sources: Vec<String>,
    pub segment_id: Option<Uuid>,
}

impl From<GqlCreateCampaignInput> for dto::CreateCampaignInput {
    fn from(value: GqlCreateCampaignInput) -> Self {
        Self {
            title: value.title,
            subject: value.subject,
            preheader: value.preheader,
            content_sources: value.content_sources,
            segment_id: value.segment_id,
        }
    }
}

/// Input for updating a draft campaign.
#[derive(InputObject)]
pub struct GqlUpdateCampaignInput {
    pub title: Option<String>,
    pub subject: Option<String>,
    pub preheader: Option<String>,
    pub content_sources: Option<Vec<String>>,
    pub segment_id: Option<Uuid>,
}

impl From<GqlUpdateCampaignInput> for dto::UpdateCampaignInput {
    fn from(value: GqlUpdateCampaignInput) -> Self {
        Self {
            title: value.title,
            subject: value.subject,
            preheader: value.preheader,
            content_sources: value.content_sources,
            segment_id: value.segment_id,
        }
    }
}

/// Input for scheduling a campaign.
#[derive(InputObject)]
pub struct GqlScheduleCampaignInput {
    pub scheduled_at: String,
}
