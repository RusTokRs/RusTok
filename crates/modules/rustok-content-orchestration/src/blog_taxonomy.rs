use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError};
use rustok_blog::ports::taxonomy::{
    BlogCategoryDeleteCleanupPort, BlogTaxonomyCategoryLocaleCopy, BlogTaxonomyCategorySnapshot,
    BlogTaxonomyConnection, BlogTaxonomyCreateTagInput, BlogTaxonomyCreatedTag,
    BlogTaxonomyPlacement, BlogTaxonomyPort, BlogTaxonomySyncCategoryInput,
    BlogTaxonomyTagSnapshot, BlogTaxonomyUpdateTagInput,
};
use rustok_blog::{BlogError, BlogPublicError, BlogResult, ports::BlogTaxonomyTranslationKind};
use rustok_core::SecurityContext;
use rustok_taxonomy::{
    ModuleTermCreateInput, ModuleTermUpdateInput, SyncModuleCategoryInput,
    TaxonomyCategoryDeleteCleanupPort, TaxonomyError, TaxonomyModuleTermTranslationOwner,
    TaxonomyModuleTermTranslationOwnerRegistry, TaxonomyOwnerCategoryReader, TaxonomyOwnerReader,
    TaxonomyOwnerTerm, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
    TaxonomyTermTranslationPolicy,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

#[cfg(test)]
mod tests;

const BLOG_SCOPE: &str = "blog";

/// Attach the canonical Taxonomy owner to Blog in a shared database runtime.
pub fn in_process_blog_taxonomy_port(db: DatabaseConnection) -> Arc<dyn BlogTaxonomyPort> {
    Arc::new(BlogTaxonomyAdapter(TaxonomyService::new(db)))
}

pub fn register_blog_translation_owner(
    registry: &mut TaxonomyModuleTermTranslationOwnerRegistry,
    db: DatabaseConnection,
) -> Result<(), TaxonomyError> {
    registry.register(BlogTranslationOwner(db))
}

struct BlogTranslationOwner(DatabaseConnection);

#[async_trait]
impl TaxonomyModuleTermTranslationOwner for BlogTranslationOwner {
    fn module_slug(&self) -> &str {
        BLOG_SCOPE
    }

    fn translation_policy(&self, kind: TaxonomyTermKind) -> TaxonomyTermTranslationPolicy {
        let (name_max, slug_max, description_max) = blog_translation_kind(kind).field_limits();
        TaxonomyTermTranslationPolicy {
            name_max,
            slug_max,
            description_max,
        }
    }

    async fn is_enabled(&self, tenant_id: Uuid) -> Result<bool, PortError> {
        rustok_api::is_tenant_module_enabled(&self.0, tenant_id, BLOG_SCOPE)
            .await
            .map_err(|error| {
                tracing::error!(%error, %tenant_id, "Blog translation enablement lookup failed");
                PortError::unavailable(
                    "blog.translation_owner_unavailable",
                    "Blog translation owner is unavailable",
                )
            })
    }

    fn authorize(
        &self,
        context: &PortContext,
        _tenant_id: Uuid,
        kind: TaxonomyTermKind,
        _term_id: Uuid,
        action: Action,
    ) -> Result<(), PortError> {
        blog_translation_kind(kind)
            .authorize(&SecurityContext::try_from_port_context(context)?, action)
            .map_err(blog_translation_error)
    }

    async fn on_translation_applied_in_tx(
        &self,
        transaction: &DatabaseTransaction,
        context: &PortContext,
        tenant_id: Uuid,
        kind: TaxonomyTermKind,
        term_id: Uuid,
    ) -> Result<(), PortError> {
        blog_translation_kind(kind)
            .record_applied_in_tx(
                transaction,
                tenant_id,
                term_id,
                &SecurityContext::try_from_port_context(context)?,
            )
            .await
            .map_err(blog_translation_error)
    }
}

fn blog_translation_kind(kind: TaxonomyTermKind) -> BlogTaxonomyTranslationKind {
    match kind {
        TaxonomyTermKind::Category => BlogTaxonomyTranslationKind::Category,
        TaxonomyTermKind::Tag => BlogTaxonomyTranslationKind::Tag,
    }
}

fn blog_translation_error(error: BlogError) -> PortError {
    let result = BlogPublicError::from(error);
    match result.status {
        400 | 422 => PortError::validation(result.code, result.message),
        403 => PortError::forbidden(result.code, result.message),
        404 => PortError::not_found(result.code, result.message),
        409 => PortError::conflict(result.code, result.message),
        _ => PortError::invariant_violation(result.code, result.message),
    }
}

/// Give Blog's Category delete transaction access to the host's capability cleanup.
pub fn blog_category_delete_cleanup(
    cleanup: Arc<dyn TaxonomyCategoryDeleteCleanupPort>,
) -> Arc<dyn BlogCategoryDeleteCleanupPort> {
    Arc::new(BlogCapabilityCleanup(cleanup))
}

struct BlogTaxonomyAdapter(TaxonomyService);

#[async_trait]
impl BlogTaxonomyPort for BlogTaxonomyAdapter {
    async fn lock_category_hierarchy_writer_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> BlogResult<()> {
        rustok_taxonomy::lock_category_hierarchy_writer_in_tx(txn, tenant_id)
            .await
            .map_err(blog_error)
    }

    async fn load_scoped_categories(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: Option<&[Uuid]>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<Vec<BlogTaxonomyCategorySnapshot>> {
        match conn {
            BlogTaxonomyConnection::Connection(conn) => {
                load_categories(conn, tenant_id, category_ids, locale, fallback_locale).await
            }
            BlogTaxonomyConnection::Transaction(txn) => {
                load_categories(txn, tenant_id, category_ids, locale, fallback_locale).await
            }
        }
    }

    async fn load_category_locale_copy_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
    ) -> BlogResult<Option<BlogTaxonomyCategoryLocaleCopy>> {
        let copy = rustok_taxonomy::load_module_category_locale_copy_in_tx(
            txn,
            tenant_id,
            category_id,
            BLOG_SCOPE,
            locale,
        )
        .await
        .map_err(blog_error)?;
        Ok(copy.map(|copy| BlogTaxonomyCategoryLocaleCopy {
            name: copy.name,
            slug: copy.slug,
            description: copy.description,
        }))
    }

    async fn sync_category_copy_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: BlogTaxonomySyncCategoryInput,
    ) -> BlogResult<()> {
        rustok_taxonomy::sync_module_category_copy_in_tx(
            txn,
            tenant_id,
            SyncModuleCategoryInput {
                category_id: input.category_id,
                module_scope: BLOG_SCOPE.to_owned(),
                canonical_key: format!("blog-category-{}", input.category_id),
                locale: input.locale,
                name: input.name,
                slug: input.slug,
                aliases: Vec::new(),
                description: input.description,
                parent_id: input.parent_id,
                position: input.position,
                icon_key: None,
                color: None,
            },
        )
        .await
        .map_err(blog_error)?;
        Ok(())
    }

    async fn move_category_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        new_parent_id: Option<Uuid>,
        position: Option<i32>,
    ) -> BlogResult<Vec<BlogTaxonomyPlacement>> {
        let position =
            position.ok_or_else(|| BlogError::validation("Category position is required"))?;
        let placements = rustok_taxonomy::move_module_category_in_tx(
            txn,
            tenant_id,
            category_id,
            BLOG_SCOPE,
            new_parent_id,
            position,
        )
        .await
        .map_err(blog_error)?;
        Ok(placements
            .into_iter()
            .map(|placement| BlogTaxonomyPlacement {
                category_id: placement.term_id,
                parent_id: placement.parent_id,
                position: placement.position,
            })
            .collect())
    }

    async fn reorder_category_siblings_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        parent_id: Option<Uuid>,
        ordered_category_ids: &[Uuid],
    ) -> BlogResult<()> {
        rustok_taxonomy::reorder_module_category_siblings_in_tx(
            txn,
            tenant_id,
            BLOG_SCOPE,
            parent_id,
            ordered_category_ids,
        )
        .await
        .map_err(blog_error)
    }

    async fn delete_category_placement_and_compact_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> BlogResult<()> {
        rustok_taxonomy::delete_module_category_placement_and_compact_in_tx(
            txn,
            tenant_id,
            category_id,
            BLOG_SCOPE,
        )
        .await
        .map_err(blog_error)
    }

    async fn delete_module_category_with_cleanup(
        &self,
        conn: &DatabaseConnection,
        tenant_id: Uuid,
        category_id: Uuid,
        _security: &SecurityContext,
        cleanup: Arc<dyn BlogCategoryDeleteCleanupPort>,
    ) -> BlogResult<()> {
        TaxonomyService::new(conn.clone())
            .delete_module_category_with_cleanup(
                tenant_id,
                category_id,
                BLOG_SCOPE,
                &TaxonomyCleanup(cleanup),
            )
            .await
            .map_err(|err| err.0)
    }

    async fn load_tags_by_ids(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        tag_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<HashMap<Uuid, BlogTaxonomyTagSnapshot>> {
        Ok(self
            .load_scoped_tags(conn, tenant_id, Some(tag_ids), locale, fallback_locale)
            .await?
            .into_iter()
            .map(|tag| (tag.id, tag))
            .collect())
    }

    async fn load_scoped_tags(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        tag_ids: Option<&[Uuid]>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<Vec<BlogTaxonomyTagSnapshot>> {
        let terms = match conn {
            BlogTaxonomyConnection::Connection(conn) => {
                load_tags(conn, tenant_id, tag_ids, locale, fallback_locale).await?
            }
            BlogTaxonomyConnection::Transaction(txn) => {
                load_tags(txn, tenant_id, tag_ids, locale, fallback_locale).await?
            }
        };
        Ok(terms
            .into_iter()
            .map(|term| tag_snapshot(tenant_id, term))
            .collect())
    }

    async fn resolve_tag_id_for_module(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        tag: &str,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<Option<Uuid>> {
        match conn {
            BlogTaxonomyConnection::Connection(conn) => {
                resolve_tag(conn, tenant_id, tag, locale, fallback_locale).await
            }
            BlogTaxonomyConnection::Transaction(txn) => {
                resolve_tag(txn, tenant_id, tag, locale, fallback_locale).await
            }
        }
        .map_err(blog_error)
    }

    async fn find_all_valid_tag_ids(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
    ) -> BlogResult<HashSet<Uuid>> {
        let kind = TaxonomyTermKind::Tag;
        match conn {
            BlogTaxonomyConnection::Connection(conn) => {
                TaxonomyOwnerReader::load_term_ids_for_module_in(conn, tenant_id, kind, BLOG_SCOPE)
                    .await
            }
            BlogTaxonomyConnection::Transaction(txn) => {
                TaxonomyOwnerReader::load_term_ids_for_module_in(txn, tenant_id, kind, BLOG_SCOPE)
                    .await
            }
        }
        .map_err(blog_error)
    }

    async fn ensure_module_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        tag_names: &[String],
        allow_create: bool,
    ) -> BlogResult<Vec<Uuid>> {
        self.0
            .ensure_module_terms_for_owner_in_tx(
                txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                BLOG_SCOPE,
                locale,
                tag_names,
                allow_create,
            )
            .await
            .map_err(blog_error)
    }

    async fn create_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: BlogTaxonomyCreateTagInput,
    ) -> BlogResult<BlogTaxonomyCreatedTag> {
        let locale = input.locale;
        let id = self
            .0
            .create_module_term_in_tx(
                txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                BLOG_SCOPE,
                ModuleTermCreateInput {
                    locale: locale.clone(),
                    name: input.name,
                    slug: input.slug,
                    canonical_key: None,
                },
            )
            .await
            .map_err(blog_error)?;
        let term = read_written_tag(txn, tenant_id, id, &locale).await?;
        Ok(BlogTaxonomyCreatedTag {
            id,
            canonical_key: term.canonical_key,
        })
    }

    async fn update_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        tag_id: Uuid,
        security: &SecurityContext,
        input: BlogTaxonomyUpdateTagInput,
    ) -> BlogResult<BlogTaxonomyTagSnapshot> {
        let updated = rustok_taxonomy::update_module_term_in_tx(
            txn,
            tenant_id,
            tag_id,
            security,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE,
            ModuleTermUpdateInput {
                locale: input.locale,
                name: input.name,
                slug: input.slug,
            },
        )
        .await
        .map_err(blog_error)?;
        let term = read_written_tag(txn, tenant_id, tag_id, &updated.locale).await?;
        Ok(tag_snapshot(tenant_id, term))
    }

    async fn delete_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        tag_id: Uuid,
        security: &SecurityContext,
    ) -> BlogResult<()> {
        rustok_taxonomy::delete_module_term_in_tx(
            txn,
            tenant_id,
            tag_id,
            security,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE,
        )
        .await
        .map_err(blog_error)
    }

    async fn lock_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        tag_id: Uuid,
    ) -> BlogResult<()> {
        rustok_taxonomy::lock_module_term_in_tx(
            txn,
            tenant_id,
            tag_id,
            TaxonomyTermKind::Tag,
            BLOG_SCOPE,
        )
        .await
        .map_err(blog_error)
    }
}

