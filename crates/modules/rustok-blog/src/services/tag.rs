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
        refresh_tag_usage_in_tx(&txn, tenant_id, &[tag_id]).await?;
        publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id).await?;
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

    // Posts with an exhausted/invalid revision are not bumped: they cannot be
    // CAS-edited either, so no concurrency protection is lost, and refusing
    // here would make the tag permanently undeletable because of one such
    // post. The tag delete publishes the reindex event that repairs their
    // search projection. `relation_count` is still observed for diagnostics.
    if relation_count != valid_relation_count {
        tracing::warn!(
            tenant_id = %tenant_id,
            tag_id = %tag_id,
            relation_count,
            valid_relation_count,
            "Blog tag removal skips the revision bump for posts without a valid persisted version"
        );
    }

    let now = Utc::now();
    let updated = blog_post::Entity::update_many()
        .col_expr(
            blog_post::Column::Version,
            Expr::col(blog_post::Column::Version).add(1),
        )
        .col_expr(blog_post::Column::UpdatedAt, Expr::value(now))
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
    for name in &normalized_names {
        validate_tag_name(name)?;
    }

    if normalized_names.is_empty() {
        // Clearing the tag set still rewrites relations and the projection.
        return remove_post_tag_links_in_tx(txn, tenant_id, post_id).await;
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

    validate_post_tag_terms_in_tx(txn, tenant_id, &term_ids, &normalized_locale).await?;
    replace_post_tag_links_in_tx(txn, tenant_id, post_id, &term_ids).await
}

async fn validate_post_tag_terms_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_ids: &[Uuid],
    locale: &str,
) -> BlogResult<()> {
    if tag_ids.is_empty() {
        return Ok(());
    }

    let terms = TaxonomyOwnerReader::load_terms_by_ids_in_tx(
        txn,
        tenant_id,
        TaxonomyTermKind::Tag,
        tag_ids,
        locale,
        Some(PLATFORM_FALLBACK_LOCALE),
    )
    .await?;

    if terms.len() != tag_ids.iter().copied().collect::<HashSet<_>>().len() {
        return Err(BlogError::invariant(
            "Blog post tag mutation references missing or duplicate Taxonomy terms",
        ));
    }

    for term in terms {
        if term.scope_type == TaxonomyScopeType::Module
            && term.scope_value.as_deref() == Some(BLOG_SCOPE_VALUE)
        {
            validate_tag_name(&term.name)?;
        }
    }

    Ok(())
}

/// Recomputes the `blog_tag_usage` projection of `tag_ids` from the relation
/// table inside the caller's transaction.
///
/// The projection is derived state, so it is **recomputed**, never adjusted by
/// `+1`/`-1`: the count is read from `blog_post_tags` after the relation write
/// of the same transaction. That makes the write path self-healing — a
/// projection row lost to a crashed transaction, a manual data fix or a writer
/// outside this service (content orchestration) is repaired by the next
/// mutation instead of turning into an underflow error that blocks every
/// subsequent post save.
///
/// Rows are kept at `use_count = 0` for Blog-owned module terms because
/// `list_tags` pages over this table (a tag with no posts must remain
/// listable); projection rows of global terms are removed at zero, exactly as
/// the previous decrement path did.
async fn refresh_tag_usage_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_ids: &[Uuid],
) -> BlogResult<()> {
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
            "Blog tag usage refresh references a missing or wrong-kind Taxonomy term",
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

        let use_count = blog_post_tag::Entity::find()
            .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
            .filter(blog_post_tag::Column::TagId.eq(term.id))
            .count(txn)
            .await?;
        let use_count = i32::try_from(use_count).unwrap_or(i32::MAX);

        if use_count == 0 && term.scope_type == TaxonomyScopeType::Global {
            blog_tag_usage::Entity::delete_many()
                .filter(blog_tag_usage::Column::TenantId.eq(tenant_id))
                .filter(blog_tag_usage::Column::TagId.eq(term.id))
                .exec(txn)
                .await?;
            continue;
        }

        blog_tag_usage::Entity::insert(blog_tag_usage::ActiveModel {
            tenant_id: Set(tenant_id),
            tag_id: Set(term.id),
            canonical_key: Set(term.canonical_key.clone()),
            use_count: Set(use_count),
        })
        .on_conflict(
            OnConflict::columns([
                blog_tag_usage::Column::TenantId,
                blog_tag_usage::Column::TagId,
            ])
            .update_columns([
                blog_tag_usage::Column::UseCount,
                blog_tag_usage::Column::CanonicalKey,
            ])
            .to_owned(),
        )
        .exec(txn)
        .await
        .map_err(BlogError::from)?;
    }
    Ok(())
}

/// Replaces the whole tag set of a post inside the caller's transaction.
///
/// This is the single write entry point for post-to-tag relations. Blog
/// itself and the content-orchestration bridge both use it, so the relation
/// rows and the `blog_tag_usage` projection can never be written separately.
/// `tag_ids` must already be resolved Taxonomy term ids of the same tenant;
/// duplicates are collapsed.
pub async fn replace_post_tag_links_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    tag_ids: &[Uuid],
) -> BlogResult<()> {
    if tenant_id.is_nil() {
        return Err(BlogError::validation("Tenant ID must not be nil"));
    }
    if post_id.is_nil() {
        return Err(BlogError::validation("Post ID must not be nil"));
    }

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

    let mut unique_tag_ids = tag_ids.to_vec();
    unique_tag_ids.sort_unstable();
    unique_tag_ids.dedup();
    let now = Utc::now();
    for tag_id in &unique_tag_ids {
        blog_post_tag::ActiveModel {
            post_id: Set(post_id),
            tag_id: Set(*tag_id),
            tenant_id: Set(tenant_id),
            created_at: Set(now.into()),
        }
        .insert(txn)
        .await?;
    }

    let mut affected = previous_tag_ids;
    affected.extend(unique_tag_ids);
    refresh_tag_usage_in_tx(txn, tenant_id, &affected).await
}

/// Removes every tag link of a post inside the caller's transaction and
/// refreshes the usage projection of the tags it pointed at.
pub async fn remove_post_tag_links_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> BlogResult<()> {
    if tenant_id.is_nil() {
        return Err(BlogError::validation("Tenant ID must not be nil"));
    }
    if post_id.is_nil() {
        return Err(BlogError::validation("Post ID must not be nil"));
    }

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

    refresh_tag_usage_in_tx(txn, tenant_id, &previous_tag_ids).await
}

pub(crate) async fn initialize_tag_usage_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_id: Uuid,
    canonical_key: &str,
) -> BlogResult<()> {
    // Idempotent by design: a retried create (or a projection row that already
    // exists because the tag was re-created) must not fail on the projection's
    // primary key. A zero-row upsert keeps the existing count untouched.
    blog_tag_usage::Entity::insert(blog_tag_usage::ActiveModel {
        tenant_id: Set(tenant_id),
        tag_id: Set(tag_id),
        canonical_key: Set(canonical_key.to_string()),
        use_count: Set(0),
    })
    .on_conflict(
        OnConflict::columns([
            blog_tag_usage::Column::TenantId,
            blog_tag_usage::Column::TagId,
        ])
        .do_nothing()
        .to_owned(),
    )
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
