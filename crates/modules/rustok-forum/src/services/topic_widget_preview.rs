/// Upper bound on candidate topics that one widget list preview may visit.
///
/// The exact `total` and the requested page both depend on the owner audience of every
/// candidate in the ordered widget query, so the scan is linear in the candidate count. A
/// request over this bound fails with a validation error instead of returning a partial total.
/// Callers narrow the preview with `category_id`.
pub(crate) const FORUM_WIDGET_TOPIC_LIST_MAX_CANDIDATES: u64 = 1_000;
const FORUM_WIDGET_TOPIC_LIST_SCAN_BATCH: u64 = 100;

/// Typed widget list query after props validation.
pub(crate) struct WidgetTopicListQuery<'a> {
    pub category_id: Option<Uuid>,
    pub page: u64,
    pub per_page: u64,
    pub include_pinned: bool,
    pub sort: &'a str,
}

impl TopicService {
    /// Ordered widget candidate query. Ordering is applied before pagination; the id tiebreaker
    /// keeps offsets stable across scan batches.
    fn widget_topic_candidates(
        tenant_id: Uuid,
        category_id: Option<Uuid>,
        include_pinned: bool,
        sort: &str,
    ) -> ForumResult<Select<forum_topic::Entity>> {
        let mut select =
            forum_topic::Entity::find().filter(forum_topic::Column::TenantId.eq(tenant_id));
        if let Some(category_id) = category_id {
            select = select.filter(forum_topic::Column::CategoryId.eq(category_id));
        }
        if !include_pinned {
            select = select.filter(forum_topic::Column::IsPinned.eq(false));
        } else {
            select = select.order_by_desc(forum_topic::Column::IsPinned);
        }

        let select = match sort {
            "activity" => select
                .order_by_desc(forum_topic::Column::LastReplyAt)
                .order_by_desc(forum_topic::Column::UpdatedAt),
            "newest" => select.order_by_desc(forum_topic::Column::CreatedAt),
            "top" => select
                .order_by_desc(Expr::cust(
                    "COALESCE((SELECT SUM(forum_topic_votes.value) FROM forum_topic_votes \
                     WHERE forum_topic_votes.tenant_id = forum_topics.tenant_id \
                     AND forum_topic_votes.topic_id = forum_topics.id), 0)",
                ))
                .order_by_desc(forum_topic::Column::LastReplyAt)
                .order_by_desc(forum_topic::Column::UpdatedAt),
            other => {
                return Err(ForumError::Validation(format!(
                    "Unsupported Forum widget topic sort: {other}"
                )));
            }
        };
        Ok(select.order_by_desc(forum_topic::Column::Id))
    }

    /// Exact owner-audience widget topic list.
    ///
    /// Every candidate is checked with
    /// `ForumTopicAudienceVisibilityService::is_topic_owner_visible`, which covers the inherited
    /// category floor and every richer category/topic layer. Pagination and `total` count only
    /// topics the viewer can read, so a page never has gaps and the total never counts hidden
    /// topics.
    #[instrument(skip(self, security, context, visibility, query, fallback_locale))]
    pub(crate) async fn list_widget_preview_owner_visible(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: rustok_api::PortContext,
        visibility: &crate::services::ForumTopicAudienceVisibilityService,
        query: WidgetTopicListQuery<'_>,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        let locale = normalize_locale(&context.locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let per_page = query.per_page.clamp(1, 100);
        let viewer = crate::services::ForumTopicAudienceViewer::authenticated(
            security.clone(),
            context,
        )?;

        let candidates = Self::widget_topic_candidates(
            tenant_id,
            query.category_id,
            query.include_pinned,
            query.sort,
        )?;
        let paginator = candidates.paginate(&self.db, FORUM_WIDGET_TOPIC_LIST_SCAN_BATCH);
        let candidate_count = paginator.num_items().await?;
        if candidate_count > FORUM_WIDGET_TOPIC_LIST_MAX_CANDIDATES {
            return Err(ForumError::Validation(format!(
                "Forum widget topic list has {candidate_count} candidate topics; narrow \
                 category_id so the preview scans at most {FORUM_WIDGET_TOPIC_LIST_MAX_CANDIDATES}"
            )));
        }

        let skip = query.page.saturating_sub(1).saturating_mul(per_page);
        let mut visible_total: u64 = 0;
        let mut selected = Vec::new();
        for page_index in 0..paginator.num_pages().await? {
            for topic in paginator.fetch_page(page_index).await? {
                if !visibility
                    .is_topic_owner_visible(tenant_id, topic.id, &viewer)
                    .await?
                {
                    continue;
                }
                if visible_total >= skip && (selected.len() as u64) < per_page {
                    selected.push(topic);
                }
                visible_total = visible_total.saturating_add(1);
            }
        }

        let items = self
            .hydrate_topic_list_items(
                tenant_id,
                security.user_id,
                selected,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?;
        Ok((items, visible_total))
    }
}
