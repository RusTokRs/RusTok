use std::collections::{HashMap, HashSet};

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
use rustok_taxonomy::{
    ModuleTermCreateInput, ModuleTermMutationResult, ModuleTermUpdateInput, TaxonomyOwnerReader,
    TaxonomyOwnerTerm, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
    delete_module_term_in_tx, lock_module_term_in_tx, update_module_term_in_tx,
};

use crate::dto::{CreateTagInput, ListTagsFilter, TagListItem, TagResponse, UpdateTagInput};
use crate::entities::{blog_post, blog_post_tag, blog_tag_usage};
use crate::error::{BlogError, BlogResult};
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
}

impl TagService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
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
        let tag_id = TaxonomyService::new(self.db.clone())
            .create_module_term_in_tx(
                &txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                BLOG_SCOPE_VALUE,
                ModuleTermCreateInput {
                    locale: normalize_locale(&input.locale)?,
                    name: input.name,
                    slug: input.slug,
                    canonical_key: None,
                },
            )
            .await?;
        let term = TaxonomyOwnerReader::load_terms_by_ids_in_tx(
            &txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            &[tag_id],
            PLATFORM_FALLBACK_LOCALE,
            None,
        )
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| BlogError::invariant("Created Blog tag is missing from Taxonomy"))?;
        initialize_tag_usage_in_tx(&txn, tenant_id, tag_id, &term.canonical_key).await?;
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
        let require_usage_projection = term.scope_type == TaxonomyScopeType::Module
            && term.scope_value.as_deref() == Some(BLOG_SCOPE_VALUE);
        let use_count = self
            .load_tag_usage_count(tenant_id, tag_id, require_usage_projection)
            .await?;

        Ok(to_tag_owner_response(tenant_id, term, use_count))
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
        // The ownership pre-check above runs outside this transaction. Lock
        // the scoped term row before writing, mirroring `delete_tag`, so a
        // concurrent scope change by the Taxonomy owner cannot interleave
        // between the check and the update and leave a Blog write on a term
        // that is no longer Blog-owned.
        lock_module_term_in_tx(
            &txn,
            tenant_id,
            tag_id,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE_VALUE,
        )
        .await?;
        let term = update_module_term_in_tx(
            &txn,
            tenant_id,
            tag_id,
            &security,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE_VALUE,
            ModuleTermUpdateInput {
                locale: locale.clone(),
                name: input.name,
                slug: input.slug,
            },
        )
        .await?;
        // The projection row is derived state, and `list_tags` pages and orders
        // by it, so the tag's own mutation re-derives it from the canonical
        // sources instead of trusting whatever the row held. Taxonomy
        // `canonical_key` is immutable identity and does not move on a rename
        // (`DECISIONS/2026-09-22-blog-tag-bounded-pagination.md`), so this is a
        // repair and re-count, not a re-sort: a row created by the backfill, or
        // left behind by a writer outside this service, is brought back to the
        // projection invariant on the next tag write.
        relations::refresh_tag_usage_in_tx(&txn, tenant_id, &[tag_id]).await?;
        relations::publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id).await?;
        txn.commit().await.map_err(BlogError::from)?;

        let use_count = self.load_tag_usage_count(tenant_id, tag_id, true).await?;

        Ok(to_tag_mutation_response(term, use_count))
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
        lock_module_term_in_tx(
            &txn,
            tenant_id,
            tag_id,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE_VALUE,
        )
        .await?;
        relations::bump_posts_for_tag_relation_removal_in_tx(&txn, tenant_id, tag_id).await?;
        delete_module_term_in_tx(
            &txn,
            tenant_id,
            tag_id,
            &security,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE_VALUE,
        )
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
        let page = filter.page.max(1);
        let per_page = bounded_tag_page_size(filter.per_page);

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        let paginator = blog_tag_usage::Entity::find()
            .filter(blog_tag_usage::Column::TenantId.eq(tenant_id))
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
        let terms = TaxonomyOwnerReader::load_terms_by_ids_in_tx(
            &txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            &term_ids,
            &locale,
            None,
        )
        .await?;
        let terms_by_id = terms
            .into_iter()
            .map(|term| (term.id, term))
            .collect::<HashMap<_, _>>();

        let mut items = Vec::with_capacity(usage_rows.len());
        for usage in usage_rows {
            let Some(term) = terms_by_id.get(&usage.tag_id) else {
                // A projection row whose Taxonomy term is gone is broken derived
                // state, not a reason to fail the whole public listing: skip the
                // row and leave a trace, as the post list does for malformed
                // rows. The write path repairs the projection on the next
                // mutation (`refresh_tag_usage_in_tx`).
                tracing::warn!(
                    tenant_id = %tenant_id,
                    tag_id = %usage.tag_id,
                    "Blog tag usage projection references a missing Taxonomy tag; skipping the row"
                );
                continue;
            };
            if term.scope_type == TaxonomyScopeType::Module
                && term.scope_value.as_deref() != Some(BLOG_SCOPE_VALUE)
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
                effective_locale: term.effective_locale.clone(),
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
        if term.scope_type != TaxonomyScopeType::Module
            || term.scope_value.as_deref() != Some(BLOG_SCOPE_VALUE)
        {
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
    ) -> BlogResult<TaxonomyOwnerTerm> {
        let reader = TaxonomyOwnerReader::new(self.db.clone());
        let term_ids = [tag_id];
        if let Some(term) = reader
            .load_scoped_terms(
                tenant_id,
                TaxonomyTermKind::Tag,
                TaxonomyScopeType::Module,
                Some(BLOG_SCOPE_VALUE),
                Some(&term_ids),
                locale,
                Some(PLATFORM_FALLBACK_LOCALE),
            )
            .await?
            .into_iter()
            .next()
        {
            return Ok(term);
        }

        reader
            .load_scoped_terms(
                tenant_id,
                TaxonomyTermKind::Tag,
                TaxonomyScopeType::Global,
                None,
                Some(&term_ids),
                locale,
                Some(PLATFORM_FALLBACK_LOCALE),
            )
            .await?
            .into_iter()
            .next()
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
            "Tag name cannot exceed {} characters",
            crate::services::post::MAX_TAG_NAME_CHARS
        )));
    }
    Ok(())
}

