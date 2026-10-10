//! Cross-boundary port implementations for the newsletter module.
//!
//! These ports implement the contracts defined in `rustok-newsletter-api`
//! and provide stable, transport-neutral interfaces for external consumers.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_newsletter_api::{
    ContentProviderRegistry, ContentSourceSlug, NewsletterCampaignPort, NewsletterContentItem,
    NewsletterSubscriberPort, SubscriberStatus,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::dto::{CreateCampaignInput, ScheduleCampaignInput, SubscribeInput};
use crate::services::{CampaignService, SubscriberService};

/// Port implementation for subscriber management operations.
pub struct SubscriberPortImpl {
    db: DatabaseConnection,
}

impl SubscriberPortImpl {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl NewsletterSubscriberPort for SubscriberPortImpl {
    async fn subscribe(
        &self,
        _context: PortContext,
        tenant_id: Uuid,
        email: String,
        name: Option<String>,
    ) -> Result<Uuid, PortError> {
        let service = SubscriberService::new(self.db.clone());
        let input = SubscribeInput {
            email: email.clone(),
            name,
            locale: None,
        };
        match service.subscribe(tenant_id, input).await {
            Ok(response) => Ok(response.id),
            Err(crate::NewsletterError::DuplicateSubscriber(_)) => {
                let existing = service
                    .find_by_email(tenant_id, &email)
                    .await
                    .map_err(map_newsletter_error)?
                    .ok_or_else(|| PortError {
                        kind: rustok_api::PortErrorKind::Conflict,
                        code: "newsletter.duplicate_subscriber".to_string(),
                        message: format!("subscriber already exists: {email}"),
                        retryable: false,
                    })?;
                Ok(existing.id)
            }
            Err(err) => Err(map_newsletter_error(err)),
        }
    }

    async fn confirm(
        &self,
        _context: PortContext,
        tenant_id: Uuid,
        subscriber_id: Uuid,
    ) -> Result<(), PortError> {
        let service = SubscriberService::new(self.db.clone());
        service
            .confirm(tenant_id, subscriber_id)
            .await
            .map_err(map_newsletter_error)
    }

    async fn unsubscribe(
        &self,
        _context: PortContext,
        tenant_id: Uuid,
        subscriber_id: Uuid,
    ) -> Result<(), PortError> {
        let service = SubscriberService::new(self.db.clone());
        service
            .unsubscribe(tenant_id, subscriber_id)
            .await
            .map_err(map_newsletter_error)
    }

    async fn status(
        &self,
        _context: PortContext,
        tenant_id: Uuid,
        subscriber_id: Uuid,
    ) -> Result<SubscriberStatus, PortError> {
        let service = SubscriberService::new(self.db.clone());
        let response = service
            .get(tenant_id, subscriber_id)
            .await
            .map_err(map_newsletter_error)?;
        Ok(response.status)
    }
}

/// Port implementation for campaign operations.
pub struct CampaignPortImpl {
    db: DatabaseConnection,
    content_registry: Option<Arc<ContentProviderRegistry>>,
}

impl CampaignPortImpl {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            content_registry: None,
        }
    }

    pub fn with_content_registry(
        db: DatabaseConnection,
        content_registry: Arc<ContentProviderRegistry>,
    ) -> Self {
        Self {
            db,
            content_registry: Some(content_registry),
        }
    }
}

#[async_trait]
impl NewsletterCampaignPort for CampaignPortImpl {
    async fn create_campaign(
        &self,
        _context: PortContext,
        tenant_id: Uuid,
        title: String,
        subject: String,
        content_sources: Vec<ContentSourceSlug>,
    ) -> Result<Uuid, PortError> {
        let service = CampaignService::new(self.db.clone());
        let input = CreateCampaignInput {
            title,
            subject,
            preheader: None,
            content_sources: content_sources
                .into_iter()
                .map(|s| s.as_str().to_string())
                .collect(),
            segment_id: None,
        };
        let response = service
            .create(tenant_id, input)
            .await
            .map_err(map_newsletter_error)?;
        Ok(response.id)
    }

    async fn schedule_campaign(
        &self,
        _context: PortContext,
        tenant_id: Uuid,
        campaign_id: Uuid,
        scheduled_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), PortError> {
        let service = CampaignService::new(self.db.clone());
        let input = ScheduleCampaignInput { scheduled_at };
        service
            .schedule(tenant_id, campaign_id, input)
            .await
            .map_err(map_newsletter_error)?;
        Ok(())
    }

    async fn cancel_campaign(
        &self,
        _context: PortContext,
        tenant_id: Uuid,
        campaign_id: Uuid,
    ) -> Result<(), PortError> {
        let service = CampaignService::new(self.db.clone());
        service
            .cancel(tenant_id, campaign_id)
            .await
            .map_err(map_newsletter_error)?;
        Ok(())
    }

    async fn fetch_campaign_content(
        &self,
        _context: PortContext,
        tenant_id: Uuid,
        campaign_id: Uuid,
    ) -> Result<Vec<NewsletterContentItem>, PortError> {
        let Some(registry) = &self.content_registry else {
            return Ok(Vec::new());
        };

        let service = CampaignService::new(self.db.clone());
        let campaign = service
            .get(tenant_id, campaign_id)
            .await
            .map_err(map_newsletter_error)?;

        let slugs: Vec<ContentSourceSlug> = campaign
            .content_sources
            .iter()
            .filter_map(|s| ContentSourceSlug::new(s).ok())
            .collect();

        registry
            .fetch_for_sources(tenant_id, &slugs, None, None, 10)
            .await
            .map_err(|err| PortError {
                kind: rustok_api::PortErrorKind::Unavailable,
                code: "newsletter.content_fetch_failed".to_string(),
                message: err.to_string(),
                retryable: true,
            })
    }
}

fn map_newsletter_error(error: crate::NewsletterError) -> PortError {
    use rustok_api::{PortError, PortErrorKind};

    match error {
        crate::NewsletterError::NotFound(msg) => PortError {
            kind: PortErrorKind::NotFound,
            code: "newsletter.not_found".to_string(),
            message: msg,
            retryable: false,
        },
        crate::NewsletterError::Validation(msg) => PortError {
            kind: PortErrorKind::Validation,
            code: "newsletter.validation".to_string(),
            message: msg,
            retryable: false,
        },
        crate::NewsletterError::DuplicateSubscriber(email) => PortError {
            kind: PortErrorKind::Conflict,
            code: "newsletter.duplicate_subscriber".to_string(),
            message: format!("subscriber already exists: {email}"),
            retryable: false,
        },
        crate::NewsletterError::InvalidCampaignStatus { current, required } => PortError {
            kind: PortErrorKind::Conflict,
            code: "newsletter.invalid_status".to_string(),
            message: format!("campaign status {current} invalid, required {required}"),
            retryable: false,
        },
        crate::NewsletterError::Conflict(msg) => PortError {
            kind: PortErrorKind::Conflict,
            code: "newsletter.conflict".to_string(),
            message: msg,
            retryable: false,
        },
        crate::NewsletterError::Delivery(msg) => PortError {
            kind: PortErrorKind::Unavailable,
            code: "newsletter.delivery".to_string(),
            message: msg,
            retryable: true,
        },
        crate::NewsletterError::Database(err) => PortError {
            kind: PortErrorKind::Unavailable,
            code: "newsletter.database".to_string(),
            message: err.to_string(),
            retryable: true,
        },
        crate::NewsletterError::Api(err) => PortError {
            kind: PortErrorKind::InvariantViolation,
            code: "newsletter.api".to_string(),
            message: err.to_string(),
            retryable: false,
        },
    }
}
