use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
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
        enforce_scope(&security, Resource::Tags, Action::Create)?;
        validate_tag_name(&input.name)?;

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
        publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id).await?;
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
        enforce_scope(&security, Resource::Tags, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let term = self.find_visible_term(tenant_id, tag_id, &locale).await?;
        let require_usage_projection =
            term.scope_type == TaxonomyScopeType::Module
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
        enforce_scope(&security, Resource::Tags, Action::Update)?;
        let locale = normalize_locale(&input.locale)?;
        self.ensure_blog_owned_tag(tenant_id, tag_id, &locale).await?;
        let txn = self.db.begin().await.map_err(BlogError::from)?;
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
        publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id).await?;
        txn.commit().await.map_err(BlogError::from)?;

        let use_count = self
            .load_tag_usage_count(tenant_id, tag_id, true)
            .await?;

        Ok(to_tag_mutation_response(term, use_count))
    }

    #[instrument(skip(self, security))]
    pub async fn delete_tag(
        &self,
        tenant_id: Uuid,
        tag_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
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
        bump_posts_for_tag_relation_removal_in_tx(&txn, tenant_id, tag_id).await?;
        delete_module_term_in_tx(
            &txn,
            tenant_id,
            tag_id,
            &security,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE_VALUE,
        )
        .await?;
        publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id).await?;
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
        let last_page = total
            .saturating_add(per_page.saturating_sub(1))
            / per_page;
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
            let term = terms_by_id.get(&usage.tag_id).ok_or_else(|| {
                BlogError::invariant(format!(
                    "Blog tag usage projection references missing Taxonomy tag {}",
                    usage.tag_id
                ))
            })?;
            if term.scope_type == TaxonomyScopeType::Module
                && term.scope_value.as_deref() != Some(BLOG_SCOPE_VALUE)
            {
                return Err(BlogError::invariant(format!(
                    "Blog tag usage projection references non-Blog module term {}",
                    term.id
                )));
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

async fn bump_posts_for_tag_relation_removal_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_id: Uuid,
) -> BlogResult<()> {
    use sea_orm::{ExprTrait, QueryTrait};

    let valid_post_ids = blog_post::Entity::find()
        .select_only()
        .column(blog_post::Column::Id)
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .filter(blog_post::Column::Version.gt(0))
        .filter(blog_post::Column::Version.ne(i32::MAX))
        .into_query();

    let relation_filter = blog_post_tag::Entity::find()
        .select_only()
        .column(blog_post_tag::Column::PostId)
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::TagId.eq(tag_id))
        .into_query();

    let relation_count = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::TagId.eq(tag_id))
        .count(txn)
        .await?;

    let valid_relation_count = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::TagId.eq(tag_id))
        .filter(blog_post_tag::Column::PostId.in_subquery(valid_post_ids))
        .count(txn)
        .await?;

    if relation_count != valid_relation_count {
        return Err(BlogError::invariant(format!(
            "Blog tag {tag_id} has {relation_count} post relation(s), but only {valid_relation_count} reference posts with a valid persisted version",
        )));
    }

    let now = Utc::now();
    let updated = blog_post::Entity::update_many()
        .col_expr(
            blog_post::Column::Version,
            Expr::col(blog_post::Column::Version).add(1),
        )
        .col_expr(
            blog_post::Column::UpdatedAt,
            Expr::value(now),
        )
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .filter(blog_post::Column::Id.in_subquery(relation_filter))
        .filter(blog_post::Column::Version.gt(0))
        .filter(blog_post::Column::Version.ne(i32::MAX))
        .exec(txn)
        .await?;

    if updated.rows_affected as u64 != valid_relation_count {
        return Err(BlogError::conflict(format!(
            "Blog tag {tag_id} invalidated {} post(s), expected {valid_relation_count}",
            updated.rows_affected
        )));
    }

    Ok(())
}

async fn publish_blog_reindex_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
) -> BlogResult<()> {
    TransactionalEventBus::publish_root_in_tx(
        txn,
        tenant_id,
        actor_id,
        DomainEvent::ReindexRequested {
            target_type: "blog".to_string(),
            target_id: None,
        },
    )
    .await
    .map_err(BlogError::from)
}

