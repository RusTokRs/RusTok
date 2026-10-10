use chrono::Utc;
use rustok_newsletter_api::CampaignStatus;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    QueryFilter,
};
use tracing::instrument;
use uuid::Uuid;

use crate::domain::CampaignLifecycle;
use crate::dto::{
    CampaignListQuery, CampaignListResponse, CampaignResponse, CampaignSummary,
    CreateCampaignInput, ScheduleCampaignInput, UpdateCampaignInput,
};
use crate::entities::campaign::{self, Entity as Campaign};
use crate::error::{NewsletterError, NewsletterResult};

/// Application service for newsletter campaign lifecycle management.
pub struct CampaignService {
    db: DatabaseConnection,
}

impl CampaignService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Create a new draft campaign.
    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, title = %input.title))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        input: CreateCampaignInput,
    ) -> NewsletterResult<CampaignResponse> {
        validate_campaign_input(&input.title, &input.subject)?;

        let now = Utc::now();
        let content_sources: Vec<String> = input.content_sources;

        let model = campaign::ActiveModel {
            id: Set(Uuid::now_v7()),
            tenant_id: Set(tenant_id),
            title: Set(input.title),
            subject: Set(input.subject),
            preheader: Set(input.preheader),
            status: Set("draft".to_string()),
            content_sources: Set(serde_json::to_value(&content_sources).unwrap_or_default()),
            segment_id: Set(input.segment_id),
            scheduled_at: Set(None),
            sent_at: Set(None),
            created_by: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let result = model.insert(&self.db).await?;
        self.to_response(result)
    }

    /// Update a draft campaign.
    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, campaign_id = %campaign_id))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        campaign_id: Uuid,
        input: UpdateCampaignInput,
    ) -> NewsletterResult<CampaignResponse> {
        let campaign = self.find_campaign(tenant_id, campaign_id).await?;
        let status = parse_status(&campaign.status)?;

        if !CampaignLifecycle::is_editable(status) {
            return Err(NewsletterError::invalid_campaign_status(
                format!("{:?}", status),
                "Draft",
            ));
        }

        let now = Utc::now();
        let mut active_model: campaign::ActiveModel = campaign.into();

        if let Some(title) = input.title {
            if title.trim().is_empty() {
                return Err(NewsletterError::validation("Title cannot be empty"));
            }
            active_model.title = Set(title);
        }
        if let Some(subject) = input.subject {
            if subject.trim().is_empty() {
                return Err(NewsletterError::validation("Subject cannot be empty"));
            }
            active_model.subject = Set(subject);
        }
        if let Some(preheader) = input.preheader {
            active_model.preheader = Set(Some(preheader));
        }
        if let Some(sources) = input.content_sources {
            active_model.content_sources =
                Set(serde_json::to_value(&sources).unwrap_or_default());
        }
        if let Some(segment_id) = input.segment_id {
            active_model.segment_id = Set(Some(segment_id));
        }

        active_model.updated_at = Set(now.into());
        let result = active_model.update(&self.db).await?;
        self.to_response(result)
    }

    /// Schedule a draft campaign for delivery.
    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, campaign_id = %campaign_id))]
    pub async fn schedule(
        &self,
        tenant_id: Uuid,
        campaign_id: Uuid,
        input: ScheduleCampaignInput,
    ) -> NewsletterResult<CampaignResponse> {
        let campaign = self.find_campaign(tenant_id, campaign_id).await?;
        let status = parse_status(&campaign.status)?;

        if !CampaignLifecycle::can_transition(status, CampaignStatus::Scheduled) {
            return Err(NewsletterError::invalid_campaign_status(
                format!("{:?}", status),
                "Draft (for scheduling)",
            ));
        }

        if input.scheduled_at <= Utc::now() {
            return Err(NewsletterError::validation(
                "scheduled_at must be in the future",
            ));
        }

        let now = Utc::now();
        let mut active_model: campaign::ActiveModel = campaign.into();
        active_model.status = Set("scheduled".to_string());
        active_model.scheduled_at = Set(Some(input.scheduled_at.into()));
        active_model.updated_at = Set(now.into());
        let result = active_model.update(&self.db).await?;
        self.to_response(result)
    }

    /// Cancel a draft or scheduled campaign.
    #[instrument(skip(self), fields(tenant_id = %tenant_id, campaign_id = %campaign_id))]
    pub async fn cancel(
        &self,
        tenant_id: Uuid,
        campaign_id: Uuid,
    ) -> NewsletterResult<CampaignResponse> {
        let campaign = self.find_campaign(tenant_id, campaign_id).await?;
        let status = parse_status(&campaign.status)?;

        if !CampaignLifecycle::can_transition(status, CampaignStatus::Cancelled) {
            return Err(NewsletterError::invalid_campaign_status(
                format!("{:?}", status),
                "Draft or Scheduled (for cancellation)",
            ));
        }

        let now = Utc::now();
        let mut active_model: campaign::ActiveModel = campaign.into();
        active_model.status = Set("cancelled".to_string());
        active_model.updated_at = Set(now.into());
        let result = active_model.update(&self.db).await?;
        self.to_response(result)
    }

    /// List campaigns with pagination and filtering.
    pub async fn list(
        &self,
        tenant_id: Uuid,
        query: CampaignListQuery,
    ) -> NewsletterResult<CampaignListResponse> {
        use sea_orm::PaginatorTrait;

        let page = query.page.unwrap_or(1).max(1);
        let per_page = query.per_page.unwrap_or(25).min(100).max(1);

        let mut find = Campaign::find().filter(campaign::Column::TenantId.eq(tenant_id));

        if let Some(status) = query.status {
            find = find.filter(campaign::Column::Status.eq(status_to_string(status)));
        }

        let total = find.clone().count(&self.db).await?;

        let items: Vec<CampaignSummary> = find
            .order_by_desc(campaign::Column::CreatedAt)
            .into_partial_select()
            .offset(Some((page - 1) * per_page))
            .limit(Some(per_page))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|c| CampaignSummary {
                id: c.id,
                title: c.title,
                status: parse_status(&c.status).unwrap_or(CampaignStatus::Draft),
                scheduled_at: c.scheduled_at.map(Into::into),
                sent_at: c.sent_at.map(Into::into),
                created_at: c.created_at.into(),
            })
            .collect();

        Ok(CampaignListResponse {
            items,
            total,
            page,
            per_page,
        })
    }

    /// Get a single campaign by ID.
    pub async fn get(
        &self,
        tenant_id: Uuid,
        campaign_id: Uuid,
    ) -> NewsletterResult<CampaignResponse> {
        let campaign = self.find_campaign(tenant_id, campaign_id).await?;
        self.to_response(campaign)
    }

    /// Delete a draft or cancelled campaign.
    #[instrument(skip(self), fields(tenant_id = %tenant_id, campaign_id = %campaign_id))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        campaign_id: Uuid,
    ) -> NewsletterResult<()> {
        let campaign = self.find_campaign(tenant_id, campaign_id).await?;
        let status = parse_status(&campaign.status)?;

        if !CampaignLifecycle::is_terminal(status) && status != CampaignStatus::Draft {
            return Err(NewsletterError::validation(
                "Only draft or cancelled campaigns can be deleted",
            ));
        }

        campaign.delete(&self.db).await?;
        Ok(())
    }

    // --- private helpers ---

    async fn find_campaign(
        &self,
        tenant_id: Uuid,
        campaign_id: Uuid,
    ) -> NewsletterResult<campaign::Model> {
        Campaign::find_by_id(campaign_id)
            .filter(campaign::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| NewsletterError::not_found(format!("campaign {campaign_id}")))
    }

    fn to_response(&self, model: campaign::Model) -> NewsletterResult<CampaignResponse> {
        let content_sources: Vec<String> =
            serde_json::from_value(model.content_sources.clone()).unwrap_or_default();

        Ok(CampaignResponse {
            id: model.id,
            title: model.title,
            subject: model.subject,
            preheader: model.preheader,
            status: parse_status(&model.status)?,
            content_sources,
            segment_id: model.segment_id,
            scheduled_at: model.scheduled_at.map(Into::into),
            sent_at: model.sent_at.map(Into::into),
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        })
    }
}

