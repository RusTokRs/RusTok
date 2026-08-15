impl TopicService {
    /// Bounded owner read for the Forum Page Builder topic-list widget.
    ///
    /// Widget ordering is applied before pagination. The public facade supplies the exact hidden
    /// category set before this persistence-layer query executes, so Page Builder does not gain a
    /// second visibility policy path.
    #[instrument(skip(self, security, hidden_category_ids))]
    pub(crate) async fn list_widget_preview_with_locale_fallback_and_hidden_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_id: Option<Uuid>,
        page: u64,
        per_page: u64,
        include_pinned: bool,
        sort: &str,
        locale: &str,
        fallback_locale: Option<&str>,
        hidden_category_ids: &[Uuid],
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

        let mut select =
            forum_topic::Entity::find().filter(forum_topic::Column::TenantId.eq(tenant_id));
        if let Some(category_id) = category_id {
            select = select.filter(forum_topic::Column::CategoryId.eq(category_id));
        }
        if !hidden_category_ids.is_empty() {
            select = select
                .filter(forum_topic::Column::CategoryId.is_not_in(hidden_category_ids.to_vec()));
        }
        if !include_pinned {
            select = select.filter(forum_topic::Column::IsPinned.eq(false));
        } else {
            select = select.order_by_desc(forum_topic::Column::IsPinned);
        }

        select = match sort {
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

        let paginator = select
            .order_by_desc(forum_topic::Column::Id)
            .paginate(&self.db, per_page.clamp(1, 100));
        let total = paginator.num_items().await?;
        let topics = paginator.fetch_page(page.saturating_sub(1)).await?;
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
