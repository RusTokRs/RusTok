/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Adapter implementing `rustok_blog::ports::taxonomy::BlogTaxonomyPort` for the Blog module.
//!
//! Provides category hierarchy synchronization, localized copy loading,
//! category deletion with cleanup delegation, and tag lifecycle management
//! backed by native Taxonomy tables without exposing private Taxonomy entities to Blog.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource};
use rustok_blog::BlogError;
use rustok_blog::ports::taxonomy::{
    BlogCategoryDeleteCleanupPort, BlogTaxonomyCategoryLocaleCopy, BlogTaxonomyCategorySnapshot,
    BlogTaxonomyConnection, BlogTaxonomyCreateTagInput, BlogTaxonomyCreatedTag,
    BlogTaxonomyPlacement, BlogTaxonomyPort, BlogTaxonomySyncCategoryInput, BlogTaxonomyTagSnapshot,
    BlogTaxonomyUpdateTagInput,
};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QuerySelect,
};
use uuid::Uuid;

use crate::category_delete::TaxonomyCategoryDeleteCleanupPort;
use crate::dto::{TaxonomyScopeType, TaxonomyTermKind};
use crate::entities::taxonomy_term;
use crate::error::TaxonomyError;
use crate::module_term_mutation::{
    ModuleTermCreateInput, ModuleTermUpdateInput, delete_module_term_in_tx, lock_module_term_in_tx,
    update_module_term_in_tx,
};
use crate::owner_category_hierarchy_mutation::{
    delete_module_category_placement_and_compact_in_tx, move_module_category_in_tx,
    reorder_module_category_siblings_in_tx,
};
use crate::owner_category_read::TaxonomyOwnerCategoryReader;
use crate::owner_category_route_sync::{
    load_module_category_locale_copy_in_tx, sync_module_category_with_owned_aliases_in_tx,
};
use crate::owner_read::TaxonomyOwnerReader;
use crate::services::TaxonomyService;
use crate::translation_target::TaxonomyModuleTermTranslationOwner;

pub const BLOG_MODULE_SCOPE: &str = "blog";

fn map_taxonomy_error(err: TaxonomyError) -> BlogError {
    match err {
        TaxonomyError::Database(db_err) => BlogError::Database(db_err),
        TaxonomyError::Validation(msg) => BlogError::validation(msg),
        TaxonomyError::Forbidden(msg) => BlogError::forbidden(msg),
        TaxonomyError::TermNotFound(id) => BlogError::TaxonomyTermNotFound(id),
        TaxonomyError::DuplicateCanonicalKey(msg)
        | TaxonomyError::DuplicateSlug(msg)
        | TaxonomyError::DuplicateAlias(msg)
        | TaxonomyError::Conflict(msg) => BlogError::conflict(msg),
        TaxonomyError::TranslationRevisionExhausted { term_id, locale } => {
            BlogError::conflict(format!(
                "Translation revision exhausted for term {term_id} locale {locale}"
            ))
        }
        TaxonomyError::Internal(msg) => BlogError::invariant(msg),
    }
}

/// Host cleanup adapter wrapping Blog's category cleanup port into Taxonomy's cleanup port.
struct BlogCategoryDeleteCleanupAdapter {
    inner: Arc<dyn BlogCategoryDeleteCleanupPort>,
}

#[async_trait]
impl TaxonomyCategoryDeleteCleanupPort for BlogCategoryDeleteCleanupAdapter {
    async fn cleanup_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> Result<(), TaxonomyError> {
        self.inner
            .cleanup_in_tx(txn, tenant_id, category_id)
            .await
            .map_err(|err| TaxonomyError::validation(err.to_string()))
    }
}

/// Adapter providing `BlogTaxonomyPort` backed by the native Taxonomy database.
pub struct TaxonomyBlogTaxonomyPortAdapter {
    _db: DatabaseConnection,
    service: TaxonomyService,
}

impl TaxonomyBlogTaxonomyPortAdapter {
    pub fn new(db: DatabaseConnection) -> Self {
        let service = TaxonomyService::new(db.clone());
        Self { _db: db, service }
    }
}

#[async_trait]
impl BlogTaxonomyPort for TaxonomyBlogTaxonomyPortAdapter {
    async fn lock_category_hierarchy_writer_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> Result<(), BlogError> {
        crate::lock_category_hierarchy_writer_in_tx(txn, tenant_id)
            .await
            .map_err(map_taxonomy_error)
    }

