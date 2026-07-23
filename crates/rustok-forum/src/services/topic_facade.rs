use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    CreateTopicCommandInput, CreateTopicInput, ListTopicsFilter, TopicListItem, TopicResponse,
    UpdateTopicCommandInput, UpdateTopicInput,
};
use crate::entities::forum_topic;
use crate::error::{ForumError, ForumResult};
use crate::state_machine::TopicStatus;

use super::topic_owner;

/// Public topic owner facade.
///
/// The facade exposes only explicit domain operations. Persistence helpers stay
/// crate-private and the public type never dereferences into the raw service.
pub struct TopicService {
    inner: topic_owner::TopicService,
}

impl TopicService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: topic_owner::TopicService::new(db, event_bus),
        }
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateTopicInput,
    ) -> ForumResult<TopicResponse> {
        self.create_command(tenant_id, security, input.into()).await
    }

    pub async fn create_command(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateTopicCommandInput,
    ) -> ForumResult<TopicResponse> {
        let response = self
            .inner
            .create_command(tenant_id, security, input)
            .await?;
        require_localized_topic_response(response)
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        locale: &str,
    ) -> ForumResult<TopicResponse> {
        let response = self.inner.get(tenant_id, security, topic_id, locale).await?;
        require_localized_topic_response(response)
    }

    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<TopicResponse> {
        let response = self
            .inner
            .get_with_locale_fallback(tenant_id, security, topic_id, locale, fallback_locale)
            .await?;
        require_localized_topic_response(response)
    }

    pub async fn update(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: UpdateTopicInput,
    ) -> ForumResult<TopicResponse> {
        self.update_command(tenant_id, topic_id, security, input.into())
            .await
    }

    pub async fn update_command(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: UpdateTopicCommandInput,
    ) -> ForumResult<TopicResponse> {
        let response = self
            .inner
            .update_command(tenant_id, topic_id, security, input)
            .await?;
        require_localized_topic_response(response)
    }

    pub async fn delete(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner.delete(tenant_id, topic_id, security).await
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListTopicsFilter,
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        let page = self.inner.list(tenant_id, security, filter).await?;
        require_localized_topic_page(page)
    }

    pub async fn list_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        let page = self
            .inner
            .list_with_locale_fallback(tenant_id, security, filter, fallback_locale)
            .await?;
        require_localized_topic_page(page)
    }

    pub async fn list_storefront_visible_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        let page = self
            .inner
            .list_storefront_visible_with_locale_fallback(
                tenant_id,
                security,
                filter,
                fallback_locale,
                channel_slug,
            )
            .await?;
        require_localized_topic_page(page)
    }

    pub(crate) async fn find_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<forum_topic::Model> {
        self.inner.find_topic(tenant_id, topic_id).await
    }

    pub(crate) async fn find_topic_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<forum_topic::Model> {
        topic_owner::TopicService::find_topic_in_tx(txn, tenant_id, topic_id).await
    }

    pub(crate) async fn adjust_reply_count_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        delta: i32,
    ) -> ForumResult<forum_topic::Model> {
        topic_owner::TopicService::adjust_reply_count_in_tx(txn, tenant_id, topic_id, delta).await
    }

    pub(crate) async fn set_pinned_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        is_pinned: bool,
    ) -> ForumResult<()> {
        topic_owner::TopicService::set_pinned_in_tx(txn, tenant_id, topic_id, is_pinned).await
    }

    pub(crate) async fn set_locked_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        is_locked: bool,
    ) -> ForumResult<()> {
        topic_owner::TopicService::set_locked_in_tx(txn, tenant_id, topic_id, is_locked).await
    }

    pub(crate) async fn set_status_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        status: TopicStatus,
    ) -> ForumResult<()> {
        topic_owner::TopicService::set_status_in_tx(txn, tenant_id, topic_id, status).await
    }
}

fn require_localized_topic_response(response: TopicResponse) -> ForumResult<TopicResponse> {
    if response.available_locales.is_empty() || response.title.is_empty() {
        return Err(ForumError::Validation(format!(
            "Topic {} has no localized translation",
            response.id
        )));
    }
    Ok(response)
}

fn require_localized_topic_page(
    page: (Vec<TopicListItem>, u64),
) -> ForumResult<(Vec<TopicListItem>, u64)> {
    let (items, total) = page;
    if let Some(item) = items
        .iter()
        .find(|item| item.available_locales.is_empty() || item.title.is_empty())
    {
        return Err(ForumError::Validation(format!(
            "Topic {} has no localized translation",
            item.id
        )));
    }
    Ok((items, total))
}
