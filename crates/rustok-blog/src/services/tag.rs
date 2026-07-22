use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection,
    DatabaseTransaction, EntityTrait, JoinType, QueryFilter, QueryOrder, QuerySelect,
    RelationTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource};
use rustok_content::{normalize_locale_code, resolve_by_locale_with_fallback};
use rustok_core::SecurityContext;
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
    UpdateTaxonomyTermInput,
    entities::{taxonomy_term, taxonomy_term_alias, taxonomy_term_translation},
};

use crate::dto::{CreateTagInput, ListTagsFilter, TagListItem, TagResponse, UpdateTagInput};
use crate::entities::{blog_post, blog_post_tag};
use crate::error::{BlogError, BlogResult};
use crate::services::rbac::{enforce_owned_scope, enforce_scope};

const BLOG_SCOPE_VALUE: &str = "blog";

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

        Ok(TaxonomyService::new(self.db.clone())
            .create_term(
                tenant_id,
                security,
                CreateTaxonomyTermInput {
                    kind: TaxonomyTermKind::Tag,
                    scope_type: TaxonomyScopeType::Module,
                    scope_value: Some(BLOG_SCOPE_VALUE.to_string()),
                    locale: normalize_locale(&input.locale)?,
                    name: input.name,
                    slug: input.slug,
                    canonical_key: None,
                    description: None,
                    aliases: vec![],
                },
            )
            .await?)
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
        self.find_visible_term(tenant_id, tag_id).await?;

        let locale = normalize_locale(locale)?;
        let term = TaxonomyService::new(self.db.clone())
            .get_term(
                tenant_id,
                security,
                tag_id,
                &locale,
                Some(PLATFORM_FALLBACK_LOCALE),
            )
            .await?;
        let use_count = self
            .count_tag_usage_map(tenant_id, &[tag_id])
            .await?
            .remove(&tag_id)
            .unwrap_or_default();

        Ok(to_tag_response(term, use_count))
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_tag(
        &self,
        tenant_id: Uuid,
        tag_id: Uuid,
        security: SecurityContext,
        input: UpdateTagInput,
    ) -> BlogResult<TagResponse> {
        let term = self.find_visible_term(tenant_id, tag_id).await?;
        enforce_owned_scope(&security, Resource::Tags, Action::Update, term.id)?;
        ensure_module_owned_term(&term)?;

        let locale = normalize_locale(&input.locale)?;
        let term = TaxonomyService::new(self.db.clone())
            .update_term(
                tenant_id,
                tag_id,
                security,
                UpdateTaxonomyTermInput {
                    locale: locale.clone(),
                    name: input.name,
                    slug: input.slug,
                    description: None,
                    status: None,
                    aliases: None,
                },
            )
            .await?;
        let use_count = self
            .count_tag_usage_map(tenant_id, &[tag_id])
            .await?
            .remove(&tag_id)
            .unwrap_or_default();

        Ok(to_tag_response(term, use_count))
    }

    #[instrument(skip(self, security))]
    pub async fn delete_tag(
        &self,
        tenant_id: Uuid,
        tag_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        let term = self.find_visible_term(tenant_id, tag_id).await?;
        enforce_owned_scope(&security, Resource::Tags, Action::Delete, term.id)?;
        ensure_module_owned_term(&term)?;

        blog_post_tag::Entity::delete_many()
            .filter(blog_post_tag::Column::TagId.eq(tag_id))
            .exec(&self.db)
            .await?;

        TaxonomyService::new(self.db.clone())
            .delete_term(tenant_id, tag_id, security)
            .await?;
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
        let per_page = filter.per_page.max(1);

        let terms = self.list_visible_terms(tenant_id).await?;
        if terms.is_empty() {
            return Ok((Vec::new(), 0));
        }

        let term_ids = terms.iter().map(|term| term.id).collect::<Vec<_>>();
        let counts = self.count_tag_usage_map(tenant_id, &term_ids).await?;
        let translations_by_tag = load_translations_map(&self.db, &term_ids).await?;

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
        let offset = ((page - 1) * per_page) as usize;
        let items = sortable
            .into_iter()
            .skip(offset)
            .take(per_page as usize)
            .map(|(use_count, term)| {
                let translations = translations_by_tag
                    .get(&term.id)
                    .cloned()
                    .unwrap_or_default();
                let translation_refs = translations.iter().collect::<Vec<_>>();
                let (translation, effective_locale) =
                    resolve_translation_with_fallback(&translation_refs, &locale, None);

                TagListItem {
                    id: term.id,
                    locale: locale.clone(),
                    effective_locale,
                    name: translation
                        .map(|translation| translation.name.clone())
                        .unwrap_or_else(|| term.canonical_key.clone()),
                    slug: translation
                        .map(|translation| translation.slug.clone())
                        .unwrap_or_else(|| term.canonical_key.clone()),
                    use_count,
                    created_at: term.created_at.into(),
                }
            })
            .collect();

        Ok((items, total))
    }

    async fn find_visible_term(
        &self,
        tenant_id: Uuid,
        tag_id: Uuid,
    ) -> BlogResult<taxonomy_term::Model> {
        taxonomy_term::Entity::find_by_id(tag_id)
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
            .filter(blog_tag_scope_condition())
            .one(&self.db)
            .await?
            .ok_or_else(|| BlogError::tag_not_found(tag_id))
    }

    async fn list_visible_terms(&self, tenant_id: Uuid) -> BlogResult<Vec<taxonomy_term::Model>> {
        let mut terms = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
            .filter(module_owned_scope_condition())
            .all(&self.db)
            .await?;

        let module_term_ids = terms.iter().map(|term| term.id).collect::<HashSet<_>>();
        let used_term_ids = blog_post_tag::Entity::find()
            .join(JoinType::InnerJoin, blog_post_tag::Relation::Post.def())
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|relation| relation.tag_id)
            .filter(|tag_id| !module_term_ids.contains(tag_id))
            .collect::<HashSet<_>>();

        if !used_term_ids.is_empty() {
            let mut global_terms = taxonomy_term::Entity::find()
                .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
                .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
                .filter(
                    taxonomy_term::Column::Id.is_in(used_term_ids.into_iter().collect::<Vec<_>>()),
                )
                .filter(global_scope_condition())
                .all(&self.db)
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

pub(crate) async fn sync_post_tags_in_tx(
    db: &DatabaseConnection,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    tag_names: &[String],
    locale: &str,
) -> BlogResult<()> {
    let normalized_locale = normalize_locale(locale)?;
    let normalized_names = normalize_tag_names(tag_names);

    blog_post_tag::Entity::delete_many()
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .exec(txn)
        .await?;

    if normalized_names.is_empty() {
        return Ok(());
    }

    let term_ids = TaxonomyService::new(db.clone())
        .ensure_terms_for_module_in_tx(
            txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE_VALUE,
            &normalized_locale,
            &normalized_names,
        )
        .await?;

    let now = Utc::now();
    for term_id in term_ids {
        blog_post_tag::ActiveModel {
            post_id: Set(post_id),
            tag_id: Set(term_id),
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

    let relations = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::PostId.is_in(post_ids.to_vec()))
        .order_by_asc(blog_post_tag::Column::CreatedAt)
        .all(db)
        .await?;

    if relations.is_empty() {
        return Ok(HashMap::new());
    }

    let term_ids = relations.iter().map(|item| item.tag_id).collect::<Vec<_>>();
    let names = TaxonomyService::new(db.clone())
        .resolve_term_names(tenant_id, &term_ids, locale, fallback_locale)
        .await?;

    let mut tags_by_post: HashMap<Uuid, Vec<String>> = HashMap::new();
    for relation in relations {
        if let Some(name) = names.get(&relation.tag_id) {
            tags_by_post
                .entry(relation.post_id)
                .or_default()
                .push(name.clone());
        }
    }

    Ok(tags_by_post)
}

pub(crate) async fn find_post_ids_by_tag(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    tag: &str,
) -> BlogResult<Vec<Uuid>> {
    let normalized_slug = normalize_tag_slug(tag);
    if normalized_slug.is_empty() {
        return Ok(Vec::new());
    }

    let mut tag_ids = taxonomy_term_translation::Entity::find()
        .join(
            JoinType::InnerJoin,
            taxonomy_term_translation::Relation::Term.def(),
        )
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_translation::Column::Slug.eq(&normalized_slug))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
        .filter(blog_tag_scope_condition())
        .all(db)
        .await?
        .into_iter()
        .map(|translation| translation.term_id)
        .collect::<Vec<_>>();

    let alias_ids = taxonomy_term_alias::Entity::find()
        .join(
            JoinType::InnerJoin,
            taxonomy_term_alias::Relation::Term.def(),
        )
        .filter(taxonomy_term_alias::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_alias::Column::Slug.eq(&normalized_slug))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
        .filter(blog_tag_scope_condition())
        .all(db)
        .await?
        .into_iter()
        .map(|alias| alias.term_id)
        .collect::<Vec<_>>();
    tag_ids.extend(alias_ids);
    tag_ids.sort();
    tag_ids.dedup();

    if tag_ids.is_empty() {
        return Ok(Vec::new());
    }

    let relations = blog_post_tag::Entity::find()
        .join(JoinType::InnerJoin, blog_post_tag::Relation::Post.def())
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::TagId.is_in(tag_ids))
        .all(db)
        .await?;

    let mut seen = HashSet::new();
    Ok(relations
        .into_iter()
        .filter_map(|relation| {
            if seen.insert(relation.post_id) {
                Some(relation.post_id)
            } else {
                None
            }
        })
        .collect())
}