    async fn load_scoped_categories(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: Option<&[Uuid]>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Vec<BlogTaxonomyCategorySnapshot>, BlogError> {
        let categories = match conn {
            BlogTaxonomyConnection::Connection(c) => {
                TaxonomyOwnerCategoryReader::load_scoped_categories_in(
                    c,
                    tenant_id,
                    TaxonomyScopeType::Module,
                    Some(BLOG_MODULE_SCOPE),
                    category_ids,
                    locale,
                    fallback_locale,
                )
                .await
            }
            BlogTaxonomyConnection::Transaction(tx) => {
                TaxonomyOwnerCategoryReader::load_scoped_categories_in(
                    tx,
                    tenant_id,
                    TaxonomyScopeType::Module,
                    Some(BLOG_MODULE_SCOPE),
                    category_ids,
                    locale,
                    fallback_locale,
                )
                .await
            }
        }
        .map_err(map_taxonomy_error)?;

        Ok(categories
            .into_iter()
            .map(|c| BlogTaxonomyCategorySnapshot {
                id: c.id,
                parent_id: c.parent_id,
                canonical_key: c.canonical_key,
                slug: c.slug,
                name: c.name,
                description: c.description,
                position: c.position,
                available_locales: c.available_locales,
                requested_locale: c.requested_locale,
                effective_locale: c.effective_locale,
            })
            .collect())
    }

    async fn load_category_locale_copy_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
    ) -> Result<Option<BlogTaxonomyCategoryLocaleCopy>, BlogError> {
        let copy = load_module_category_locale_copy_in_tx(
            txn,
            tenant_id,
            BLOG_MODULE_SCOPE,
            category_id,
            locale,
        )
        .await
        .map_err(map_taxonomy_error)?;

        Ok(copy.map(|c| BlogTaxonomyCategoryLocaleCopy {
            name: c.name,
            slug: c.slug,
            description: c.description,
        }))
    }

