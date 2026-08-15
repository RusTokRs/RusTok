impl CategoryService {
    pub const MAX_FORUM_CATEGORY_LOCALE_ENUMERATION_IDS: usize = 512;

    pub(crate) async fn available_locales_for_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_ids: &[Uuid],
    ) -> ForumResult<Vec<(Uuid, Vec<String>)>> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;

        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum category locale enumeration requires a non-nil tenant id".to_string(),
            ));
        }
        if category_ids.len() > Self::MAX_FORUM_CATEGORY_LOCALE_ENUMERATION_IDS {
            return Err(ForumError::Validation(format!(
                "Forum category locale enumeration is limited to {} category IDs",
                Self::MAX_FORUM_CATEGORY_LOCALE_ENUMERATION_IDS
            )));
        }
        if category_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut seen = std::collections::BTreeSet::new();
        for category_id in category_ids {
            if category_id.is_nil() {
                return Err(ForumError::Validation(
                    "Forum category locale enumeration requires non-nil category IDs".to_string(),
                ));
            }
            if !seen.insert(*category_id) {
                return Err(ForumError::Validation(format!(
                    "Forum category locale enumeration repeats category {category_id}"
                )));
            }
        }

        let existing = forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .filter(forum_category::Column::Id.is_in(category_ids.to_vec()))
            .all(&self.db)
            .await?;
        let existing_ids = existing
            .into_iter()
            .map(|category| category.id)
            .collect::<std::collections::BTreeSet<_>>();
        for category_id in category_ids {
            if !existing_ids.contains(category_id) {
                return Err(ForumError::CategoryNotFound(*category_id));
            }
        }

        let mut translations_by_category = self
            .load_translations_map_for_categories(tenant_id, category_ids)
            .await?;
        let mut result = Vec::with_capacity(category_ids.len());
        for category_id in category_ids {
            let translations = translations_by_category
                .remove(category_id)
                .unwrap_or_default();
            let locales =
                available_locales_from(&translations, |translation| translation.locale.as_str());
            if locales.is_empty() {
                return Err(ForumError::Validation(format!(
                    "Forum category {category_id} has no stored locale translation"
                )));
            }
            result.push((*category_id, locales));
        }

        Ok(result)
    }
}
