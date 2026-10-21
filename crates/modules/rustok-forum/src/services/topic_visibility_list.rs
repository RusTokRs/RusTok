use super::pending_visibility::topic_owner_read_condition;

impl TopicService {
    #[instrument(skip(self, security, filter, hidden_category_ids))]
    pub(crate) async fn list_with_locale_fallback_and_hidden_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        mut filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
        hidden_category_ids: &[Uuid],
    ) -> ForumResult<TopicPage<TopicListItem>> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        filter.normalize_selection()?;
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
        select = select.filter(topic_owner_read_condition(&security));

        let scope = filter.cursor_scope(
            tenant_id,
            security.user_id,
            &locale,
            fallback_locale.as_deref(),
            None,
            false,
        )?;
        self.fetch_localized_topic_page(
            select,
            &filter,
            tenant_id,
            security.user_id,
            (&locale, fallback_locale.as_deref()),
            scope,
        )
        .await
    }

    #[instrument(skip(self, security, filter, hidden_category_ids))]
    pub(crate) async fn list_storefront_visible_with_locale_fallback_and_hidden_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        mut filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
        hidden_category_ids: &[Uuid],
    ) -> ForumResult<TopicPage<TopicListItem>> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        filter.normalize_selection()?;
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
        let show_locked_topics_in_lists = self
            .settings
            .module_settings(tenant_id)
            .await?
            .show_locked_topics_in_lists;
        if !show_locked_topics_in_lists {
            select = select.filter(forum_topic::Column::IsLocked.eq(false));
        }

        let cursor_channel = normalize_public_channel_slug(channel_slug);
        let scope = filter.cursor_scope(
            tenant_id,
            security.user_id,
            &locale,
            fallback_locale.as_deref(),
            cursor_channel.as_deref(),
            true,
        )?;
        self.fetch_localized_topic_page(
            select,
            &filter,
            tenant_id,
            security.user_id,
            (&locale, fallback_locale.as_deref()),
            scope,
        )
        .await
    }

    /// Select over hydrated owner-localized items, not a browser page or an arbitrary translation.
    async fn fetch_localized_topic_page(
        &self,
        select: Select<forum_topic::Entity>,
        filter: &ListTopicsFilter,
        tenant_id: Uuid,
        viewer_id: Option<Uuid>,
        locale: (&str, Option<&str>),
        scope: [u8; 32],
    ) -> ForumResult<TopicPage<TopicListItem>> {
        if filter.search.is_none() && !filter.solved_only {
            let page = self.fetch_topic_keyset_page(select, filter, scope).await?;
            let mut items = self
                .hydrate_topic_list_items(tenant_id, viewer_id, page.items, locale.0, locale.1)
                .await?;
            for topic in &mut items {
                if let Some(key) = topic.sort_key.as_mut() {
                    key.order = filter.order;
                    key.selection_scope = Some(scope);
                }
            }
            return Ok(TopicPage {
                items,
                next_cursor: page.next_cursor,
            });
        }
        let mut candidate = filter.clone();
        candidate.per_page = crate::dto::MAX_FORUM_READ_LIMIT;
        let mut items: Vec<TopicListItem> = Vec::new();
        loop {
            let page = self
                .fetch_topic_keyset_page(select.clone(), &candidate, scope)
                .await?;
            let hydrated = self
                .hydrate_topic_list_items(tenant_id, viewer_id, page.items, locale.0, locale.1)
                .await?;
            for mut topic in hydrated {
                if !filter.matches_selection(&topic) {
                    continue;
                }
                if items.len() as u64 == filter.per_page.max(1) {
                    let next_cursor = items
                        .last()
                        .and_then(|last| last.sort_key)
                        .map(|key| key.encode());
                    return Ok(TopicPage { items, next_cursor });
                }
                if let Some(key) = topic.sort_key.as_mut() {
                    key.order = filter.order;
                    key.selection_scope = Some(scope);
                }
                items.push(topic);
            }
            match page.next_cursor {
                Some(after) => candidate.after = Some(after),
                None => {
                    return Ok(TopicPage {
                        items,
                        next_cursor: None,
                    });
                }
            }
        }
    }

    /// Keyset page of topics ordered `is_pinned DESC, last_reply_at DESC NULLS LAST,
    /// updated_at DESC, id DESC`. Fetches one extra row to learn whether another page
    /// exists, so the list never counts rows.
    async fn fetch_topic_keyset_page(
        &self,
        mut select: Select<forum_topic::Entity>,
        filter: &ListTopicsFilter,
        scope: [u8; 32],
    ) -> ForumResult<TopicPage<forum_topic::Model>> {
        let per_page = filter.per_page.max(1);
        if let Some(raw) = filter.after.as_deref() {
            let after = TopicListCursor::decode(raw)?;
            if after.order != filter.order || after.selection_scope != Some(scope) {
                return Err(ForumError::Validation(
                    "Forum topic cursor does not match this selection".into(),
                ));
            }
            let condition = if filter.order == crate::ForumTopicListOrder::Top {
                Condition::any()
                    .add(forum_topic::Column::ReplyCount.lt(after.reply_count))
                    .add(
                        Condition::all()
                            .add(forum_topic::Column::ReplyCount.eq(after.reply_count))
                            .add(topic_list_after_condition(&after)),
                    )
            } else {
                topic_list_after_condition(&after)
            };
            select = select.filter(condition);
        }

        if filter.order == crate::ForumTopicListOrder::Top {
            select = select.order_by_desc(forum_topic::Column::ReplyCount);
        }
        let mut topics = order_topic_list(select)
            .limit(per_page + 1)
            .all(&self.db)
            .await?;

        let has_next_page = topics.len() as u64 > per_page;
        topics.truncate(per_page as usize);
        let next_cursor = if has_next_page {
            let last = topics.last().ok_or_else(|| {
                ForumError::Validation("Topic page reported a next page without rows".to_string())
            })?;
            let mut key = topic_list_cursor(last);
            key.order = filter.order;
            key.selection_scope = Some(scope);
            Some(key.encode())
        } else {
            None
        };
        Ok(TopicPage {
            items: topics,
            next_cursor,
        })
    }
}

