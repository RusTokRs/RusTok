impl CategoryService {
    #[instrument(skip(self, security, hidden_category_ids))]
    pub(crate) async fn list_paginated_with_locale_fallback_and_hidden_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        page: u64,
        per_page: u64,
        fallback_locale: Option<&str>,
        hidden_category_ids: &[Uuid],
    ) -> ForumResult<(Vec<CategoryListItem>, u64)> {
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let mut query = forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .filter(
                Expr::col((forum_category::Entity, forum_category::Column::Id))
                    .not_in_subquery(archived_category_ids_subquery(tenant_id)),
            );
        if !hidden_category_ids.is_empty() {
            query =
                query.filter(forum_category::Column::Id.is_not_in(hidden_category_ids.to_vec()));
        }
        let paginator = query
            .order_by_asc(forum_category::Column::Position)
            .paginate(&self.db, per_page.max(1));
        let total = paginator.num_items().await?;
        let categories = paginator.fetch_page(page.saturating_sub(1)).await?;
        let category_ids: Vec<Uuid> = categories.iter().map(|item| item.id).collect();
        let translations_by_category_id = self
            .load_translations_map_for_categories(tenant_id, &category_ids)
            .await?;
        let subscription_flags = SubscriptionService::new(self.db.clone())
            .category_subscription_flags(tenant_id, &category_ids, security.user_id)
            .await?;

        let mut items = Vec::with_capacity(categories.len());
        for category in categories {
            let localized = translations_by_category_id
                .get(&category.id)
                .cloned()
                .unwrap_or_default();
            let resolved = resolve_by_locale_with_fallback(
                &localized,
                &locale,
                fallback_locale.as_deref(),
                |translation| translation.locale.as_str(),
            );
            let translation = resolved.item.ok_or_else(|| {
                ForumError::Validation(format!(
                    "Forum category {} has no localized translation",
                    category.id
                ))
            })?;

            items.push(CategoryListItem {
                id: category.id,
                requested_locale: locale.clone(),
                locale: locale.clone(),
                effective_locale: resolved.effective_locale,
                available_locales: available_locales_from(&localized, |translation| {
                    translation.locale.as_str()
                }),
                name: translation.name.clone(),
                slug: translation.slug.clone(),
                description: translation.description.clone(),
                icon: category.icon.clone(),
                color: category.color.clone(),
                topic_count: category.topic_count,
                reply_count: category.reply_count,
                is_subscribed: subscription_flags
                    .get(&category.id)
                    .copied()
                    .unwrap_or(false),
            });
        }

        Ok((items, total))
    }
}
