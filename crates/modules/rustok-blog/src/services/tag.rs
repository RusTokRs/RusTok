/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource};
use rustok_content::normalize_locale_code;
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{CreateTagInput, ListTagsFilter, TagListItem, TagResponse, UpdateTagInput};
use crate::entities::{blog_post, blog_post_tag, blog_tag_usage};
use crate::error::{BlogError, BlogResult};
use crate::ports::taxonomy::{
    BlogTaxonomyCreateTagInput, BlogTaxonomyPort, BlogTaxonomyTagSnapshot,
    BlogTaxonomyUpdateTagInput, NoopBlogTaxonomyPort,
};
use crate::services::rbac::enforce_scope;

mod relations;

pub(crate) use relations::{
    initialize_tag_usage_in_tx, load_post_tags_map, resolve_tag_id_for_posts, sync_post_tags_in_tx,
};
pub use relations::{remove_post_tag_links_in_tx, replace_post_tag_links_in_tx};

const BLOG_SCOPE_VALUE: &str = "blog";
const MAX_TAGS_PER_PAGE: u64 = 100;

pub struct TagService {
    db: DatabaseConnection,
    pub(crate) taxonomy_port: Arc<dyn BlogTaxonomyPort>,
}

impl TagService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            taxonomy_port: Arc::new(NoopBlogTaxonomyPort),
        }
    }

    pub fn with_taxonomy_port(mut self, port: Arc<dyn BlogTaxonomyPort>) -> Self {
        self.taxonomy_port = port;
        self
    }

    #[instrument(skip(self, security, input))]
    pub async fn create_tag(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateTagInput,
    ) -> BlogResult<Uuid> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        enforce_scope(&security, Resource::Tags, Action::Create)?;
        validate_tag_name(&input.name)?;
        validate_optional_tag_slug(input.slug.as_deref())?;

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        let created = self
            .taxonomy_port
            .create_tag_in_tx(
                &txn,
                tenant_id,
                BlogTaxonomyCreateTagInput {
                    name: input.name,
                    slug: input.slug,
                    locale: normalize_locale(&input.locale)?,
                },
            )
            .await?;
        let tag_id = created.id;
        initialize_tag_usage_in_tx(&txn, tenant_id, tag_id, &created.canonical_key).await?;
        relations::publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id).await?;
        txn.commit().await.map_err(BlogError::from)?;
        Ok(tag_id)
    }

    #[instrument(skip(self, security))]
    pub async fn get_tag(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        tag_id: Uuid,
        locale: &str,
    ) -> BlogResult<TagResponse> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if tag_id.is_nil() {
            return Err(BlogError::validation("Tag ID must not be nil"));
        }
        enforce_scope(&security, Resource::Tags, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let term = self.find_visible_term(tenant_id, tag_id, &locale).await?;
        let require_usage_projection =
            term.scope_type == "module" && term.scope_value.as_deref() == Some(BLOG_SCOPE_VALUE);
        let use_count = self
            .load_tag_usage_count(tenant_id, tag_id, require_usage_projection)
            .await?;

        let effective_locale = tag_effective_locale(&term)?;
        Ok(to_tag_owner_response(
            tenant_id,
            term,
            effective_locale,
            use_count,
        ))
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_tag(
        &self,
        tenant_id: Uuid,
        tag_id: Uuid,
        security: SecurityContext,
        input: UpdateTagInput,
    ) -> BlogResult<TagResponse> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if tag_id.is_nil() {
            return Err(BlogError::validation("Tag ID must not be nil"));
        }
        enforce_scope(&security, Resource::Tags, Action::Update)?;
        let locale = normalize_locale(&input.locale)?;
        validate_tag_name_if_present(input.name.as_deref())?;
        validate_optional_tag_slug(input.slug.as_deref())?;
        self.ensure_blog_owned_tag(tenant_id, tag_id, &locale)
            .await?;
        let txn = self.db.begin().await.map_err(BlogError::from)?;
        self.taxonomy_port
            .lock_tag_in_tx(&txn, tenant_id, tag_id)
            .await?;
        let term = self
            .taxonomy_port
            .update_tag_in_tx(
                &txn,
                tenant_id,
                tag_id,
                &security,
                BlogTaxonomyUpdateTagInput {
                    locale: locale.clone(),
                    name: input.name,
                    slug: input.slug,
                },
            )
            .await?;
        // Checked inside the transaction: a snapshot that cannot be presented rolls the
        // update back instead of failing after the commit.
        let effective_locale = tag_effective_locale(&term)?;
        relations::refresh_tag_usage_in_tx(Some(&*self.taxonomy_port), &txn, tenant_id, &[tag_id])
            .await?;
        relations::publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id).await?;
        txn.commit().await.map_err(BlogError::from)?;

        let use_count = self.load_tag_usage_count(tenant_id, tag_id, true).await?;

        Ok(to_tag_mutation_response(term, effective_locale, use_count))
    }

    #[instrument(skip(self, security))]
    pub async fn delete_tag(
        &self,
        tenant_id: Uuid,
        tag_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if tag_id.is_nil() {
            return Err(BlogError::validation("Tag ID must not be nil"));
        }
        enforce_scope(&security, Resource::Tags, Action::Delete)?;
        self.ensure_blog_owned_tag(tenant_id, tag_id, PLATFORM_FALLBACK_LOCALE)
            .await?;
        let txn = self.db.begin().await.map_err(BlogError::from)?;
        self.taxonomy_port
            .lock_tag_in_tx(&txn, tenant_id, tag_id)
            .await?;
        relations::bump_posts_for_tag_relation_removal_in_tx(&txn, tenant_id, tag_id).await?;
        self.taxonomy_port
            .delete_tag_in_tx(&txn, tenant_id, tag_id, &security)
            .await?;
        relations::publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id).await?;
        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn list_tags(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListTagsFilter,
    ) -> BlogResult<(Vec<TagListItem>, u64)> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        enforce_scope(&security, Resource::Tags, Action::List)?;
        let locale =
            normalize_locale(filter.locale.as_deref().unwrap_or(PLATFORM_FALLBACK_LOCALE))?;
        let page = Ord::max(filter.page, 1);
        let per_page = bounded_tag_page_size(filter.per_page);

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        let valid_tag_ids = self
            .taxonomy_port
            .find_all_valid_tag_ids((&txn).into(), tenant_id)
            .await?;
        let paginator = blog_tag_usage::Entity::find()
            .filter(blog_tag_usage::Column::TenantId.eq(tenant_id))
            .filter(blog_tag_usage::Column::TagId.is_in(valid_tag_ids))
            .order_by_desc(blog_tag_usage::Column::UseCount)
            .order_by_asc(blog_tag_usage::Column::CanonicalKey)
            .order_by_asc(blog_tag_usage::Column::TagId)
            .paginate(&txn, per_page);

        let total = paginator.num_items().await.map_err(BlogError::from)?;
        if total == 0 {
            txn.commit().await.map_err(BlogError::from)?;
            return Ok((Vec::new(), 0));
        }

        let last_page = total.saturating_add(per_page.saturating_sub(1)) / per_page;
        if page > last_page {
            txn.commit().await.map_err(BlogError::from)?;
            return Ok((Vec::new(), total));
        }

        let usage_rows = paginator
            .fetch_page(page - 1)
            .await
            .map_err(BlogError::from)?;
        let term_ids = usage_rows.iter().map(|row| row.tag_id).collect::<Vec<_>>();
        let terms_by_id = self
            .taxonomy_port
            .load_tags_by_ids((&txn).into(), tenant_id, &term_ids, &locale, None)
            .await?;

        let mut items = Vec::with_capacity(usage_rows.len());
        for usage in usage_rows {
            let Some(term) = terms_by_id.get(&usage.tag_id) else {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    tag_id = %usage.tag_id,
                    "Blog tag usage projection references a missing Taxonomy tag; skipping the row"
                );
                continue;
            };
            if term.scope_type == "module" && term.scope_value.as_deref() != Some(BLOG_SCOPE_VALUE)
            {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    tag_id = %term.id,
                    "Blog tag usage projection references a non-Blog module term; skipping the row"
                );
                continue;
            }
            items.push(TagListItem {
                id: term.id,
                locale: locale.clone(),
                effective_locale: tag_effective_locale(term)?,
                name: term.name.clone(),
                slug: term.slug.clone(),
                use_count: usage.use_count,
                created_at: term.created_at,
            });
        }

        txn.commit().await.map_err(BlogError::from)?;
        Ok((items, total))
    }

    async fn ensure_blog_owned_tag(
        &self,
        tenant_id: Uuid,
        tag_id: Uuid,
        locale: &str,
    ) -> BlogResult<()> {
        let term = self.find_visible_term(tenant_id, tag_id, locale).await?;
        if term.scope_type != "module" || term.scope_value.as_deref() != Some(BLOG_SCOPE_VALUE) {
            return Err(BlogError::forbidden(
                "Shared Taxonomy tags must be managed by the Taxonomy owner",
            ));
        }
        Ok(())
    }

    async fn find_visible_term(
        &self,
        tenant_id: Uuid,
        tag_id: Uuid,
        locale: &str,
    ) -> BlogResult<BlogTaxonomyTagSnapshot> {
        let terms = self
            .taxonomy_port
            .load_tags_by_ids(
                (&self.db).into(),
                tenant_id,
                &[tag_id],
                locale,
                Some(PLATFORM_FALLBACK_LOCALE),
            )
            .await?;
        terms
            .get(&tag_id)
            .cloned()
            .ok_or_else(|| BlogError::tag_not_found(tag_id))
    }

    async fn load_tag_usage_count(
        &self,
        tenant_id: Uuid,
        tag_id: Uuid,
        require_usage_projection: bool,
    ) -> BlogResult<i32> {
        let usage = blog_tag_usage::Entity::find()
            .filter(blog_tag_usage::Column::TenantId.eq(tenant_id))
            .filter(blog_tag_usage::Column::TagId.eq(tag_id))
            .one(&self.db)
            .await?;

        match usage {
            Some(usage) => Ok(usage.use_count),
            None if require_usage_projection => Err(BlogError::invariant(format!(
                "Blog tag {tag_id} is missing its usage projection",
            ))),
            None => Ok(0),
        }
    }
}

