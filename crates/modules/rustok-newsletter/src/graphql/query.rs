use async_graphql::*;
use uuid::Uuid;

use crate::NewsletterGraphqlRuntimeData;
use crate::dto::{CampaignListQuery, SubscriberListQuery};
use crate::graphql::types::*;
use crate::services::{CampaignService, SubscriberService};

/// Newsletter GraphQL queries.
pub struct NewsletterQuery;

#[Object]
impl NewsletterQuery {
    /// Get a subscriber by ID.
    async fn newsletter_subscriber(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlSubscriber> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let service = SubscriberService::new(runtime.db.clone());
        let response = service
            .get(tenant_id, id)
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(response.into())
    }

    /// List subscribers with pagination.
    async fn newsletter_subscribers(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        page: Option<i64>,
        per_page: Option<i64>,
        status: Option<GqlSubscriberStatus>,
        search: Option<String>,
    ) -> Result<GqlSubscriberList> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let query = SubscriberListQuery {
            page: page.map(|p| p as u64),
            per_page: per_page.map(|p| p as u64),
            status: status.map(Into::into),
            search,
        };

        let service = SubscriberService::new(runtime.db.clone());
        let response = service
            .list(tenant_id, query)
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(GqlSubscriberList {
            items: response.items.into_iter().map(Into::into).collect(),
            total: response.total as i64,
            page: response.page as i64,
            per_page: response.per_page as i64,
        })
    }

    /// Get a campaign by ID.
    async fn newsletter_campaign(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlCampaign> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let service = CampaignService::new(runtime.db.clone());
        let response = service
            .get(tenant_id, id)
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(response.into())
    }

    /// List campaigns with pagination.
    async fn newsletter_campaigns(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        page: Option<i64>,
        per_page: Option<i64>,
        status: Option<GqlCampaignStatus>,
    ) -> Result<GqlCampaignList> {
        let runtime = ctx.data::<std::sync::Arc<NewsletterGraphqlRuntimeData>>()?;
        let tenant_id = tenant_id.ok_or_else(|| Error::new("tenant_id is required"))?;

        let query = CampaignListQuery {
            page: page.map(|p| p as u64),
            per_page: per_page.map(|p| p as u64),
            status: status.map(Into::into),
        };

        let service = CampaignService::new(runtime.db.clone());
        let response = service
            .list(tenant_id, query)
            .await
            .map_err(|e| Error::new(e.to_string()))?;

        Ok(GqlCampaignList {
            items: response.items.into_iter().map(Into::into).collect(),
            total: response.total as i64,
            page: response.page as i64,
            per_page: response.per_page as i64,
        })
    }
}
