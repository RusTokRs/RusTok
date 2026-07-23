use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    CreateReplyCommandInput, CreateReplyInput, ListRepliesFilter, ReplyListItem, ReplyResponse,
    UpdateReplyCommandInput, UpdateReplyInput,
};
use crate::entities::forum_reply;
use crate::error::{ForumError, ForumResult};
use crate::state_machine::ReplyStatus;

use super::reply_owner;

/// Public reply owner facade.
///
/// The facade exposes only explicit domain operations. Persistence helpers stay
/// crate-private and the public type never dereferences into the raw service.
pub struct ReplyService {
    inner: reply_owner::ReplyService,
}

impl ReplyService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: reply_owner::ReplyService::new(db, event_bus),
        }
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        input: CreateReplyInput,
    ) -> ForumResult<ReplyResponse> {
        self.create_command(tenant_id, security, topic_id, input.into())
            .await
    }

    pub async fn create_command(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        input: CreateReplyCommandInput,
    ) -> ForumResult<ReplyResponse> {
        let response = self
            .inner
            .create_command(tenant_id, security, topic_id, input)
            .await?;
        require_localized_reply_response(response)
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        reply_id: Uuid,
        locale: &str,
    ) -> ForumResult<ReplyResponse> {
        let response = self.inner.get(tenant_id, security, reply_id, locale).await?;
        require_localized_reply_response(response)
    }

    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        reply_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ReplyResponse> {
        let response = self
            .inner
            .get_with_locale_fallback(tenant_id, security, reply_id, locale, fallback_locale)
            .await?;
        require_localized_reply_response(response)
    }

    pub async fn update(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        input: UpdateReplyInput,
    ) -> ForumResult<ReplyResponse> {
        self.update_command(tenant_id, reply_id, security, input.into())
            .await
    }

    pub async fn update_command(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        input: UpdateReplyCommandInput,
    ) -> ForumResult<ReplyResponse> {
        let response = self
            .inner
            .update_command(tenant_id, reply_id, security, input)
            .await?;
        require_localized_reply_response(response)
    }

    pub async fn delete(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner.delete(tenant_id, reply_id, security).await
    }

    pub async fn list_for_topic(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        filter: ListRepliesFilter,
    ) -> ForumResult<(Vec<ReplyListItem>, u64)> {
        let page = self
            .inner
            .list_for_topic(tenant_id, security, topic_id, filter)
            .await?;
        require_localized_reply_list_page(page)
    }

    pub async fn list_for_topic_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<ReplyListItem>, u64)> {
        let page = self
            .inner
            .list_for_topic_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                filter,
                fallback_locale,
            )
            .await?;
        require_localized_reply_list_page(page)
    }

    pub async fn list_response_for_topic_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<ReplyResponse>, u64)> {
        let page = self
            .inner
            .list_response_for_topic_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                filter,
                fallback_locale,
            )
            .await?;
        require_localized_reply_response_page(page)
    }

    pub async fn list_response_for_topic_by_statuses_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
        statuses: Option<&[ReplyStatus]>,
    ) -> ForumResult<(Vec<ReplyResponse>, u64)> {
        let page = self
            .inner
            .list_response_for_topic_by_statuses_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                filter,
                fallback_locale,
                statuses,
            )
            .await?;
        require_localized_reply_response_page(page)
    }

    pub(crate) async fn find_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        self.inner.find_reply(tenant_id, reply_id).await
    }

    pub(crate) async fn find_reply_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        reply_owner::ReplyService::find_reply_in_tx(txn, tenant_id, reply_id).await
    }

    pub(crate) async fn set_status_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
        status: ReplyStatus,
    ) -> ForumResult<forum_reply::Model> {
        reply_owner::ReplyService::set_status_in_tx(txn, tenant_id, reply_id, status).await
    }
}

fn require_localized_reply_response(response: ReplyResponse) -> ForumResult<ReplyResponse> {
    if response.content.is_empty() {
        return Err(ForumError::Validation(format!(
            "Reply {} has no localized body",
            response.id
        )));
    }
    Ok(response)
}

fn require_localized_reply_list_page(
    page: (Vec<ReplyListItem>, u64),
) -> ForumResult<(Vec<ReplyListItem>, u64)> {
    let (items, total) = page;
    if let Some(item) = items.iter().find(|item| item.content_preview.is_empty()) {
        return Err(ForumError::Validation(format!(
            "Reply {} has no localized body",
            item.id
        )));
    }
    Ok((items, total))
}

fn require_localized_reply_response_page(
    page: (Vec<ReplyResponse>, u64),
) -> ForumResult<(Vec<ReplyResponse>, u64)> {
    let (items, total) = page;
    if let Some(item) = items.iter().find(|item| item.content.is_empty()) {
        return Err(ForumError::Validation(format!(
            "Reply {} has no localized body",
            item.id
        )));
    }
    Ok((items, total))
}