async fn load_translations_map(
    db: &DatabaseConnection,
    term_ids: &[Uuid],
) -> BlogResult<HashMap<Uuid, Vec<taxonomy_term_translation::Model>>> {
    if term_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let translations = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TermId.is_in(term_ids.to_vec()))
        .all(db)
        .await?;
    let mut map = HashMap::new();
    for translation in translations {
        map.entry(translation.term_id)
            .or_insert_with(Vec::new)
            .push(translation);
    }
    Ok(map)
}

fn ensure_module_owned_term(term: &taxonomy_term::Model) -> BlogResult<()> {
    if term.scope_type == TaxonomyScopeType::Module && term.scope_value == BLOG_SCOPE_VALUE {
        return Ok(());
    }

    Err(BlogError::forbidden(
        "Global taxonomy tags must be managed through rustok-taxonomy",
    ))
}

fn blog_tag_scope_condition() -> Condition {
    Condition::any()
        .add(module_owned_scope_condition())
        .add(global_scope_condition())
}

fn module_owned_scope_condition() -> Condition {
    Condition::all()
        .add(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .add(taxonomy_term::Column::ScopeValue.eq(BLOG_SCOPE_VALUE))
}

fn global_scope_condition() -> Condition {
    Condition::all()
        .add(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Global))
        .add(taxonomy_term::Column::ScopeValue.eq(""))
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

fn normalize_tag_slug(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            normalized.push('-');
            previous_dash = true;
        }
    }
    normalized.trim_matches('-').to_string()
}

fn resolve_translation_with_fallback<'a>(
    translations: &[&'a taxonomy_term_translation::Model],
    locale: &str,
    fallback_locale: Option<&str>,
) -> (Option<&'a taxonomy_term_translation::Model>, String) {
    let resolved =
        resolve_by_locale_with_fallback(translations, locale, fallback_locale, |translation| {
            translation.locale.as_str()
        });
    (resolved.item.copied(), resolved.effective_locale)
}

fn to_tag_response(term: rustok_taxonomy::TaxonomyTermResponse, use_count: i32) -> TagResponse {
    TagResponse {
        id: term.id,
        tenant_id: term.tenant_id,
        locale: term.requested_locale,
        effective_locale: term.effective_locale,
        name: term.name,
        slug: term.slug,
        use_count,
        created_at: term.created_at,
    }
}
