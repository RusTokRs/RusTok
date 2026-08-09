use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use rustok_api::{Action, PortContext, Resource};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::audience::SharedForumAudienceFactsPort;
use crate::dto::{
    CreateReplyCommandInput, CreateReplyInput, ListRepliesFilter, ReplyListItem, ReplyResponse,
    UpdateReplyCommandInput, UpdateReplyInput,
};
use crate::entities::forum_reply;
use crate::error::{ForumError, ForumResult};
use crate::state_machine::ReplyStatus;

use super::rbac::enforce_scope;
use super::reply_create_audience_authorization::ForumReplyCreateAudienceAuthorizationService;
use super::reply_owner;
use super::topic_visibility::ForumTopicVisibilityService;

/// Public reply owner facade.
///
/// The facade exposes only explicit domain operations. Persistence helpers stay
/// crate-private and the public type never dereferences into the raw service.
pub struct ReplyService {
    db: DatabaseConnection,
    inner: reply_owner::ReplyService,
    create_audience: ForumReplyCreateAudienceAuthorizationService,
}

impl ReplyService {
    pub const MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS: usize =
        reply_owner::ReplyService::MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS;

    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self::with_optional_audience_facts(db, event_bus, None)
    }

    pub fn with_audience_facts(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        facts_port: SharedForumAudienceFactsPort,
    ) -> Self {
        Self::with_optional_audience_facts(db, event_bus, Some(facts_port))
    }

    fn with_optional_audience_facts(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        facts_port: Option<SharedForumAudienceFactsPort>,
    ) -> Self {
        Self {
            inner: reply_owner::ReplyService::new(db.clone(), event_bus),
            create_audience: ForumReplyCreateAudienceAuthorizationService::new(
                db.clone(),
                facts_port,
            ),
            db,
        }
    }

    pub async fn available_locales_for_replies(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        reply_ids: &[Uuid],
    ) -> ForumResult<Vec<(Uuid, Vec<String>)>> {
        enforce_scope(&security, Resource::ForumReplies, Action::List)?;
        if security.is_public_read() {
            return Err(ForumError::forbidden(
                "Forum reply locale enumeration requires an authenticated operator context",
            ));
        }
        self.inner
            .available_locales_for_replies(tenant_id, security, reply_ids)
            .await
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        input: CreateReplyInput,
    ) -> ForumResult<ReplyResponse> {
        self.create_command_with_optional_audience_context(
            tenant_id,
            security,
            topic_id,
            None,
            input.into(),
        )
        .await
    }

    pub async fn create_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        context: PortContext,
        input: CreateReplyInput,
    ) -> ForumResult<ReplyResponse> {
        self.create_command_with_optional_audience_context(
            tenant_id,
            security,
            topic_id,
            Some(context),
            input.into(),
        )
        .await
    }

    pub async fn create_command(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        input: CreateReplyCommandInput,
    ) -> ForumResult<ReplyResponse> {
        self.create_command_with_optional_audience_context(
            tenant_id, security, topic_id, None, input,
        )
        .await
    }

    pub async fn create_command_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        context: PortContext,
        input: CreateReplyCommandInput,
    ) -> ForumResult<ReplyResponse> {
        self.create_command_with_optional_audience_context(
            tenant_id,
            security,
            topic_id,
            Some(context),
            input,
        )
        .await
    }

    async fn create_command_with_optional_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        context: Option<PortContext>,
        input: CreateReplyCommandInput,
    ) -> ForumResult<ReplyResponse> {
        self.create_audience
            .require(tenant_id, topic_id, &security, context)
            .await?;
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
        self.get_with_locale_fallback(tenant_id, security, reply_id, locale, None)
            .await
    }

    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        reply_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ReplyResponse> {
        enforce_scope(&security, Resource::ForumReplies, Action::Read)?;
        let reply = self.inner.find_reply(tenant_id, reply_id).await?;
        if !self
            .topic_category_is_visible(tenant_id, reply.topic_id, &security)
            .await?
        {
            return Err(ForumError::ReplyNotFound(reply_id));
        }
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
        self.list_for_topic_with_locale_fallback(tenant_id, security, topic_id, filter, None)
            .await
    }

    pub async fn list_for_topic_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<ReplyListItem>, u64)> {
        enforce_scope(&security, Resource::ForumReplies, Action::List)?;
        if !self
            .topic_category_is_visible(tenant_id, topic_id, &security)
            .await?
        {
            return Err(ForumError::TopicNotFound(topic_id));
        }
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
        self.list_response_for_topic_by_statuses_with_locale_fallback(
            tenant_id,
            security,
            topic_id,
            filter,
            fallback_locale,
            None,
        )
        .await
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
        enforce_scope(&security, Resource::ForumReplies, Action::List)?;
        if !self
            .topic_category_is_visible(tenant_id, topic_id, &security)
            .await?
        {
            return Err(ForumError::TopicNotFound(topic_id));
        }
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

    async fn topic_category_is_visible(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: &SecurityContext,
    ) -> ForumResult<bool> {
        if !security.is_public_read() {
            return Ok(true);
        }
        ForumTopicVisibilityService::new(self.db.clone())
            .is_topic_category_visible_to_viewer(tenant_id, topic_id, false)
            .await
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

    pub(crate) async fn remove_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<reply_owner::ReplyRemovalOutcome> {
        reply_owner::ReplyService::remove_in_tx(txn, tenant_id, reply_id).await
    }
}

fn require_localized_reply_response(response: ReplyResponse) -> ForumResult<ReplyResponse> {
    if response.content.document.content.is_empty() {
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
    if let Some(item) = items
        .iter()
        .find(|item| item.content.document.content.is_empty())
    {
        return Err(ForumError::Validation(format!(
            "Reply {} has no localized body",
            item.id
        )));
    }
    Ok((items, total))
}