fn normalize_tag_names(tag_names: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for name in tag_names {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Unicode-aware case folding: ASCII-only folding let pairs like
        // "Новости"/"НОВОСТИ" through as distinct names, which then resolved
        // to duplicate Taxonomy terms and failed the post write on the
        // relation primary key instead of deduplicating here.
        let key = trimmed.to_lowercase();
        if seen.insert(key) {
            normalized.push(trimmed.to_string());
        }
    }
    normalized
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
    if let Some(normalized) = rustok_taxonomy::normalize_term_route_key(slug)
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
    fn normalized_tag_slug_respects_taxonomy_storage_limit() {
        let unicode_slug = "北".repeat(100);
        assert!(validate_optional_tag_slug(Some(unicode_slug.as_str())).is_err());
    }

    #[test]
    fn ordinary_tag_slug_stays_valid() {
        assert!(validate_optional_tag_slug(Some("summer-sale")).is_ok());
    }
}

fn normalize_locale(locale: &str) -> BlogResult<String> {
    normalize_locale_code(locale).ok_or_else(|| BlogError::validation("Locale cannot be empty"))
}

fn to_tag_owner_response(tenant_id: Uuid, term: TaxonomyOwnerTerm, use_count: i32) -> TagResponse {
    TagResponse {
        id: term.id,
        tenant_id,
        locale: term.requested_locale,
        effective_locale: term.effective_locale,
        name: term.name,
        slug: term.slug,
        use_count,
        created_at: term.created_at,
    }
}

fn to_tag_mutation_response(term: ModuleTermMutationResult, use_count: i32) -> TagResponse {
    TagResponse {
        id: term.id,
        tenant_id: term.tenant_id,
        locale: term.locale,
        effective_locale: term.effective_locale,
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
