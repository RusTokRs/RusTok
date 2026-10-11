use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use uuid::Uuid;

use crate::{NewsletterContentItem, SubscriberStatus};

/// Typed port for subscriber management operations.
///
/// Used by admin UI and external integrations to manage newsletter subscribers
/// through a stable, transport-neutral contract.
#[async_trait]
pub trait NewsletterSubscriberPort: Send + Sync {
    /// Subscribe an email address to the newsletter.
    ///
    /// Returns the subscriber ID. If the email already exists, returns the
    /// existing ID without changing status.
    async fn subscribe(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        email: String,
        name: Option<String>,
    ) -> Result<Uuid, PortError>;

    /// Confirm a pending subscription.
    async fn confirm(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        subscriber_id: Uuid,
    ) -> Result<(), PortError>;

    /// Unsubscribe an email address.
    async fn unsubscribe(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        subscriber_id: Uuid,
    ) -> Result<(), PortError>;

    /// Get subscriber status.
    async fn status(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        subscriber_id: Uuid,
    ) -> Result<SubscriberStatus, PortError>;
}

/// Typed port for campaign operations.
///
/// Used by admin UI to create, schedule, and manage newsletter campaigns.
#[async_trait]
pub trait NewsletterCampaignPort: Send + Sync {
    /// Create a new draft campaign.
    async fn create_campaign(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        title: String,
        subject: String,
        content_sources: Vec<crate::ContentSourceSlug>,
    ) -> Result<Uuid, PortError>;

    /// Schedule a campaign for delivery.
    async fn schedule_campaign(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        campaign_id: Uuid,
        scheduled_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), PortError>;

    /// Cancel a scheduled campaign.
    async fn cancel_campaign(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        campaign_id: Uuid,
    ) -> Result<(), PortError>;

    /// Fetch content items for a campaign from registered providers.
    async fn fetch_campaign_content(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        campaign_id: Uuid,
    ) -> Result<Vec<NewsletterContentItem>, PortError>;
}