async fn load_categories<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    category_ids: Option<&[Uuid]>,
    locale: &str,
    fallback_locale: Option<&str>,
) -> BlogResult<Vec<BlogTaxonomyCategorySnapshot>> {
    let categories = TaxonomyOwnerCategoryReader::load_scoped_categories_in_strict(
        conn,
        tenant_id,
        TaxonomyScopeType::Module,
        Some(BLOG_SCOPE),
        category_ids,
        locale,
        fallback_locale,
    )
    .await
    .map_err(blog_error)?;
    Ok(categories
        .into_iter()
        .map(|category| BlogTaxonomyCategorySnapshot {
            id: category.id,
            parent_id: category.parent_id,
            canonical_key: category.canonical_key,
            slug: category.slug,
            name: category.name,
            description: category.description,
            position: category.position,
            available_locales: category.available_locales,
            requested_locale: category.requested_locale,
            effective_locale: category.effective_locale,
        })
        .collect())
}

async fn load_tags<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    tag_ids: Option<&[Uuid]>,
    locale: &str,
    fallback_locale: Option<&str>,
) -> BlogResult<Vec<TaxonomyOwnerTerm>> {
    TaxonomyOwnerReader::load_terms_for_module_in(
        conn,
        tenant_id,
        TaxonomyTermKind::Tag,
        BLOG_SCOPE,
        tag_ids,
        locale,
        fallback_locale,
    )
    .await
    .map_err(blog_error)
}