/// The one ordering of every topic list: `is_pinned DESC, last_reply_at DESC NULLS LAST,
/// updated_at DESC, id DESC`. It is expressed as a column order with explicit NULL
/// placement, not as an `IS NULL` expression, so `idx_forum_topics_list_keyset` can
/// serve it. Read-model and unread projections order through this function too.
pub(crate) fn order_topic_list(select: Select<forum_topic::Entity>) -> Select<forum_topic::Entity> {
    select
        .order_by_desc(forum_topic::Column::IsPinned)
        .order_by_with_nulls(
            forum_topic::Column::LastReplyAt,
            Order::Desc,
            NullOrdering::Last,
        )
        .order_by_desc(forum_topic::Column::UpdatedAt)
        .order_by_desc(forum_topic::Column::Id)
}

/// Cursor that resumes the list directly after `topic` in `order_topic_list`.
pub(crate) fn topic_list_cursor(topic: &forum_topic::Model) -> TopicListCursor {
    TopicListCursor {
        is_pinned: topic.is_pinned,
        last_reply_at: topic.last_reply_at,
        updated_at: topic.updated_at,
        id: topic.id,
        reply_count: topic.reply_count,
        order: Default::default(),
        selection_scope: None,
    }
}

/// Rows strictly after `after` in `order_topic_list`, expanded lexicographically over
/// the four sort keys.
pub(crate) fn topic_list_after_condition(after: &TopicListCursor) -> Condition {
    let pinned_eq = forum_topic::Column::IsPinned.eq(after.is_pinned);
    let last_reply_eq = match after.last_reply_at {
        Some(value) => forum_topic::Column::LastReplyAt.eq(value),
        None => forum_topic::Column::LastReplyAt.is_null(),
    };

    let mut condition = Condition::any().add(forum_topic::Column::IsPinned.lt(after.is_pinned));
    if let Some(last_reply_at) = after.last_reply_at {
        // NULLs sort last, so after a non-NULL value come smaller values and every NULL.
        condition = condition.add(
            Condition::all().add(pinned_eq.clone()).add(
                Condition::any()
                    .add(forum_topic::Column::LastReplyAt.lt(last_reply_at))
                    .add(forum_topic::Column::LastReplyAt.is_null()),
            ),
        );
    }
    // After a NULL `last_reply_at` nothing follows within the same pinned group except
    // rows that are also NULL, which the two branches below already cover.
    condition = condition.add(
        Condition::all()
            .add(pinned_eq.clone())
            .add(last_reply_eq.clone())
            .add(forum_topic::Column::UpdatedAt.lt(after.updated_at)),
    );
    condition.add(
        Condition::all()
            .add(pinned_eq)
            .add(last_reply_eq)
            .add(forum_topic::Column::UpdatedAt.eq(after.updated_at))
            .add(forum_topic::Column::Id.lt(after.id)),
    )
}

fn apply_tenant_scoped_storefront_channel_filter(
    select: Select<forum_topic::Entity>,
    tenant_id: Uuid,
    channel_slug: Option<&str>,
) -> Select<forum_topic::Entity> {
    let unrestricted =
        forum_topic::Column::Id.not_in_subquery(tenant_topic_channel_access_subquery(tenant_id));
    let condition = match normalize_public_channel_slug(channel_slug) {
        Some(channel_slug) => {
            Condition::any()
                .add(unrestricted)
                .add(forum_topic::Column::Id.in_subquery(
                    matching_tenant_topic_channel_access_subquery(tenant_id, &channel_slug),
                ))
        }
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

fn normalize_public_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}