fn parse_status(value: &str) -> NewsletterResult<CampaignStatus> {
    match value {
        "draft" => Ok(CampaignStatus::Draft),
        "scheduled" => Ok(CampaignStatus::Scheduled),
        "sending" => Ok(CampaignStatus::Sending),
        "sent" => Ok(CampaignStatus::Sent),
        "cancelled" => Ok(CampaignStatus::Cancelled),
        other => Err(NewsletterError::validation(format!(
            "unknown campaign status: {other}"
        ))),
    }
}

fn status_to_string(status: CampaignStatus) -> String {
    match status {
        CampaignStatus::Draft => "draft".to_string(),
        CampaignStatus::Scheduled => "scheduled".to_string(),
        CampaignStatus::Sending => "sending".to_string(),
        CampaignStatus::Sent => "sent".to_string(),
        CampaignStatus::Cancelled => "cancelled".to_string(),
    }
}

fn validate_campaign_input(title: &str, subject: &str) -> NewsletterResult<()> {
    if title.trim().is_empty() {
        return Err(NewsletterError::validation("Title cannot be empty"));
    }
    if title.len() > 512 {
        return Err(NewsletterError::validation("Title exceeds 512 characters"));
    }
    if subject.trim().is_empty() {
        return Err(NewsletterError::validation("Subject cannot be empty"));
    }
    if subject.len() > 512 {
        return Err(NewsletterError::validation("Subject exceeds 512 characters"));
    }
    Ok(())
}