pub(crate) async fn sync_post_tags_in_tx(
    db: &DatabaseConnection,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    tag_names: &[String],
    locale: &str,
    allow_create: bool,
) -> BlogResult<()> {
    let normalized_locale = normalize_locale(locale)?;
    let normalized_names = normalize_tag_names(tag_names);

    let previous_relations = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .all(txn)
        .await?;
    let previous_tag_ids = previous_relations
        .into_iter()
        .map(|relation| relation.tag_id)
        .collect::<Vec<_>>();

    blog_post_tag::Entity::delete_many()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .exec(txn)
        .await?;

    decrement_tag_usage_in_tx(txn, tenant_id, &previous_tag_ids).await?;

    if normalized_names.is_empty() {
        return Ok(());
    }

    let term_ids = TaxonomyService::new(db.clone())
        .ensure_module_terms_for_owner_in_tx(
            txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE_VALUE,
            &normalized_locale,
            &normalized_names,
            allow_create,
        )
        .await?;

    increment_tag_usage_in_tx(txn, tenant_id, &term_ids).await?;

    let now = Utc::now();
    for term_id in term_ids {
        blog_post_tag::ActiveModel {
            post_id: Set(post_id),
            tag_id: Set(term_id),
            tenant_id: Set(tenant_id),
            created_at: Set(now.into()),
        }
        .insert(txn)
        .await?;
    }

    Ok(())
}

async fn increment_tag_usage_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_ids: &[Uuid],
) -> BlogResult<()> {
    use sea_orm::ExprTrait;

    if tag_ids.is_empty() {
        return Ok(());
    }
    let mut unique_ids = tag_ids.to_vec();
    unique_ids.sort_unstable();
    unique_ids.dedup();
    let terms = TaxonomyOwnerReader::load_terms_by_ids_in_tx(
        txn,
        tenant_id,
        TaxonomyTermKind::Tag,
        &unique_ids,
        PLATFORM_FALLBACK_LOCALE,
        None,
    )
    .await?;
    if terms.len() != unique_ids.len() {
        return Err(BlogError::invariant(
            "Blog tag usage increment references a missing or wrong-kind Taxonomy term",
        ));
    }
    for term in terms {
        if term.scope_type == TaxonomyScopeType::Module
            && term.scope_value.as_deref() != Some(BLOG_SCOPE_VALUE)
        {
            return Err(BlogError::invariant(format!(
                "Blog post tag {} references a Taxonomy term outside the Blog/module scope",
                term.id
            )));
        }
        blog_tag_usage::Entity::insert(blog_tag_usage::ActiveModel {
            tenant_id: Set(tenant_id),
            tag_id: Set(term.id),
            canonical_key: Set(term.canonical_key.clone()),
            use_count: Set(1),
        })
        .on_conflict(
            OnConflict::columns([
                blog_tag_usage::Column::TenantId,
                blog_tag_usage::Column::TagId,
            ])
            .values([
                (
                    blog_tag_usage::Column::UseCount,
                    Expr::col(blog_tag_usage::Column::UseCount).add(1),
                ),
                (
                    blog_tag_usage::Column::CanonicalKey,
                    Expr::value(term.canonical_key),
                ),
            ])
            .to_owned(),
        )
        .exec(txn)
        .await
        .map_err(BlogError::from)?;
    }
    Ok(())
}

async fn decrement_tag_usage_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_ids: &[Uuid],
) -> BlogResult<()> {
    use sea_orm::ExprTrait;

    if tag_ids.is_empty() { return Ok(()); }
    let mut unique_ids = tag_ids.to_vec();
    unique_ids.sort_unstable();
    unique_ids.dedup();
    let terms = TaxonomyOwnerReader::load_terms_by_ids_in_tx(
        txn,
        tenant_id,
        TaxonomyTermKind::Tag,
        &unique_ids,
        PLATFORM_FALLBACK_LOCALE,
        None,
    )
    .await?;
    if terms.len() != unique_ids.len() {
        return Err(BlogError::invariant(
            "Blog tag usage decrement references a missing or wrong-kind Taxonomy term",
        ));
    }
    for term in terms {
        if term.scope_type == TaxonomyScopeType::Module
            && term.scope_value.as_deref() != Some(BLOG_SCOPE_VALUE)
        {
            return Err(BlogError::invariant(format!(
                "Blog post tag {} references a Taxonomy term outside the Blog/module scope",
                term.id
            )));
        }
        let updated = blog_tag_usage::Entity::update_many()
            .col_expr(
                blog_tag_usage::Column::UseCount,
                Expr::col(blog_tag_usage::Column::UseCount).sub(1),
            )
            .filter(blog_tag_usage::Column::TenantId.eq(tenant_id))
            .filter(blog_tag_usage::Column::TagId.eq(term.id))
            .filter(blog_tag_usage::Column::UseCount.gt(0))
            .exec(txn)
            .await?;
        if updated.rows_affected != 1 {
            return Err(BlogError::invariant(format!(
                "Blog tag usage projection underflow for Taxonomy term {}",
                term.id
            )));
        }
        if term.scope_type == TaxonomyScopeType::Global {
            blog_tag_usage::Entity::delete_many()
                .filter(blog_tag_usage::Column::TenantId.eq(tenant_id))
                .filter(blog_tag_usage::Column::TagId.eq(term.id))
                .filter(blog_tag_usage::Column::UseCount.eq(0))
                .exec(txn)
                .await?;
        }
    }
    Ok(())
}

