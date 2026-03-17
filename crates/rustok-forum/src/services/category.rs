use sea_orm::DatabaseConnection;
use tracing::instrument;
use uuid::Uuid;

use rustok_content::{CategoryService as ContentCategoryService, ListCategoriesFilter};
use rustok_core::SecurityContext;

use crate::dto::{CategoryListItem, CategoryResponse, CreateCategoryInput, UpdateCategoryInput};
use crate::error::{ForumError, ForumResult};

pub struct CategoryService {
    categories: ContentCategoryService,
}

impl CategoryService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            categories: ContentCategoryService::new(db),
        }
    }

    #[instrument(skip(self, input, security))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateCategoryInput,
    ) -> ForumResult<CategoryResponse> {
        if input.name.trim().is_empty() {
            return Err(ForumError::Validation(
                "Category name cannot be empty".to_string(),
            ));
        }
        if input.slug.trim().is_empty() {
            return Err(ForumError::Validation(
                "Category slug cannot be empty".to_string(),
            ));
        }

        let settings = serde_json::json!({
            "icon": input.icon,
            "color": input.color,
            "moderated": input.moderated,
            "topic_count": 0,
            "reply_count": 0
        });

        let id = self
            .categories
            .create(
                tenant_id,
                security,
                rustok_content::CreateCategoryInput {
                    locale: input.locale.clone(),
                    name: input.name,
                    slug: Some(input.slug),
                    description: input.description,
                    parent_id: input.parent_id,
                    position: input.position,
                    settings,
                },
            )
            .await?;

        let cat = self
            .categories
            .get(tenant_id, id, &input.locale)
            .await?;

        Ok(content_to_forum_response(cat, &input.locale))
    }

    #[instrument(skip(self))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
    ) -> ForumResult<CategoryResponse> {
        self.get_with_locale_fallback(tenant_id, category_id, locale, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
        _fallback_locale: Option<&str>,
    ) -> ForumResult<CategoryResponse> {
        let cat = self.categories.get(tenant_id, category_id, locale).await?;
        Ok(content_to_forum_response(cat, locale))
    }

    #[instrument(skip(self, security, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryInput,
    ) -> ForumResult<CategoryResponse> {
        let existing = self
            .categories
            .get(tenant_id, category_id, &input.locale)
            .await?;

        let existing_settings = &existing.settings;
        let settings = serde_json::json!({
            "icon": input.icon.or_else(|| existing_settings.get("icon").and_then(|v| v.as_str()).map(|s| s.to_string())),
            "color": input.color.or_else(|| existing_settings.get("color").and_then(|v| v.as_str()).map(|s| s.to_string())),
            "moderated": input.moderated.unwrap_or_else(|| existing_settings.get("moderated").and_then(|v| v.as_bool()).unwrap_or(false)),
            "topic_count": existing_settings.get("topic_count").and_then(|v| v.as_i64()).unwrap_or(0),
            "reply_count": existing_settings.get("reply_count").and_then(|v| v.as_i64()).unwrap_or(0)
        });

        let cat = self
            .categories
            .update(
                tenant_id,
                category_id,
                security,
                rustok_content::UpdateCategoryInput {
                    locale: input.locale.clone(),
                    name: input.name,
                    slug: input.slug,
                    description: input.description,
                    position: input.position,
                    settings: Some(settings),
                },
            )
            .await?;

        Ok(content_to_forum_response(cat, &input.locale))
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.categories
            .delete(tenant_id, category_id, security)
            .await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
    ) -> ForumResult<Vec<CategoryListItem>> {
        let (items, _) = self
            .list_paginated_with_locale_fallback(tenant_id, security, locale, 1, 1000, None)
            .await?;
        Ok(items)
    }

    #[instrument(skip(self, security))]
    pub async fn list_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<Vec<CategoryListItem>> {
        let (items, _) = self
            .list_paginated_with_locale_fallback(tenant_id, security, locale, 1, 1000, fallback_locale)
            .await?;
        Ok(items)
    }

    #[instrument(skip(self, security))]
    pub async fn list_paginated_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        page: u64,
        per_page: u64,
        _fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<CategoryListItem>, u64)> {
        let (items, total) = self
            .categories
            .list(
                tenant_id,
                security,
                ListCategoriesFilter {
                    locale: Some(locale.to_string()),
                    page,
                    per_page,
                },
            )
            .await?;

        let list = items
            .into_iter()
            .map(|item| {
                let s = &item.settings;
                CategoryListItem {
                    id: item.id,
                    locale: locale.to_string(),
                    effective_locale: item.effective_locale,
                    name: item.name,
                    slug: item.slug,
                    description: None,
                    icon: s.get("icon").and_then(|v| v.as_str()).map(|v| v.to_string()),
                    color: s.get("color").and_then(|v| v.as_str()).map(|v| v.to_string()),
                    topic_count: s.get("topic_count").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                    reply_count: s.get("reply_count").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                }
            })
            .collect();

        Ok((list, total))
    }
}

fn content_to_forum_response(
    cat: rustok_content::CategoryResponse,
    locale: &str,
) -> CategoryResponse {
    let s = &cat.settings;
    CategoryResponse {
        id: cat.id,
        requested_locale: locale.to_string(),
        locale: locale.to_string(),
        effective_locale: cat.effective_locale,
        available_locales: cat.available_locales,
        name: cat.name,
        slug: cat.slug,
        description: cat.description,
        icon: s.get("icon").and_then(|v| v.as_str()).map(|v| v.to_string()),
        color: s.get("color").and_then(|v| v.as_str()).map(|v| v.to_string()),
        parent_id: cat.parent_id,
        position: cat.position,
        topic_count: s.get("topic_count").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        reply_count: s.get("reply_count").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        moderated: s.get("moderated").and_then(|v| v.as_bool()).unwrap_or(false),
    }
}
