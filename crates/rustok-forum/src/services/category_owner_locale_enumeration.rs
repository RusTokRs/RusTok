impl CategoryService {
    pub const MAX_FORUM_CATEGORY_LOCALE_ENUMERATION_IDS: usize =
        category::CategoryService::MAX_FORUM_CATEGORY_LOCALE_ENUMERATION_IDS;

    pub async fn available_locales_for_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_ids: &[Uuid],
    ) -> ForumResult<Vec<(Uuid, Vec<String>)>> {
        if security.is_public_read() {
            return Err(ForumError::forbidden(
                "Forum category locale enumeration requires an authenticated operator context",
            ));
        }
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        self.inner
            .available_locales_for_categories(tenant_id, security, category_ids)
            .await
    }
}