fn bounded_tag_page_size(value: u64) -> u64 {
    value.clamp(1, MAX_TAGS_PER_PAGE)
}

fn validate_tag_name(name: &str) -> BlogResult<()> {
    if name.trim().is_empty() {
        return Err(BlogError::validation("Tag name cannot be empty"));
    }
    if name.chars().count() > crate::services::post::MAX_TAG_NAME_CHARS {
        return Err(BlogError::validation(format!(
            "Tag name exceeds maximum length of {} characters",
            crate::services::post::MAX_TAG_NAME_CHARS
        )));
    }
    Ok(())
}

fn validate_tag_name_if_present(name: Option<&str>) -> BlogResult<()> {
    if let Some(name) = name {
        validate_tag_name(name)?;
    }
    Ok(())
}

fn validate_optional_tag_slug(slug: Option<&str>) -> BlogResult<()> {
    let Some(slug) = slug else {
        return Ok(());
    };
    if slug.chars().count() > 100 {
        return Err(BlogError::validation(
            "Tag slug cannot exceed 100 characters",
        ));
    }
    if let Some(normalized) = crate::normalization::normalize_term_route_key(slug)
        && normalized.chars().count() > 120
    {
        return Err(BlogError::validation(
            "Tag slug cannot exceed 120 characters after normalization",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod slug_validation_tests {
    use super::validate_optional_tag_slug;

    #[test]
    fn unicode_slug_with_long_ascii_expansion_is_bounded_by_taxonomy_limit() {
        let non_ascii_slug = "щ".repeat(31);
        assert!(validate_optional_tag_slug(Some(&non_ascii_slug)).is_err());
    }
}

fn normalize_tag_names(names: &[String]) -> Vec<String> {
    let mut normalized = Vec::with_capacity(names.len());
    let mut seen = HashSet::with_capacity(names.len());

    for name in names {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        let canonical_comparison = trimmed.to_lowercase();
        if seen.insert(canonical_comparison) {
            normalized.push(trimmed.to_string());
        }
    }

    normalized
}

fn normalize_locale(locale: &str) -> BlogResult<String> {
    normalize_locale_code(locale).ok_or_else(|| BlogError::validation("Locale cannot be empty"))
}

/// The Taxonomy provider states the locale of the copy it returned. A snapshot without
/// it cannot be presented as a localized tag, so it is rejected as a broken projection
/// instead of being given an assumed locale.
fn tag_effective_locale(term: &BlogTaxonomyTagSnapshot) -> BlogResult<String> {
    if term.effective_locale.trim().is_empty() {
        return Err(BlogError::invariant(format!(
            "Taxonomy tag {} was returned without an effective locale",
            term.id
        )));
    }
    Ok(term.effective_locale.clone())
}

fn to_tag_owner_response(
    tenant_id: Uuid,
    term: BlogTaxonomyTagSnapshot,
    effective_locale: String,
    use_count: i32,
) -> TagResponse {
    TagResponse {
        id: term.id,
        tenant_id,
        locale: term.locale,
        effective_locale,
        name: term.name,
        slug: term.slug,
        use_count,
        created_at: term.created_at,
    }
}

fn to_tag_mutation_response(
    term: BlogTaxonomyTagSnapshot,
    effective_locale: String,
    use_count: i32,
) -> TagResponse {
    TagResponse {
        id: term.id,
        tenant_id: term.tenant_id,
        locale: term.locale,
        effective_locale,
        name: term.name,
        slug: term.slug,
        use_count,
        created_at: term.created_at,
    }
}

#[cfg(test)]
mod pagination_tests {
    use super::{
        MAX_TAGS_PER_PAGE, bounded_tag_page_size, validate_optional_tag_slug, validate_tag_name,
    };

    #[test]
    fn tag_page_size_is_bounded_by_owner_service() {
        assert_eq!(bounded_tag_page_size(0), 1);
        assert_eq!(bounded_tag_page_size(20), 20);
        assert_eq!(
            bounded_tag_page_size(MAX_TAGS_PER_PAGE + 1),
            MAX_TAGS_PER_PAGE
        );
    }

    #[test]
    fn tag_name_limit_counts_unicode_characters_not_utf8_bytes() {
        let hundred_characters = "Ж".repeat(100);
        let one_hundred_and_one_characters = "Ж".repeat(101);

        assert!(validate_tag_name(&hundred_characters).is_ok());
        assert!(validate_tag_name(&one_hundred_and_one_characters).is_err());
    }

    #[test]
    fn tag_slug_limit_is_enforced_at_the_owner_service_boundary() {
        assert!(validate_optional_tag_slug(Some(&"a".repeat(100))).is_ok());
        assert!(validate_optional_tag_slug(Some(&"a".repeat(101))).is_err());
        assert!(validate_optional_tag_slug(None).is_ok());
    }
}