async fn resolve_tag<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    tag: &str,
    locale: &str,
    fallback_locale: Option<&str>,
) -> Result<Option<Uuid>, TaxonomyError> {
    TaxonomyService::resolve_term_id_for_module_in(
        conn,
        tenant_id,
        TaxonomyTermKind::Tag,
        BLOG_SCOPE,
        locale,
        fallback_locale,
        tag,
    )
    .await
}

async fn read_written_tag(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    id: Uuid,
    locale: &str,
) -> BlogResult<TaxonomyOwnerTerm> {
    TaxonomyOwnerReader::load_scoped_terms_in(
        txn,
        tenant_id,
        TaxonomyTermKind::Tag,
        TaxonomyScopeType::Module,
        Some(BLOG_SCOPE),
        Some(&[id]),
        locale,
        None,
    )
    .await
    .map_err(blog_error)?
    .into_iter()
    .next()
    .filter(|term| term.requested_locale == term.effective_locale)
    .ok_or_else(|| BlogError::invariant("Taxonomy did not return the written Blog tag and locale"))
}

fn tag_snapshot(tenant_id: Uuid, term: TaxonomyOwnerTerm) -> BlogTaxonomyTagSnapshot {
    BlogTaxonomyTagSnapshot {
        id: term.id,
        tenant_id,
        canonical_key: term.canonical_key,
        name: term.name,
        slug: term.slug,
        scope_type: term.scope_type.to_string(),
        scope_value: term.scope_value,
        locale: term.requested_locale,
        effective_locale: term.effective_locale,
        created_at: term.created_at,
    }
}

