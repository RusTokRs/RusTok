impl TopicService {
    #[instrument(skip(self, security, hidden_category_ids))]
    pub(crate) async fn list_with_locale_fallback_and_hidden_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
        hidden_category_ids: &[Uuid],
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        let locale = filter
            .locale
            .clone()
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

        let mut select =
            forum_topic::Entity::find().filter(forum_topic::Column::TenantId.eq(tenant_id));
        if let Some(category_id) = filter.category_id {
            select = select.filter(forum_topic::Column::CategoryId.eq(category_id));
        }
        if let Some(status) = filter.status {
            select = select.filter(forum_topic::Column::Status.eq(status));
        }
        if !hidden_category_ids.is_empty() {
            select = select
                .filter(forum_topic::Column::CategoryId.is_not_in(hidden_category_ids.to_vec()));
        }

        let paginator = select
            .order_by_desc(forum_topic::Column::IsPinned)
            .order_by_desc(forum_topic::Column::LastReplyAt)
            .order_by_desc(forum_topic::Column::UpdatedAt)
            .paginate(&self.db, filter.per_page.max(1));
        let total = paginator.num_items().await?;
        let topics = paginator.fetch_page(filter.page.saturating_sub(1)).await?;
        let items = self
            .hydrate_topic_list_items(
                tenant_id,
                security.user_id,
                topics,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?;

        Ok((items, total))
    }

    #[instrument(skip(self, security, hidden_category_ids))]
    pub(crate) async fn list_storefront_visible_with_locale_fallback_and_hidden_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
        hidden_category_ids: &[Uuid],
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        let locale = filter
            .locale
            .clone()
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

        let mut select = forum_topic::Entity::find()
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .filter(forum_topic::Column::Status.eq(TopicStatus::Open));
        if let Some(category_id) = filter.category_id {
            select = select.filter(forum_topic::Column::CategoryId.eq(category_id));
        }
        if !hidden_category_ids.is_empty() {
            select = select
                .filter(forum_topic::Column::CategoryId.is_not_in(hidden_category_ids.to_vec()));
        }
        select = apply_tenant_scoped_storefront_channel_filter(select, tenant_id, channel_slug);

        let paginator = select
            .order_by_desc(forum_topic::Column::IsPinned)
            .order_by_desc(forum_topic::Column::LastReplyAt)
            .order_by_desc(forum_topic::Column::UpdatedAt)
            .paginate(&self.db, filter.per_page.max(1));
        let total = paginator.num_items().await?;
        let topics = paginator.fetch_page(filter.page.saturating_sub(1)).await?;
        let items = self
            .hydrate_topic_list_items(
                tenant_id,
                security.user_id,
                topics,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?;

        Ok((items, total))
    }
}

fn apply_tenant_scoped_storefront_channel_filter(
    select: Select<forum_topic::Entity>,
    tenant_id: Uuid,
    channel_slug: Option<&str>,
) -> Select<forum_topic::Entity> {
    let unrestricted = forum_topic::Column::Id
        .not_in_subquery(tenant_topic_channel_access_subquery(tenant_id));
    let condition = match normalize_public_channel_slug(channel_slug) {
        Some(channel_slug) => Condition::any().add(unrestricted).add(
            forum_topic::Column::Id.in_subquery(
                matching_tenant_topic_channel_access_subquery(tenant_id, &channel_slug),
            ),
        ),
        None => Condition::all().add(unrestricted),
    };

    select.filter(condition)
}

fn tenant_topic_channel_access_subquery(tenant_id: Uuid) -> SelectStatement {
    Query::select()
        .column(forum_topic_channel_access::Column::TopicId)
        .from(forum_topic_channel_access::Entity)
        .and_where(forum_topic_channel_access::Column::TenantId.eq(tenant_id))
        .to_owned()
}

fn matching_tenant_topic_channel_access_subquery(
    tenant_id: Uuid,
    channel_slug: &str,
) -> SelectStatement {
    Query::select()
        .column(forum_topic_channel_access::Column::TopicId)
        .from(forum_topic_channel_access::Entity)
        .and_where(forum_topic_channel_access::Column::TenantId.eq(tenant_id))
        .and_where(forum_topic_channel_access::Column::ChannelSlug.eq(channel_slug))
        .to_owned()
}
