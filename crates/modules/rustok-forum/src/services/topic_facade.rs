use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use rustok_api::{Action, PortContext, Resource};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::audience::SharedForumAudienceFactsPort;
use crate::dto::{
    CreateTopicCommandInput, CreateTopicInput, ListTopicsFilter, TopicListItem, TopicResponse,
    UpdateTopicCommandInput, UpdateTopicInput,
};
use crate::entities::forum_topic;
use crate::error::{ForumError, ForumResult};
use crate::state_machine::TopicStatus;

use super::rbac::enforce_scope;
use super::topic_canonical_resolution::{
    ForumTopicCanonicalResolution, ForumTopicCanonicalResolutionService,
};
use super::topic_create_audience_authorization::ForumTopicCreateAudienceAuthorizationService;
use super::topic_owner;
use super::topic_route::{ForumTopicSlugRenameResult, RenameForumTopicSlugInput};
use super::topic_visibility::{ForumTopicVisibilityScope, ForumTopicVisibilityService};

/// Public topic owner facade.
///
/// The facade exposes only explicit domain operations. Persistence helpers stay
/// crate-private and the public type never dereferences into the raw service.
pub struct TopicService {
    db: DatabaseConnection,
    inner: topic_owner::TopicService,
    create_audience: ForumTopicCreateAudienceAuthorizationService,
}

impl TopicService {
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
            inner: topic_owner::TopicService::new(db.clone(), event_bus),
            create_audience: ForumTopicCreateAudienceAuthorizationService::new(
                db.clone(),
                facts_port,
            ),
            db,
        }
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateTopicInput,
    ) -> ForumResult<TopicResponse> {
        self.create_command_with_optional_audience_context(tenant_id, security, None, input.into())
            .await
    }

    pub async fn create_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        input: CreateTopicInput,
    ) -> ForumResult<TopicResponse> {
        self.create_command_with_optional_audience_context(
            tenant_id,
            security,
            Some(context),
            input.into(),
        )
        .await
    }

    pub async fn create_command(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateTopicCommandInput,
    ) -> ForumResult<TopicResponse> {
        self.create_command_with_optional_audience_context(tenant_id, security, None, input)
            .await
    }

    pub async fn create_command_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        input: CreateTopicCommandInput,
    ) -> ForumResult<TopicResponse> {
        self.create_command_with_optional_audience_context(
            tenant_id,
            security,
            Some(context),
            input,
        )
        .await
    }

    async fn create_command_with_optional_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: Option<PortContext>,
        input: CreateTopicCommandInput,
    ) -> ForumResult<TopicResponse> {
        self.create_audience
            .require(tenant_id, input.category_id, &security, context)
            .await?;
        let response = self
            .inner
            .create_command(tenant_id, security, input)
            .await?;
        require_localized_topic_response(response)
    }

    pub async fn resolve_canonical_topic(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
    ) -> ForumResult<ForumTopicCanonicalResolution> {
        self.resolve_canonical_topic_for_security(tenant_id, &security, topic_id)
            .await
    }

    async fn resolve_canonical_topic_for_security(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        topic_id: Uuid,
    ) -> ForumResult<ForumTopicCanonicalResolution> {
        enforce_scope(security, Resource::ForumTopics, Action::Read)?;
        let resolution = ForumTopicCanonicalResolutionService::new(self.db.clone())
            .resolve_unchecked(tenant_id, topic_id)
            .await?;
        let visible = ForumTopicVisibilityService::new(self.db.clone())
            .is_topic_category_visible_to_viewer(
                tenant_id,
                resolution.canonical_topic_id,
                !security.is_public_read(),
            )
            .await?;
        if !visible {
            return Err(ForumError::TopicNotFound(topic_id));
        }
        Ok(resolution)
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        locale: &str,
    ) -> ForumResult<TopicResponse> {
        self.get_with_locale_fallback(tenant_id, security, topic_id, locale, None)
            .await
    }

    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<TopicResponse> {
        self.get_with_canonical_resolution_and_locale_fallback(
            tenant_id,
            security,
            topic_id,
            locale,
            fallback_locale,
        )
        .await
        .map(|(_, topic)| topic)
    }

    pub async fn get_with_canonical_resolution_and_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(ForumTopicCanonicalResolution, TopicResponse)> {
        let resolution = self
            .resolve_canonical_topic_for_security(tenant_id, &security, topic_id)
            .await?;
        let response = self
            .inner
            .get_with_locale_fallback(
                tenant_id,
                security,
                resolution.canonical_topic_id,
                locale,
                fallback_locale,
            )
            .await?;
        Ok((resolution, require_localized_topic_response(response)?))
    }

    pub async fn get_storefront_visible_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> ForumResult<Option<TopicResponse>> {
        let scope = ForumTopicVisibilityScope::storefront_for_viewer(
            channel_slug,
            !security.is_public_read(),
        )?;
        let resolution = match self
            .resolve_canonical_topic_for_security(tenant_id, &security, topic_id)
            .await
        {
            Ok(resolution) => resolution,
            Err(ForumError::TopicNotFound(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
        let topic = match self
            .inner
            .get_with_locale_fallback(
                tenant_id,
                security,
                resolution.canonical_topic_id,
                locale,
                fallback_locale,
            )
            .await
        {
            Ok(topic) => require_localized_topic_response(topic)?,
            Err(ForumError::TopicNotFound(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
        if !ForumTopicVisibilityService::new(self.db.clone())
            .is_topic_visible(tenant_id, resolution.canonical_topic_id, &scope)
            .await?
        {
            return Ok(None);
        }
        Ok(Some(topic))
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

    pub async fn rename_slug(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: RenameForumTopicSlugInput,
    ) -> ForumResult<ForumTopicSlugRenameResult> {
        self.inner
            .rename_slug(tenant_id, topic_id, security, input)
            .await
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
        self.list_with_locale_fallback(tenant_id, security, filter, None)
            .await
    }

    pub async fn list_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        let visibility = ForumTopicVisibilityService::new(self.db.clone());
        let hidden_category_ids = visibility
            .hidden_category_ids_for_viewer(tenant_id, !security.is_public_read())
            .await?;
        let page = self
            .inner
            .list_with_locale_fallback_and_hidden_categories(
                tenant_id,
                security,
                filter,
                fallback_locale,
                &hidden_category_ids,
            )
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
        let scope = ForumTopicVisibilityScope::storefront_for_viewer(
            channel_slug,
            !security.is_public_read(),
        )?;
        let visibility = ForumTopicVisibilityService::new(self.db.clone());
        let hidden_category_ids = visibility
            .hidden_category_ids_for_scope(tenant_id, &scope)
            .await?;
        let page = self
            .inner
            .list_storefront_visible_with_locale_fallback_and_hidden_categories(
                tenant_id,
                security,
                filter,
                fallback_locale,
                scope.channel_slug(),
                &hidden_category_ids,
            )
            .await?;
        let candidate_ids = page.0.iter().map(|topic| topic.id).collect::<Vec<_>>();
        let visible_ids = visibility
            .filter_visible_topic_ids(tenant_id, &candidate_ids, &scope)
            .await?;
        if visible_ids != candidate_ids {
            return Err(ForumError::Internal(rustok_core::Error::External(
                "Forum storefront topic selection diverged from the owner visibility scope"
                    .to_string(),
            )));
        }
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
