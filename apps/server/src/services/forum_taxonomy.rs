//! In-process Forum -> Taxonomy adapter. Only the host depends on both modules.
//! All persistence stays behind Taxonomy's owner API and on the caller's connection.
use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use rustok_forum::{
    FORUM_MODULE_SLUG, ForumError, ForumTaxonomyCategoryLocaleCopy, ForumTaxonomyCategorySnapshot,
    ForumTaxonomyConnection, ForumTaxonomyPlacement, ForumTaxonomyPort, ForumTaxonomyRouteMatch,
    ForumTaxonomySyncCategoryInput,
};
use rustok_taxonomy::{
    SyncModuleCategoryInput, TaxonomyError, TaxonomyOwnerCategory, TaxonomyOwnerCategoryReader,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

/// Distribution selection, not compile-time availability alone, enables the adapter.
/// A Forum-only registry deliberately retains the isolated no-op profile.
pub(crate) fn register(
    extensions: &mut rustok_core::ModuleRuntimeExtensions,
    registry: &rustok_core::ModuleRegistry,
    db: DatabaseConnection,
) {
    if registry.contains(FORUM_MODULE_SLUG) && registry.contains("taxonomy") {
        extensions.insert(ServerForumTaxonomyPort::shared(db));
    }
}

pub(crate) struct ServerForumTaxonomyPort {
    taxonomy: TaxonomyService,
}

impl ServerForumTaxonomyPort {
    pub(crate) fn shared(db: DatabaseConnection) -> Arc<dyn ForumTaxonomyPort> {
        Arc::new(Self {
            taxonomy: TaxonomyService::new(db),
        })
    }
}

// No fallback to the adapter's pool: a missing connection is a composition error,
// not permission to read outside the consumer's aggregate transaction.
macro_rules! on_connection {
    ($connection:expr, $db:ident => $read:expr) => {
        match $connection {
            ForumTaxonomyConnection::Connection($db) => $read,
            ForumTaxonomyConnection::Transaction($db) => $read,
            ForumTaxonomyConnection::None => Err(ForumError::capability_unavailable(
                "taxonomy",
                "FORUM_TAXONOMY_CONNECTION_REQUIRED",
            )),
        }
    };
}

#[async_trait]
impl ForumTaxonomyPort for ServerForumTaxonomyPort {
    async fn lock_category_hierarchy_writer_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> Result<(), ForumError> {
        rustok_taxonomy::lock_category_hierarchy_writer_in_tx(txn, tenant_id)
            .await
            .map_err(map_error)
    }

    async fn load_category_parents(
        &self,
        conn: ForumTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Option<Uuid>>, ForumError> {
        on_connection!(conn, db => TaxonomyOwnerCategoryReader::load_module_category_parents_in_strict(db, tenant_id, FORUM_MODULE_SLUG, category_ids).await.map_err(map_error))
    }

    async fn load_category_siblings_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        parent_id: Option<Uuid>,
    ) -> Result<Vec<Uuid>, ForumError> {
        TaxonomyOwnerCategoryReader::load_module_category_sibling_ids_in(
            txn,
            tenant_id,
            FORUM_MODULE_SLUG,
            parent_id,
        )
        .await
        .map_err(map_error)
    }

    async fn shift_category_siblings_for_insert_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        parent_id: Option<Uuid>,
        requested_position: i32,
    ) -> Result<(), ForumError> {
        rustok_taxonomy::shift_module_category_siblings_for_insert_in_tx(
            txn,
            tenant_id,
            category_id,
            FORUM_MODULE_SLUG,
            parent_id,
            requested_position,
        )
        .await
        .map_err(map_error)
    }

    async fn move_category_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        parent_id: Option<Uuid>,
        position: i32,
    ) -> Result<Vec<ForumTaxonomyPlacement>, ForumError> {
        Ok(rustok_taxonomy::move_module_category_in_tx(
            txn,
            tenant_id,
            category_id,
            FORUM_MODULE_SLUG,
            parent_id,
            position,
        )
        .await
        .map_err(map_error)?
        .into_iter()
        .map(|p| ForumTaxonomyPlacement {
            term_id: p.term_id,
            parent_id: p.parent_id,
            position: p.position,
        })
        .collect())
    }

    async fn reorder_category_siblings_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        parent_id: Option<Uuid>,
        ordered_ids: &[Uuid],
    ) -> Result<(), ForumError> {
        rustok_taxonomy::reorder_module_category_siblings_in_tx(
            txn,
            tenant_id,
            FORUM_MODULE_SLUG,
            parent_id,
            ordered_ids,
        )
        .await
        .map_err(map_error)
    }

    async fn load_scoped_categories(
        &self,
        conn: ForumTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: Option<&[Uuid]>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Vec<ForumTaxonomyCategorySnapshot>, ForumError> {
        let categories = on_connection!(conn, db => TaxonomyOwnerCategoryReader::load_scoped_categories_in_strict(db, tenant_id, TaxonomyScopeType::Module, Some(FORUM_MODULE_SLUG), category_ids, locale, fallback_locale).await.map_err(map_error))?;
        Ok(categories.into_iter().map(snapshot).collect())
    }

    async fn load_category_locale_copy_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
    ) -> Result<Option<ForumTaxonomyCategoryLocaleCopy>, ForumError> {
        Ok(rustok_taxonomy::load_module_category_locale_copy_in_tx(
            txn,
            tenant_id,
            category_id,
            FORUM_MODULE_SLUG,
            locale,
        )
        .await
        .map_err(map_error)?
        .map(|c| ForumTaxonomyCategoryLocaleCopy {
            locale: c.locale,
            name: c.name,
            slug: c.slug,
            description: c.description,
        }))
    }

    async fn sync_category_copy_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: ForumTaxonomySyncCategoryInput,
    ) -> Result<(), ForumError> {
        rustok_taxonomy::sync_module_category_with_owned_aliases_in_tx(
            txn,
            tenant_id,
            SyncModuleCategoryInput {
                category_id: input.category_id,
                module_scope: FORUM_MODULE_SLUG.to_owned(),
                // Same immutable identity as the canonical Forum backfill migration.
                canonical_key: format!("forum-category-{}", input.category_id),
                parent_id: input.parent_id,
                position: input.position,
                icon_key: input.icon_key,
                color: input.color,
                locale: input.locale,
                name: input.name,
                slug: input.slug,
                description: input.description,
                aliases: Vec::new(),
            },
        )
        .await
        .map_err(map_error)?;
        Ok(())
    }

    async fn resolve_category_route(
        &self,
        conn: ForumTaxonomyConnection<'_>,
        tenant_id: Uuid,
        requested_locale: &str,
        requested_slug: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Option<ForumTaxonomyRouteMatch>, ForumError> {
        let matched = on_connection!(conn, db => TaxonomyService::resolve_term_route_for_module_in(db, tenant_id, TaxonomyTermKind::Category, FORUM_MODULE_SLUG, requested_locale, fallback_locale, requested_slug).await.map_err(map_error))?;
        // The owner resolver can fall back to a global dictionary. Forum owns only
        // its module binding; do not expose another scope as a Forum category.
        Ok(matched
            .filter(|m| {
                m.scope_type == TaxonomyScopeType::Module
                    && m.scope_value.as_deref() == Some(FORUM_MODULE_SLUG)
            })
            .map(|m| ForumTaxonomyRouteMatch {
                term_id: m.term_id,
                matched_locale: m.matched_locale,
                alias_id: m.alias_id,
            }))
    }

    async fn resolve_term_names(
        &self,
        conn: ForumTaxonomyConnection<'_>,
        tenant_id: Uuid,
        term_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, String>, ForumError> {
        on_connection!(conn, db => self.taxonomy.resolve_term_names_in(db, tenant_id, term_ids, locale, fallback_locale).await.map_err(map_error))
    }

    async fn ensure_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        tags: &[String],
    ) -> Result<Vec<Uuid>, ForumError> {
        self.taxonomy
            .ensure_terms_for_module_in_tx(
                txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                FORUM_MODULE_SLUG,
                locale,
                tags,
            )
            .await
            .map_err(map_error)
    }
}