fn blog_error(error: TaxonomyError) -> BlogError {
    match error {
        TaxonomyError::Database(error) => BlogError::Database(error),
        TaxonomyError::Internal(message) => BlogError::invariant(message),
        TaxonomyError::TermNotFound(id) => BlogError::TaxonomyTermNotFound(id),
        TaxonomyError::Validation(message) => BlogError::validation(message),
        TaxonomyError::Forbidden(message) => BlogError::forbidden(message),
        error @ (TaxonomyError::DuplicateCanonicalKey(_)
        | TaxonomyError::DuplicateSlug(_)
        | TaxonomyError::DuplicateAlias(_)
        | TaxonomyError::Conflict(_)
        | TaxonomyError::TranslationRevisionExhausted { .. }) => {
            BlogError::conflict(error.to_string())
        }
    }
}

struct BlogDeleteError(BlogError);

impl From<TaxonomyError> for BlogDeleteError {
    fn from(error: TaxonomyError) -> Self {
        Self(blog_error(error))
    }
}

struct TaxonomyCleanup(Arc<dyn BlogCategoryDeleteCleanupPort>);

#[async_trait]
impl TaxonomyCategoryDeleteCleanupPort<BlogDeleteError> for TaxonomyCleanup {
    async fn cleanup_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> Result<(), BlogDeleteError> {
        self.0
            .cleanup_in_tx(txn, tenant_id, category_id)
            .await
            .map_err(BlogDeleteError)
    }
}

struct BlogCapabilityCleanup(Arc<dyn TaxonomyCategoryDeleteCleanupPort>);

#[async_trait]
impl BlogCategoryDeleteCleanupPort for BlogCapabilityCleanup {
    async fn cleanup_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> BlogResult<()> {
        self.0
            .cleanup_in_tx(txn, tenant_id, category_id)
            .await
            .map_err(blog_error)
    }
}
