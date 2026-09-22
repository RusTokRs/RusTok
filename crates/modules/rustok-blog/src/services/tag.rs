use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, JoinType, QueryFilter, QueryOrder, QuerySelect, RelationTrait, TransactionTrait,
    sea_query::Expr,
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
use crate::entities::{blog_post, blog_post_tag};
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
        let use_count = self
            .count_tag_usage_map(tenant_id, &[tag_id])
            .await?
            .remove(&tag_id)
            .unwrap_or_default();

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
            .count_tag_usage_map(tenant_id, &[tag_id])
            .await?
            .remove(&tag_id)
            .unwrap_or_default();

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

        let terms = self.list_visible_terms(tenant_id, &locale).await?;
        if terms.is_empty() {
            return Ok((Vec::new(), 0));
        }

        let term_ids = terms.iter().map(|term| term.id).collect::<Vec<_>>();
        let counts = self.count_tag_usage_map(tenant_id, &term_ids).await?;

        let mut sortable = terms
            .into_iter()
            .map(|term| {
                let use_count = counts.get(&term.id).copied().unwrap_or_default();
                (use_count, term)
            })
            .collect::<Vec<_>>();
        sortable.sort_by(|(left_count, left_term), (right_count, right_term)| {
            right_count
                .cmp(left_count)
                .then_with(|| left_term.canonical_key.cmp(&right_term.canonical_key))
        });

        let total = sortable.len() as u64;
        let offset = tag_page_offset(page, per_page);
        let items = sortable
            .into_iter()
            .skip(offset)
            .take(per_page as usize)
            .map(|(use_count, term)| TagListItem {
                id: term.id,
                locale: locale.clone(),
                effective_locale: term.effective_locale,
                name: term.name,
                slug: term.slug,
                use_count,
                created_at: term.created_at,
            })
            .collect();

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

    async fn list_visible_terms(
        &self,
        tenant_id: Uuid,
        locale: &str,
    ) -> BlogResult<Vec<TaxonomyOwnerTerm>> {
        let reader = TaxonomyOwnerReader::new(self.db.clone());
        let mut terms = reader
            .load_scoped_terms(
                tenant_id,
                TaxonomyTermKind::Tag,
                TaxonomyScopeType::Module,
                Some(BLOG_SCOPE_VALUE),
                None,
                locale,
                None,
            )
            .await?;

        let module_term_ids = terms.iter().map(|term| term.id).collect::<HashSet<_>>();
        let used_term_ids = blog_post_tag::Entity::find()
            .join(JoinType::InnerJoin, blog_post_tag::Relation::Post.def())
            .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|relation| relation.tag_id)
            .filter(|tag_id| !module_term_ids.contains(tag_id))
            .collect::<HashSet<_>>();

        if !used_term_ids.is_empty() {
            let used_term_ids = used_term_ids.into_iter().collect::<Vec<_>>();
            let mut global_terms = reader
                .load_scoped_terms(
                    tenant_id,
                    TaxonomyTermKind::Tag,
                    TaxonomyScopeType::Global,
                    None,
                    Some(&used_term_ids),
                    locale,
                    None,
                )
                .await?;
            terms.append(&mut global_terms);
        }

        Ok(terms)
    }

    async fn count_tag_usage_map(
        &self,
        tenant_id: Uuid,
        tag_ids: &[Uuid],
    ) -> BlogResult<HashMap<Uuid, i32>> {
        if tag_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let relations = blog_post_tag::Entity::find()
            .join(JoinType::InnerJoin, blog_post_tag::Relation::Post.def())
            .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post_tag::Column::TagId.is_in(tag_ids.to_vec()))
            .all(&self.db)
            .await?;

        let mut counts = HashMap::new();
        for relation in relations {
            *counts.entry(relation.tag_id).or_insert(0) += 1;
        }
        Ok(counts)
    }
}

async fn bump_posts_for_tag_relation_removal_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tag_id: Uuid,
) -> BlogResult<()> {
    let relations = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::TagId.eq(tag_id))
        .order_by_asc(blog_post_tag::Column::PostId)
        .all(txn)
        .await?;

    let now = Utc::now();
    for relation in relations {
        let post = blog_post::Entity::find_by_id(relation.post_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
            .ok_or_else(|| {
                BlogError::invariant(format!(
                    "Blog post-tag relation {} references missing post {}",
                    relation.tag_id, relation.post_id
                ))
            })?;

        if post.version <= 0 {
            return Err(BlogError::invariant(format!(
                "Blog post {} has invalid persisted version {}",
                post.id, post.version
            )));
        }

        let next_version = post
            .version
            .checked_add(1)
            .filter(|next| *next > 0)
            .ok_or_else(|| {
                BlogError::invariant(format!(
                    "Blog post version {} is invalid or exhausted",
                    post.version
                ))
            })?;

        let updated = blog_post::Entity::update_many()
            .col_expr(
                blog_post::Column::Version,
                Expr::value(next_version),
            )
            .col_expr(
                blog_post::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(blog_post::Column::Id.eq(post.id))
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Version.eq(post.version))
            .exec(txn)
            .await?;

        if updated.rows_affected != 1 {
            return Err(BlogError::conflict(format!(
                "Blog post {} changed before Tag relation invalidation could commit",
                post.id
            )));
        }
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

    blog_post_tag::Entity::delete_many()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .exec(txn)
        .await?;

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
}

fn bounded_tag_page_size(value: u64) -> u64 {
    value.clamp(1, MAX_TAGS_PER_PAGE)
}

fn tag_page_offset(page: u64, per_page: u64) -> usize {
    let offset = page.saturating_sub(1).saturating_mul(per_page);
    usize::try_from(offset).unwrap_or(usize::MAX)
}

fn validate_tag_name(name: &str) -> BlogResult<()> {
    if name.trim().is_empty() {
        return Err(BlogError::validation("Tag name cannot be empty"));
    }
    if name.len() > 100 {
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
    use super::{MAX_TAGS_PER_PAGE, bounded_tag_page_size, tag_page_offset};

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
    fn tag_page_offset_saturates_without_arithmetic_overflow() {
        assert_eq!(tag_page_offset(1, 20), 0);
        assert_eq!(tag_page_offset(2, 20), 20);
        assert_eq!(tag_page_offset(u64::MAX, MAX_TAGS_PER_PAGE), usize::MAX);
    }
}
