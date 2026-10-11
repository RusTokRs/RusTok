use async_graphql::*;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::NewsletterGraphqlRuntimeData;
use crate::dto::ScheduleCampaignInput;
use crate::graphql::types::*;
use crate::services::{CampaignService, SubscriberService};

/// Newsletter GraphQL mutations.
pub struct NewsletterMutation;

#[Object]
impl NewsletterMutation {
    /// Subscribe a new email address to the newsletter.
    async fn newsletter_subscribe(
        &self,
        ctx: &Context<'_>,
        input: GqlSubscribeInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlSubscriber> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let service = SubscriberService::new(runtime.db.clone());
        let response = service
            .subscribe(tenant_id, input.into())
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(response.into())
    }

    /// Confirm a pending subscription.
    async fn newsletter_confirm_subscription(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let service = SubscriberService::new(runtime.db.clone());
        service
            .confirm(tenant_id, id)
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(true)
    }

    /// Unsubscribe from the newsletter.
    async fn newsletter_unsubscribe(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let service = SubscriberService::new(runtime.db.clone());
        service
            .unsubscribe(tenant_id, id)
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(true)
    }

    /// Update subscriber metadata.
    async fn newsletter_update_subscriber(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: GqlUpdateSubscriberInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlSubscriber> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let service = SubscriberService::new(runtime.db.clone());
        let response = service
            .update(tenant_id, id, input.into())
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(response.into())
    }

    /// Delete a subscriber permanently.
    async fn newsletter_delete_subscriber(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let service = SubscriberService::new(runtime.db.clone());
        service
            .delete(tenant_id, id)
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(true)
    }

    /// Create a new draft campaign.
    async fn newsletter_create_campaign(
        &self,
        ctx: &Context<'_>,
        input: GqlCreateCampaignInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlCampaign> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let service = CampaignService::new(runtime.db.clone());
        let response = service
            .create(tenant_id, input.into())
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(response.into())
    }

    /// Update a draft campaign.
    async fn newsletter_update_campaign(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: GqlUpdateCampaignInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlCampaign> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let service = CampaignService::new(runtime.db.clone());
        let response = service
            .update(tenant_id, id, input.into())
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(response.into())
    }

    /// Schedule a draft campaign for delivery.
    async fn newsletter_schedule_campaign(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: GqlScheduleCampaignInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlCampaign> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let scheduled_at: DateTime<Utc> = DateTime::parse_from_rfc3339(&input.scheduled_at)
            .map_err(|e| Error::new(format!("invalid scheduled_at format: {e}")))?
            .into();

        let service = CampaignService::new(runtime.db.clone());
        let response = service
            .schedule(tenant_id, id, ScheduleCampaignInput { scheduled_at })
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(response.into())
    }

    /// Cancel a draft or scheduled campaign.
    async fn newsletter_cancel_campaign(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlCampaign> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let service = CampaignService::new(runtime.db.clone());
        let response = service
            .cancel(tenant_id, id)
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(response.into())
    }

    /// Delete a draft or cancelled campaign.
    async fn newsletter_delete_campaign(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let service = CampaignService::new(runtime.db.clone());
        service
            .delete(tenant_id, id)
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(true)
    }
}