fn snapshot(c: TaxonomyOwnerCategory) -> ForumTaxonomyCategorySnapshot {
    ForumTaxonomyCategorySnapshot {
        id: c.id,
        parent_id: c.parent_id,
        position: c.position,
        canonical_key: c.canonical_key,
        slug: c.slug,
        name: c.name,
        description: c.description,
        icon_key: c.icon_key,
        color: c.color,
        available_locales: c.available_locales,
        requested_locale: c.requested_locale,
        effective_locale: c.effective_locale,
    }
}

fn map_error(error: TaxonomyError) -> ForumError {
    // Display redacts provider-internal diagnostics. Keep the mapping exhaustive
    // so a new owner error cannot silently turn into a successful absence.
    let message = error.to_string();
    let code = match error {
        TaxonomyError::Database(error) => return ForumError::Database(error),
        TaxonomyError::Validation(message) => return ForumError::Validation(message),
        TaxonomyError::Forbidden(message) => return ForumError::Forbidden(message),
        TaxonomyError::TermNotFound(_) => "taxonomy.term_not_found",
        TaxonomyError::DuplicateCanonicalKey(_) => "taxonomy.duplicate_canonical_key",
        TaxonomyError::DuplicateSlug(_) => "taxonomy.duplicate_slug",
        TaxonomyError::DuplicateAlias(_) => "taxonomy.duplicate_alias",
        TaxonomyError::Conflict(_) => "taxonomy.conflict",
        TaxonomyError::TranslationRevisionExhausted { .. } => {
            "taxonomy.translation_revision_exhausted"
        }
        TaxonomyError::Internal(_) => "taxonomy.internal",
    };
    ForumError::capability_failure("taxonomy", code, message, false)
}

#[cfg(test)]
mod tests;