pub(crate) async fn remove_post_tag_usage_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> BlogResult<()> {
    let relations = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .all(txn)
        .await?;
    let tag_ids = relations
        .into_iter()
        .map(|relation| relation.tag_id)
        .collect::<Vec<_>>();
    decrement_tag_usage_in_tx(txn, tenant_id, &tag_ids).await
}

pub(crate) async fn initialize_tag_usage_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_id: Uuid,
    canonical_key: &str,
) -> BlogResult<()> {
    blog_tag_usage::Entity::insert(blog_tag_usage::ActiveModel {
        tenant_id: Set(tenant_id),
        tag_id: Set(tag_id),
        canonical_key: Set(canonical_key.to_string()),
        use_count: Set(0),
    })
    .exec(txn)
    .await
    .map_err(BlogError::from)?;
    Ok(())
}

pub(crate) async fn load_post_tags_map(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    post_ids: &[Uuid],
    locale: &str,
    fallback_locale: Option<&str>,
) -> BlogResult<HashMap<Uuid, Vec<String>>> {
    if post_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut tags_by_post = post_ids
        .iter()
        .copied()
        .map(|post_id| (post_id, Vec::new()))
        .collect::<HashMap<_, _>>();

    let relations = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.is_in(post_ids.to_vec()))
        .order_by_asc(blog_post_tag::Column::CreatedAt)
        .all(db)
        .await?;

    if relations.is_empty() {
        return Ok(tags_by_post);
    }

    let term_ids = relations.iter().map(|item| item.tag_id).collect::<Vec<_>>();
    let names = TaxonomyOwnerReader::new(db.clone())
        .load_term_names_strict_for_module(
            tenant_id,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE_VALUE,
            &term_ids,
        )
        .await?;
    let mut locale_chain = vec![locale.to_string()];
    if let Some(fallback_locale) = fallback_locale
        && fallback_locale != locale
    {
        locale_chain.push(fallback_locale.to_string());
    }

    for relation in relations {
        let Some(term_names) = names.get(&relation.tag_id) else {
            return Err(BlogError::invariant(format!(
                "Blog post {} references missing Taxonomy tag {}",
                relation.post_id, relation.tag_id
            )));
        };
        tags_by_post
            .entry(relation.post_id)
            .or_default()
            .push(term_names.resolve_name_for_locale_chain(&locale_chain));
    }

    Ok(tags_by_post)
}

pub(crate) async fn resolve_tag_id_for_posts(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    tag: &str,
    locale: &str,
    fallback_locale: Option<&str>,
) -> BlogResult<Option<Uuid>> {
    TaxonomyService::new(db.clone())
        .resolve_term_id_for_module(
            tenant_id,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE_VALUE,
            locale,
            fallback_locale,
            tag,
        )
        .await
        .map_err(BlogError::from)
}

fn bounded_tag_page_size(value: u64) -> u64 {
    value.clamp(1, MAX_TAGS_PER_PAGE)
}

fn validate_tag_name(name: &str) -> BlogResult<()> {
    if name.trim().is_empty() {
        return Err(BlogError::validation("Tag name cannot be empty"));
    }
    if name.chars().count() > 100 {
        return Err(BlogError::validation(
            "Tag name cannot exceed 100 characters",
        ));
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
        let key = trimmed.to_ascii_lowercase();
        if seen.insert(key) {
            normalized.push(trimmed.to_string());
        }
    }
    normalized
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
    use super::{MAX_TAGS_PER_PAGE, bounded_tag_page_size, validate_tag_name};

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
}
