use chrono::Utc;
use rustok_newsletter_api::SubscriberStatus;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    ModelTrait, QueryFilter, QueryOrder, QuerySelect,
};
use tracing::instrument;
use uuid::Uuid;

use crate::domain::SubscriberLifecycle;
use crate::dto::{
    SubscribeInput, SubscriberListQuery, SubscriberListResponse, SubscriberResponse,
    SubscriberSummary, UpdateSubscriberInput,
};
use crate::entities::subscriber::{self, Entity as Subscriber};
use crate::error::{NewsletterError, NewsletterResult};

/// Application service for newsletter subscriber management.
pub struct SubscriberService {
    db: DatabaseConnection,
}

impl SubscriberService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Subscribe a new email address. Creates a pending subscriber.
    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, email = %input.email))]
    pub async fn subscribe(
        &self,
        tenant_id: Uuid,
        input: SubscribeInput,
    ) -> NewsletterResult<SubscriberResponse> {
        let email = input.email.trim().to_lowercase();
        if email.is_empty() || !email.contains('@') {
            return Err(NewsletterError::validation("Invalid email address"));
        }

        // Check for existing subscriber
        let existing = Subscriber::find()
            .filter(subscriber::Column::TenantId.eq(tenant_id))
            .filter(subscriber::Column::Email.eq(&email))
            .one(&self.db)
            .await?;

        if existing.is_some() {
            return Err(NewsletterError::DuplicateSubscriber(email));
        }

        let now = Utc::now();
        let confirm_token = generate_confirm_token();

        let model = subscriber::ActiveModel {
            id: Set(Uuid::now_v7()),
            tenant_id: Set(tenant_id),
            email: Set(email.clone()),
            name: Set(input.name),
            status: Set("pending".to_string()),
            locale: Set(input.locale),
            confirm_token: Set(Some(confirm_token)),
            subscribed_at: Set(now.into()),
            confirmed_at: Set(None),
            unsubscribed_at: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let result = model.insert(&self.db).await?;

        Ok(SubscriberResponse {
            id: result.id,
            email: result.email,
            name: result.name,
            status: SubscriberStatus::Pending,
            locale: result.locale,
            subscribed_at: result.subscribed_at.into(),
            confirmed_at: None,
            unsubscribed_at: None,
        })
    }

    /// Confirm a pending subscription.
    #[instrument(skip(self), fields(tenant_id = %tenant_id, subscriber_id = %subscriber_id))]
    pub async fn confirm(
        &self,
        tenant_id: Uuid,
        subscriber_id: Uuid,
    ) -> NewsletterResult<()> {
        let subscriber = self.find_subscriber(tenant_id, subscriber_id).await?;
        let current = parse_status(&subscriber.status)?;

        if !SubscriberLifecycle::can_transition(current, SubscriberStatus::Active) {
            return Err(NewsletterError::validation(format!(
                "Cannot confirm subscriber in {:?} status",
                current
            )));
        }

        let now = Utc::now();
        let mut active_model: subscriber::ActiveModel = subscriber.into();
        active_model.status = Set("active".to_string());
        active_model.confirmed_at = Set(Some(now.into()));
        active_model.confirm_token = Set(None);
        active_model.updated_at = Set(now.into());
        active_model.update(&self.db).await?;

        Ok(())
    }

    /// Unsubscribe an active or pending subscriber.
    #[instrument(skip(self), fields(tenant_id = %tenant_id, subscriber_id = %subscriber_id))]
    pub async fn unsubscribe(
        &self,
        tenant_id: Uuid,
        subscriber_id: Uuid,
    ) -> NewsletterResult<()> {
        let subscriber = self.find_subscriber(tenant_id, subscriber_id).await?;
        let current = parse_status(&subscriber.status)?;

        if !SubscriberLifecycle::can_transition(current, SubscriberStatus::Unsubscribed) {
            return Err(NewsletterError::validation(format!(
                "Cannot unsubscribe subscriber in {:?} status",
                current
            )));
        }

        let now = Utc::now();
        let mut active_model: subscriber::ActiveModel = subscriber.into();
        active_model.status = Set("unsubscribed".to_string());
        active_model.unsubscribed_at = Set(Some(now.into()));
        active_model.updated_at = Set(now.into());
        active_model.update(&self.db).await?;

        Ok(())
    }

    /// Update subscriber metadata (name, locale).
    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, subscriber_id = %subscriber_id))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        subscriber_id: Uuid,
        input: UpdateSubscriberInput,
    ) -> NewsletterResult<SubscriberResponse> {
        let subscriber = self.find_subscriber(tenant_id, subscriber_id).await?;
        let now = Utc::now();

        let mut active_model: subscriber::ActiveModel = subscriber.clone().into();
        if let Some(name) = input.name {
            active_model.name = Set(Some(name));
        }
        if let Some(locale) = input.locale {
            active_model.locale = Set(Some(locale));
        }
        active_model.updated_at = Set(now.into());
        let result = active_model.update(&self.db).await?;

        self.to_response(result)
    }

    /// List subscribers with pagination and filtering.
    pub async fn list(
        &self,
        tenant_id: Uuid,
        query: SubscriberListQuery,
    ) -> NewsletterResult<SubscriberListResponse> {
        use sea_orm::PaginatorTrait;

        let page = query.page.unwrap_or(1).max(1);
        let per_page = query.per_page.unwrap_or(25).min(100).max(1);

        let mut find = Subscriber::find()
            .filter(subscriber::Column::TenantId.eq(tenant_id));

        if let Some(status) = query.status {
            find = find.filter(subscriber::Column::Status.eq(status_to_string(status)));
        }

        if let Some(search) = &query.search {
            let search_pattern = format!("%{}%", search);
            find = find.filter(
                Condition::any()
                    .add(subscriber::Column::Email.like(&search_pattern))
                    .add(subscriber::Column::Name.like(&search_pattern)),
            );
        }

        let total = find.clone().count(&self.db).await?;

        let items: Vec<SubscriberSummary> = find
            .order_by_desc(subscriber::Column::SubscribedAt)
            .offset(Some((page - 1) * per_page))
            .limit(Some(per_page))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|s| SubscriberSummary {
                id: s.id,
                email: s.email,
                name: s.name,
                status: parse_status(&s.status).unwrap_or(SubscriberStatus::Pending),
                subscribed_at: s.subscribed_at.into(),
            })
            .collect();

        Ok(SubscriberListResponse {
            items,
            total,
            page,
            per_page,
        })
    }

    /// Get a single subscriber by ID.
    pub async fn get(
        &self,
        tenant_id: Uuid,
        subscriber_id: Uuid,
    ) -> NewsletterResult<SubscriberResponse> {
        let subscriber = self.find_subscriber(tenant_id, subscriber_id).await?;
        self.to_response(subscriber)
    }

    /// Find a subscriber by email within a tenant.
    pub async fn find_by_email(
        &self,
        tenant_id: Uuid,
        email: &str,
    ) -> NewsletterResult<Option<SubscriberResponse>> {
        let email = email.trim().to_lowercase();
        let subscriber = Subscriber::find()
            .filter(subscriber::Column::TenantId.eq(tenant_id))
            .filter(subscriber::Column::Email.eq(&email))
            .one(&self.db)
            .await?;
        subscriber.map(|s| self.to_response(s)).transpose()
    }

    /// Delete a subscriber permanently.
    #[instrument(skip(self), fields(tenant_id = %tenant_id, subscriber_id = %subscriber_id))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        subscriber_id: Uuid,
    ) -> NewsletterResult<()> {
        let subscriber = self.find_subscriber(tenant_id, subscriber_id).await?;
        subscriber.delete(&self.db).await?;
        Ok(())
    }

    // --- private helpers ---

    async fn find_subscriber(
        &self,
        tenant_id: Uuid,
        subscriber_id: Uuid,
    ) -> NewsletterResult<subscriber::Model> {
        Subscriber::find_by_id(subscriber_id)
            .filter(subscriber::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| NewsletterError::not_found(format!("subscriber {subscriber_id}")))
    }

    fn to_response(&self, model: subscriber::Model) -> NewsletterResult<SubscriberResponse> {
        Ok(SubscriberResponse {
            id: model.id,
            email: model.email,
            name: model.name,
            status: parse_status(&model.status)?,
            locale: model.locale,
            subscribed_at: model.subscribed_at.into(),
            confirmed_at: model.confirmed_at.map(Into::into),
            unsubscribed_at: model.unsubscribed_at.map(Into::into),
        })
    }
}

fn parse_status(value: &str) -> NewsletterResult<SubscriberStatus> {
    value
        .parse()
        .map_err(|e: rustok_newsletter_api::NewsletterApiError| {
            NewsletterError::validation(e.to_string())
        })
}

fn status_to_string(status: SubscriberStatus) -> String {
    status.as_str().to_string()
}

fn generate_confirm_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}{}", timestamp, Uuid::now_v7().to_string().replace('-', ""))
        .chars()
        .take(64)
        .collect()
}