    async fn sync_category_copy_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: BlogTaxonomySyncCategoryInput,
    ) -> Result<(), BlogError> {
        sync_module_category_with_owned_aliases_in_tx(
            txn,
            tenant_id,
            BLOG_MODULE_SCOPE,
            input.category_id,
            input.parent_id,
            input.position,
            &input.locale,
            &input.name,
            &input.slug,
            input.description.as_deref(),
            &[],
        )
        .await
        .map_err(map_taxonomy_error)?;

        Ok(())
    }

    async fn move_category_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        new_parent_id: Option<Uuid>,
        position: Option<i32>,
    ) -> Result<Vec<BlogTaxonomyPlacement>, BlogError> {
        let placements = move_module_category_in_tx(
            txn,
            tenant_id,
            BLOG_MODULE_SCOPE,
            category_id,
            new_parent_id,
            position,
        )
        .await
        .map_err(map_taxonomy_error)?;

        Ok(placements
            .into_iter()
            .map(|p| BlogTaxonomyPlacement {
                category_id: p.category_id,
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
        ordered_category_ids: &[Uuid],
    ) -> Result<(), BlogError> {
        reorder_module_category_siblings_in_tx(
            txn,
            tenant_id,
            BLOG_MODULE_SCOPE,
            parent_id,
            ordered_category_ids,
        )
        .await
        .map_err(map_taxonomy_error)?;

        Ok(())
    }

    async fn delete_category_placement_and_compact_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> Result<(), BlogError> {
        delete_module_category_placement_and_compact_in_tx(
            txn,
            tenant_id,
            BLOG_MODULE_SCOPE,
            category_id,
        )
        .await
        .map_err(map_taxonomy_error)?;

        Ok(())
    }

    async fn delete_module_category_with_cleanup(
        &self,
        conn: &DatabaseConnection,
        tenant_id: Uuid,
        category_id: Uuid,
        _security: &SecurityContext,
        cleanup: Arc<dyn BlogCategoryDeleteCleanupPort>,
    ) -> Result<(), BlogError> {
        let adapter = BlogCategoryDeleteCleanupAdapter { inner: cleanup };
        self.service
            .delete_module_category_with_cleanup(
                tenant_id,
                category_id,
                BLOG_MODULE_SCOPE,
                &adapter,
            )
            .await
            .map_err(map_taxonomy_error)?;

        Ok(())
    }

    async fn load_tags_by_ids(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        tag_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, BlogTaxonomyTagSnapshot>, BlogError> {
        let terms = match conn {
            BlogTaxonomyConnection::Connection(c) => {
                TaxonomyOwnerReader::load_terms_by_ids_in(
                    c,
                    tenant_id,
                    TaxonomyTermKind::Tag,
                    tag_ids,
                    locale,
                    fallback_locale,
                )
                .await
            }
            BlogTaxonomyConnection::Transaction(tx) => {
                TaxonomyOwnerReader::load_terms_by_ids_in(
                    tx,
                    tenant_id,
                    TaxonomyTermKind::Tag,
                    tag_ids,
                    locale,
                    fallback_locale,
                )
                .await
            }
        }
        .map_err(map_taxonomy_error)?;

        Ok(terms
            .into_iter()
            .map(|(id, term)| {
                (
                    id,
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
                    },
                )
            })
            .collect())
    }

    async fn load_scoped_tags(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        tag_ids: Option<&[Uuid]>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Vec<BlogTaxonomyTagSnapshot>, BlogError> {
        let terms = match conn {
            BlogTaxonomyConnection::Connection(c) => {
                TaxonomyOwnerReader::load_scoped_terms_in(
                    c,
                    tenant_id,
                    TaxonomyTermKind::Tag,
                    TaxonomyScopeType::Module,
                    Some(BLOG_MODULE_SCOPE),
                    tag_ids,
                    locale,
                    fallback_locale,
                )
                .await
            }
            BlogTaxonomyConnection::Transaction(tx) => {
                TaxonomyOwnerReader::load_scoped_terms_in(
                    tx,
                    tenant_id,
                    TaxonomyTermKind::Tag,
                    TaxonomyScopeType::Module,
                    Some(BLOG_MODULE_SCOPE),
                    tag_ids,
                    locale,
                    fallback_locale,
                )
                .await
            }
        }
        .map_err(map_taxonomy_error)?;

        Ok(terms
            .into_iter()
            .map(|term| BlogTaxonomyTagSnapshot {
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
            })
            .collect())
    }

    async fn resolve_tag_id_for_module(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        tag: &str,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Option<Uuid>, BlogError> {
        match conn {
            BlogTaxonomyConnection::Connection(c) => {
                TaxonomyOwnerReader::resolve_module_term_id_in(
                    c,
                    tenant_id,
                    TaxonomyTermKind::Tag,
                    BLOG_MODULE_SCOPE,
                    tag,
                    locale,
                    fallback_locale,
                )
                .await
            }
            BlogTaxonomyConnection::Transaction(tx) => {
                TaxonomyOwnerReader::resolve_module_term_id_in(
                    tx,
                    tenant_id,
                    TaxonomyTermKind::Tag,
                    BLOG_MODULE_SCOPE,
                    tag,
                    locale,
                    fallback_locale,
                )
                .await
            }
        }
        .map_err(map_taxonomy_error)
    }

    async fn find_all_valid_tag_ids(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
    ) -> Result<HashSet<Uuid>, BlogError> {
        let query = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
            .filter(
                sea_orm::Condition::any()
                    .add(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Global))
                    .add(
                        sea_orm::Condition::all()
                            .add(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
                            .add(taxonomy_term::Column::ScopeValue.eq(BLOG_MODULE_SCOPE)),
                    ),
            )
            .select_only()
            .column(taxonomy_term::Column::Id);

        let ids: Vec<Uuid> = match conn {
            BlogTaxonomyConnection::Connection(c) => query.into_tuple().all(c).await,
            BlogTaxonomyConnection::Transaction(tx) => query.into_tuple().all(tx).await,
        }
        .map_err(BlogError::Database)?;

        Ok(ids.into_iter().collect())
    }

    async fn ensure_module_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        tag_names: &[String],
        allow_create: bool,
    ) -> Result<Vec<Uuid>, BlogError> {
        if allow_create {
            self.service
                .ensure_terms_for_module_in_tx(
                    txn,
                    tenant_id,
                    TaxonomyTermKind::Tag,
                    BLOG_MODULE_SCOPE,
                    locale,
                    tag_names,
                )
                .await
                .map_err(map_taxonomy_error)
        } else {
            let mut resolved = Vec::new();
            for name in tag_names {
                let term_id = TaxonomyOwnerReader::resolve_module_term_id_in(
                    txn,
                    tenant_id,
                    TaxonomyTermKind::Tag,
                    BLOG_MODULE_SCOPE,
                    name,
                    locale,
                    None,
                )
                .await
                .map_err(map_taxonomy_error)?;
                if let Some(id) = term_id {
                    resolved.push(id);
                }
            }
            Ok(resolved)
        }
    }

    async fn create_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: BlogTaxonomyCreateTagInput,
    ) -> Result<BlogTaxonomyCreatedTag, BlogError> {
        let tag_id = self
            .service
            .create_module_term_in_tx(
                txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                BLOG_MODULE_SCOPE,
                ModuleTermCreateInput {
                    locale: input.locale,
                    name: input.name,
                    slug: input.slug,
                    canonical_key: None,
                },
            )
            .await
            .map_err(map_taxonomy_error)?;

        let term = taxonomy_term::Entity::find_by_id(tag_id)
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await
            .map_err(BlogError::Database)?
            .ok_or_else(|| BlogError::TaxonomyTermNotFound(tag_id))?;

        Ok(BlogTaxonomyCreatedTag {
            id: tag_id,
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
    ) -> Result<BlogTaxonomyTagSnapshot, BlogError> {
        let updated = update_module_term_in_tx(
            txn,
            tenant_id,
            tag_id,
            security,
            TaxonomyTermKind::Tag,
            BLOG_MODULE_SCOPE,
            ModuleTermUpdateInput {
                locale: input.locale,
                name: input.name,
                slug: input.slug,
            },
        )
        .await
        .map_err(map_taxonomy_error)?;

        let term = taxonomy_term::Entity::find_by_id(tag_id)
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await
            .map_err(BlogError::Database)?
            .ok_or_else(|| BlogError::TaxonomyTermNotFound(tag_id))?;

        Ok(BlogTaxonomyTagSnapshot {
            id: updated.id,
            tenant_id: updated.tenant_id,
            canonical_key: term.canonical_key,
            name: updated.name,
            slug: updated.slug,
            scope_type: term.scope_type.to_string(),
            scope_value: term.scope_value,
            locale: updated.locale,
            effective_locale: updated.effective_locale,
            created_at: updated.created_at,
        })
    }

    async fn delete_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        tag_id: Uuid,
        security: &SecurityContext,
    ) -> Result<(), BlogError> {
        delete_module_term_in_tx(
            txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            BLOG_MODULE_SCOPE,
            tag_id,
            security,
        )
        .await
        .map_err(map_taxonomy_error)?;

        Ok(())
    }

    async fn lock_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        tag_id: Uuid,
    ) -> Result<(), BlogError> {
        lock_module_term_in_tx(
            txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            BLOG_MODULE_SCOPE,
            tag_id,
        )
        .await
        .map_err(map_taxonomy_error)?;

        Ok(())
    }
}

/// Translation owner for Blog module terms, publishing reindex domain events upon updates.
pub struct BlogTaxonomyTranslationOwner;

#[async_trait]
impl TaxonomyModuleTermTranslationOwner for BlogTaxonomyTranslationOwner {
    fn module_slug(&self) -> &str {
        BLOG_MODULE_SCOPE
    }

    fn authorize(
        &self,
        context: &PortContext,
        _tenant_id: Uuid,
        kind: TaxonomyTermKind,
        _term_id: Uuid,
        action: Action,
    ) -> Result<(), PortError> {
        let security = SecurityContext::try_from_port_context(context)?;
        let resource = match kind {
            TaxonomyTermKind::Tag => Resource::Tags,
            TaxonomyTermKind::Category => Resource::BlogCategories,
        };
        if security.get_scope(resource, action) == PermissionScope::None {
            return Err(PortError::forbidden(
                "blog.taxonomy_translation_permission_denied",
                format!("blog owner permission is required for {resource}:{action}"),
            ));
        }
        Ok(())
    }

    async fn on_translation_applied_in_tx(
        &self,
        transaction: &DatabaseTransaction,
        context: &PortContext,
        tenant_id: Uuid,
        _kind: TaxonomyTermKind,
        _term_id: Uuid,
    ) -> Result<(), PortError> {
        let security = SecurityContext::try_from_port_context(context)?;
        TransactionalEventBus::publish_root_in_tx(
            transaction,
            tenant_id,
            security.user_id,
            DomainEvent::ReindexRequested {
                target_type: "blog".to_string(),
                target_id: None,
            },
        )
        .await
        .map_err(|_| {
            PortError::unavailable(
                "blog.taxonomy_translation_reindex_unavailable",
                "Blog reindex side effect could not be queued",
            )
        })?;
        Ok(())
    }
}

/// Constructs an in-process `BlogTaxonomyPort` adapter backed by the native Taxonomy module.
pub fn in_process_blog_taxonomy_port(db: DatabaseConnection) -> Arc<dyn BlogTaxonomyPort> {
    Arc::new(TaxonomyBlogTaxonomyPortAdapter::new(db))
}
