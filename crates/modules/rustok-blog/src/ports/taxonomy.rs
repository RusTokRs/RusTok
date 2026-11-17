/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Blog-owned port boundary for taxonomy collaboration (tags and categories).
//!
//! Provides transport-neutral and persistence-neutral contracts for blog tag
//! resolution, tag lifecycle, category synchronization, hierarchy mutations,
//! and category tree projections without direct coupling to Taxonomy entities or tables.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::SecurityContext;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, Statement, TransactionTrait,
};
use uuid::Uuid;

use crate::error::BlogError;

/// Either an open database transaction or a connection pool handle.
#[derive(Copy, Clone)]
pub enum BlogTaxonomyConnection<'a> {
    Connection(&'a DatabaseConnection),
    Transaction(&'a DatabaseTransaction),
}

impl<'a> From<&'a DatabaseConnection> for BlogTaxonomyConnection<'a> {
    fn from(c: &'a DatabaseConnection) -> Self {
        BlogTaxonomyConnection::Connection(c)
    }
}

impl<'a> From<&'a DatabaseTransaction> for BlogTaxonomyConnection<'a> {
    fn from(tx: &'a DatabaseTransaction) -> Self {
        BlogTaxonomyConnection::Transaction(tx)
    }
}

pub trait AsBlogTaxonomyConnection {
    fn as_blog_taxonomy_connection(&self) -> BlogTaxonomyConnection<'_>;
}

impl AsBlogTaxonomyConnection for DatabaseConnection {
    fn as_blog_taxonomy_connection(&self) -> BlogTaxonomyConnection<'_> {
        BlogTaxonomyConnection::Connection(self)
    }
}

impl AsBlogTaxonomyConnection for DatabaseTransaction {
    fn as_blog_taxonomy_connection(&self) -> BlogTaxonomyConnection<'_> {
        BlogTaxonomyConnection::Transaction(self)
    }
}

/// Category metadata projected from the taxonomy provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogTaxonomyCategorySnapshot {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub canonical_key: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub position: i32,
    pub available_locales: Vec<String>,
    pub requested_locale: String,
    pub effective_locale: String,
}

/// Localized copy for an existing category term in taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogTaxonomyCategoryLocaleCopy {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
}

/// Input for synchronizing a blog category translation and placement into taxonomy.
#[derive(Debug, Clone)]
pub struct BlogTaxonomySyncCategoryInput {
    pub category_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
}

/// Structural category placement response after a hierarchy move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogTaxonomyPlacement {
    pub category_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub position: i32,
}

/// Tag metadata projected from the taxonomy provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogTaxonomyTagSnapshot {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub canonical_key: String,
    pub name: String,
    pub slug: String,
    pub scope_type: String,
    pub scope_value: Option<String>,
    pub locale: String,
    pub effective_locale: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Tag creation input for authoring a module tag term in taxonomy.
#[derive(Debug, Clone)]
pub struct BlogTaxonomyCreateTagInput {
    pub name: String,
    pub slug: Option<String>,
    pub locale: String,
}

/// Tag update input for modifying a module tag term in taxonomy.
#[derive(Debug, Clone)]
pub struct BlogTaxonomyUpdateTagInput {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub locale: String,
}

/// Result of creating a tag in taxonomy containing its identity and immutable canonical key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogTaxonomyCreatedTag {
    pub id: Uuid,
    pub canonical_key: String,
}

/// Cleanup port passed to category deletion to remove host-owned and blog-owned attached rows.
#[async_trait]
pub trait BlogCategoryDeleteCleanupPort: Send + Sync {
    async fn cleanup_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> Result<(), BlogError>;
}

/// Default no-op cleanup port implementation.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopBlogCategoryDeleteCleanupPort;

#[async_trait]
impl BlogCategoryDeleteCleanupPort for NoopBlogCategoryDeleteCleanupPort {
    async fn cleanup_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _category_id: Uuid,
    ) -> Result<(), BlogError> {
        Ok(())
    }
}

/// Transactional and projection port for Blog taxonomy operations (categories and tags).
#[async_trait]
pub trait BlogTaxonomyPort: Send + Sync {
    // --- Category methods ---

    /// Locks category hierarchy writer within the transaction to serialize structural modifications.
    async fn lock_category_hierarchy_writer_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> Result<(), BlogError>;

    /// Loads localized category snapshots for the specified IDs and locale.
    async fn load_scoped_categories(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: Option<&[Uuid]>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Vec<BlogTaxonomyCategorySnapshot>, BlogError>;

    /// Loads the localized copy of a single category term in transaction.
    async fn load_category_locale_copy_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
    ) -> Result<Option<BlogTaxonomyCategoryLocaleCopy>, BlogError>;

    /// Synchronizes a created or updated blog category copy and hierarchy into taxonomy in transaction.
    async fn sync_category_copy_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: BlogTaxonomySyncCategoryInput,
    ) -> Result<(), BlogError>;

    /// Repositions a category in the hierarchy within a transaction.
    async fn move_category_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        new_parent_id: Option<Uuid>,
        position: Option<i32>,
    ) -> Result<Vec<BlogTaxonomyPlacement>, BlogError>;

    /// Reorders sibling categories under a parent in transaction.
    async fn reorder_category_siblings_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        parent_id: Option<Uuid>,
        ordered_category_ids: &[Uuid],
    ) -> Result<(), BlogError>;

    /// Deletes a category's hierarchy placement and compacts siblings in transaction.
    async fn delete_category_placement_and_compact_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> Result<(), BlogError>;

    /// Hard-deletes a category in taxonomy along with invocation of the host-provided cleanup.
    async fn delete_module_category_with_cleanup(
        &self,
        conn: &DatabaseConnection,
        tenant_id: Uuid,
        category_id: Uuid,
        security: &SecurityContext,
        cleanup: Arc<dyn BlogCategoryDeleteCleanupPort>,
    ) -> Result<(), BlogError>;

    // --- Tag methods ---

    /// Loads tag snapshots by term UUIDs.
    async fn load_tags_by_ids(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        tag_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, BlogTaxonomyTagSnapshot>, BlogError>;

    /// Loads scoped tags matching tenant and optional ID filter (e.g. for search projections).
    async fn load_scoped_tags(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        tag_ids: Option<&[Uuid]>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Vec<BlogTaxonomyTagSnapshot>, BlogError>;

    /// Resolves a single term ID from a user-supplied tag name or slug string.
    async fn resolve_tag_id_for_module(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        tag: &str,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Option<Uuid>, BlogError>;

    /// Returns the set of all active tag IDs that are visible to Blog (global or module:blog).
    async fn find_all_valid_tag_ids(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
    ) -> Result<HashSet<Uuid>, BlogError>;

    /// Ensures module:blog tags exist for the specified names, optionally authoring missing ones in transaction.
    async fn ensure_module_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        tag_names: &[String],
        allow_create: bool,
    ) -> Result<Vec<Uuid>, BlogError>;

    /// Creates a module:blog tag term in taxonomy in transaction.
    async fn create_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: BlogTaxonomyCreateTagInput,
    ) -> Result<BlogTaxonomyCreatedTag, BlogError>;

    /// Updates a module:blog tag term in taxonomy in transaction.
    async fn update_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        tag_id: Uuid,
        security: &SecurityContext,
        input: BlogTaxonomyUpdateTagInput,
    ) -> Result<BlogTaxonomyTagSnapshot, BlogError>;

    /// Deletes a module:blog tag term from taxonomy in transaction.
    async fn delete_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        tag_id: Uuid,
        security: &SecurityContext,
    ) -> Result<(), BlogError>;

    /// Locks a module:blog tag term for update in transaction.
    async fn lock_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        tag_id: Uuid,
    ) -> Result<(), BlogError>;
}

/// Default no-op taxonomy implementation used when running Blog in standalone microservice mode
/// or in tests without an attached taxonomy provider.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopBlogTaxonomyPort;

#[async_trait]
impl BlogTaxonomyPort for NoopBlogTaxonomyPort {
    async fn lock_category_hierarchy_writer_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
    ) -> Result<(), BlogError> {
        Ok(())
    }

    async fn load_scoped_categories(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: Option<&[Uuid]>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Vec<BlogTaxonomyCategorySnapshot>, BlogError> {
        let Some(ids) = category_ids else {
            return Ok(Vec::new());
        };
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let backend = match conn {
            BlogTaxonomyConnection::Connection(c) => c.get_database_backend(),
            BlogTaxonomyConnection::Transaction(tx) => tx.get_database_backend(),
        };

        let mut results = Vec::with_capacity(ids.len());
        for &id in ids {
            let query = Statement::from_sql_and_values(
                backend,
                r#"SELECT t.canonical_key, h.parent_term_id, h.position,
                          tr.name, tr.slug, tr.description, tr.locale
                   FROM taxonomy_terms t
                   JOIN taxonomy_category_hierarchy h ON h.tenant_id = t.tenant_id AND h.term_id = t.id
                   LEFT JOIN taxonomy_term_translations tr ON tr.term_id = t.id AND (tr.locale = $3 OR tr.locale = $4)
                   WHERE t.tenant_id = $1 AND t.id = $2
                   ORDER BY CASE WHEN tr.locale = $3 THEN 0 ELSE 1 END
                   LIMIT 1"#,
                vec![
                    tenant_id.into(),
                    id.into(),
                    locale.into(),
                    fallback_locale.unwrap_or(locale).into(),
                ],
            );

            let row_res = match conn {
                BlogTaxonomyConnection::Connection(c) => c.query_one_raw(query).await,
                BlogTaxonomyConnection::Transaction(tx) => tx.query_one_raw(query).await,
            };

            if let Ok(Some(row)) = row_res {
                let canonical_key = row
                    .try_get::<String>("", "canonical_key")
                    .unwrap_or_else(|_| format!("blog-category-{id}"));
                let parent_id = row.try_get::<Option<Uuid>>("", "parent_term_id").ok().flatten();
                let position = row.try_get::<i32>("", "position").unwrap_or(0);
                let name = row
                    .try_get::<String>("", "name")
                    .unwrap_or_else(|_| canonical_key.clone());
                let slug = row
                    .try_get::<String>("", "slug")
                    .unwrap_or_else(|_| canonical_key.clone());
                let description = row.try_get::<Option<String>>("", "description").ok().flatten();
                let eff_locale = row
                    .try_get::<String>("", "locale")
                    .unwrap_or_else(|_| locale.to_string());

                let get_locales = Statement::from_sql_and_values(
                    backend,
                    r#"SELECT DISTINCT locale FROM taxonomy_term_translations WHERE tenant_id = $1 AND term_id = $2"#,
                    vec![tenant_id.into(), id.into()],
                );
                let loc_rows = match conn {
                    BlogTaxonomyConnection::Connection(c) => c.query_all_raw(get_locales).await,
                    BlogTaxonomyConnection::Transaction(tx) => tx.query_all_raw(get_locales).await,
                };
                let mut available_locales = Vec::new();
                if let Ok(locs) = loc_rows {
                    for l in locs {
                        if let Ok(loc) = l.try_get::<String>("", "locale") {
                            available_locales.push(loc);
                        }
                    }
                }
                if available_locales.is_empty() {
                    available_locales.push(locale.to_string());
                }

                results.push(BlogTaxonomyCategorySnapshot {
                    id,
                    parent_id,
                    canonical_key,
                    slug,
                    name,
                    description,
                    position,
                    available_locales,
                    requested_locale: locale.to_string(),
                    effective_locale: eff_locale,
                });
            }
        }
        Ok(results)
    }

    async fn load_category_locale_copy_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
    ) -> Result<Option<BlogTaxonomyCategoryLocaleCopy>, BlogError> {
        let backend = txn.get_database_backend();
        let query = Statement::from_sql_and_values(
            backend,
            r#"SELECT tr.name, tr.slug, tr.description
               FROM taxonomy_term_translations tr
               WHERE tr.tenant_id = $1 AND tr.term_id = $2 AND tr.locale = $3
               LIMIT 1"#,
            vec![tenant_id.into(), category_id.into(), locale.into()],
        );
        if let Ok(Some(row)) = txn.query_one_raw(query).await {
            let name = row.try_get::<String>("", "name").map_err(BlogError::from)?;
            let slug = row.try_get::<String>("", "slug").map_err(BlogError::from)?;
            let description = row.try_get::<Option<String>>("", "description").ok().flatten();
            return Ok(Some(BlogTaxonomyCategoryLocaleCopy {
                name,
                slug,
                description,
            }));
        }
        Ok(None)
    }

    async fn sync_category_copy_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: BlogTaxonomySyncCategoryInput,
    ) -> Result<(), BlogError> {
        let backend = txn.get_database_backend();
        let canonical_key = format!("blog-category-{}", input.category_id);

        // 1. Conflict check against taxonomy_term_route_keys
        let check_route = Statement::from_sql_and_values(
            backend,
            r#"SELECT term_id FROM taxonomy_term_route_keys
               WHERE tenant_id = $1 AND locale = $2 AND route_key = $3 AND term_id != $4
               LIMIT 1"#,
            vec![
                tenant_id.into(),
                input.locale.clone().into(),
                input.slug.clone().into(),
                input.category_id.into(),
            ],
        );
        if let Ok(Some(_)) = txn.query_one_raw(check_route).await {
            return Err(BlogError::conflict(format!(
                "Category route '{}' conflicts with an existing route",
                input.slug
            )));
        }

        // 2. Conflict check against taxonomy_term_aliases
        let check_alias = Statement::from_sql_and_values(
            backend,
            r#"SELECT term_id FROM taxonomy_term_aliases
               WHERE tenant_id = $1 AND locale = $2 AND slug = $3 AND term_id != $4
               LIMIT 1"#,
            vec![
                tenant_id.into(),
                input.locale.clone().into(),
                input.slug.clone().into(),
                input.category_id.into(),
            ],
        );
        if let Ok(Some(_)) = txn.query_one_raw(check_alias).await {
            return Err(BlogError::conflict(format!(
                "Category route '{}' conflicts with an existing alias",
                input.slug
            )));
        }

        // 3. Conflict check against other categories' translations
        let check_tr = Statement::from_sql_and_values(
            backend,
            r#"SELECT term_id FROM taxonomy_term_translations
               WHERE tenant_id = $1 AND locale = $2 AND slug = $3 AND term_id != $4
               LIMIT 1"#,
            vec![
                tenant_id.into(),
                input.locale.clone().into(),
                input.slug.clone().into(),
                input.category_id.into(),
            ],
        );
        if let Ok(Some(_)) = txn.query_one_raw(check_tr).await {
            return Err(BlogError::conflict(format!(
                "Category slug '{}' conflicts with an existing category translation",
                input.slug
            )));
        }

        let select_term = Statement::from_sql_and_values(
            backend,
            r#"SELECT id FROM taxonomy_terms WHERE tenant_id = $1 AND id = $2"#,
            vec![tenant_id.into(), input.category_id.into()],
        );
        let exists = txn.query_one_raw(select_term).await.map_err(BlogError::from)?.is_some();

        if !exists {
            let insert_term = Statement::from_sql_and_values(
                backend,
                r#"INSERT INTO taxonomy_terms (id, tenant_id, kind, scope_type, scope_value, canonical_key, created_at, updated_at)
                   VALUES ($1, $2, 'category', 'module', 'blog', $3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
                vec![input.category_id.into(), tenant_id.into(), canonical_key.into()],
            );
            txn.execute_raw(insert_term).await.map_err(BlogError::from)?;
        }

        let select_tr = Statement::from_sql_and_values(
            backend,
            r#"SELECT id, slug, name FROM taxonomy_term_translations WHERE term_id = $1 AND locale = $2"#,
            vec![input.category_id.into(), input.locale.clone().into()],
        );
        let existing_tr = txn.query_one_raw(select_tr).await.map_err(BlogError::from)?;

        if let Some(row) = existing_tr {
            let old_slug = row.try_get::<String>("", "slug").map_err(BlogError::from)?;
            let old_name = row.try_get::<String>("", "name").map_err(BlogError::from)?;

            if old_slug != input.slug {
                // Insert old slug into taxonomy_term_aliases
                let check_alias_exists = Statement::from_sql_and_values(
                    backend,
                    r#"SELECT id FROM taxonomy_term_aliases WHERE tenant_id = $1 AND term_id = $2 AND locale = $3 AND slug = $4"#,
                    vec![tenant_id.into(), input.category_id.into(), input.locale.clone().into(), old_slug.clone().into()],
                );
                let alias_exists = txn.query_one_raw(check_alias_exists).await.map_err(BlogError::from)?.is_some();
                if !alias_exists {
                    let insert_alias = Statement::from_sql_and_values(
                        backend,
                        r#"INSERT INTO taxonomy_term_aliases (id, term_id, tenant_id, locale, name, slug, created_at)
                           VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP)"#,
                        vec![
                            Uuid::new_v4().into(),
                            input.category_id.into(),
                            tenant_id.into(),
                            input.locale.clone().into(),
                            old_name.into(),
                            old_slug.clone().into(),
                        ],
                    );
                    txn.execute_raw(insert_alias)
                        .await
                        .map_err(BlogError::from)?;
                }
            }

            let update_tr = Statement::from_sql_and_values(
                backend,
                r#"UPDATE taxonomy_term_translations SET name = $1, slug = $2, description = $3, updated_at = CURRENT_TIMESTAMP
                   WHERE term_id = $4 AND locale = $5"#,
                vec![
                    input.name.into(),
                    input.slug.clone().into(),
                    input.description.into(),
                    input.category_id.into(),
                    input.locale.clone().into(),
                ],
            );
            txn.execute_raw(update_tr).await.map_err(BlogError::from)?;
        } else {
            let insert_tr = Statement::from_sql_and_values(
                backend,
                r#"INSERT INTO taxonomy_term_translations (id, term_id, tenant_id, locale, name, slug, description, created_at, updated_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
                vec![
                    Uuid::new_v4().into(),
                    input.category_id.into(),
                    tenant_id.into(),
                    input.locale.clone().into(),
                    input.name.into(),
                    input.slug.clone().into(),
                    input.description.into(),
                ],
            );
            txn.execute_raw(insert_tr).await.map_err(BlogError::from)?;
        }

        // Register route key in taxonomy_term_route_keys if table exists
        let check_rk = Statement::from_sql_and_values(
            backend,
            r#"SELECT term_id FROM taxonomy_term_route_keys
               WHERE tenant_id = $1 AND kind = 'category' AND scope_type = 'module' AND scope_value = 'blog'
                 AND locale = $2 AND route_key = $3"#,
            vec![tenant_id.into(), input.locale.clone().into(), input.slug.clone().into()],
        );
        if let Ok(existing_rk) = txn.query_one_raw(check_rk).await {
            if existing_rk.is_none() {
                let insert_rk = Statement::from_sql_and_values(
                    backend,
                    r#"INSERT INTO taxonomy_term_route_keys (tenant_id, kind, scope_type, scope_value, locale, route_key, term_id)
                       VALUES ($1, 'category', 'module', 'blog', $2, $3, $4)"#,
                    vec![tenant_id.into(), input.locale.clone().into(), input.slug.clone().into(), input.category_id.into()],
                );
                txn.execute_raw(insert_rk)
                    .await
                    .map_err(BlogError::from)?;
            }
        }

        let select_h = Statement::from_sql_and_values(
            backend,
            r#"SELECT term_id FROM taxonomy_category_hierarchy WHERE tenant_id = $1 AND term_id = $2"#,
            vec![tenant_id.into(), input.category_id.into()],
        );
        let h_exists = txn.query_one_raw(select_h).await.map_err(BlogError::from)?.is_some();

        if h_exists {
            let update_h = Statement::from_sql_and_values(
                backend,
                r#"UPDATE taxonomy_category_hierarchy SET parent_term_id = $1, position = $2 WHERE tenant_id = $3 AND term_id = $4"#,
                vec![
                    input.parent_id.into(),
                    input.position.into(),
                    tenant_id.into(),
                    input.category_id.into(),
                ],
            );
            txn.execute_raw(update_h).await.map_err(BlogError::from)?;
        } else {
            let insert_h = Statement::from_sql_and_values(
                backend,
                r#"INSERT INTO taxonomy_category_hierarchy (tenant_id, term_id, parent_term_id, position)
                   VALUES ($1, $2, $3, $4)"#,
                vec![
                    tenant_id.into(),
                    input.category_id.into(),
                    input.parent_id.into(),
                    input.position.into(),
                ],
            );
            txn.execute_raw(insert_h).await.map_err(BlogError::from)?;
        }

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
        let backend = txn.get_database_backend();

        let get_placement = Statement::from_sql_and_values(
            backend,
            r#"SELECT parent_term_id, position FROM taxonomy_category_hierarchy WHERE tenant_id = $1 AND term_id = $2"#,
            vec![tenant_id.into(), category_id.into()],
        );
        let row = txn
            .query_one_raw(get_placement)
            .await
            .map_err(BlogError::from)?
            .ok_or_else(|| {
                BlogError::invariant(format!(
                    "Module Category {category_id} hierarchy placement is missing during move"
                ))
            })?;
        let source_parent_id = row.try_get::<Option<Uuid>>("", "parent_term_id").ok().flatten();
        let source_position = row.try_get::<i32>("", "position").unwrap_or(0);
        let target_position = position.unwrap_or(0);

        if source_parent_id != new_parent_id {
            // Compact source siblings
            let compact_source = match source_parent_id {
                Some(p) => Statement::from_sql_and_values(
                    backend,
                    r#"UPDATE taxonomy_category_hierarchy SET position = position - 1
                       WHERE tenant_id = $1 AND parent_term_id = $2 AND position > $3"#,
                    vec![tenant_id.into(), p.into(), source_position.into()],
                ),
                None => Statement::from_sql_and_values(
                    backend,
                    r#"UPDATE taxonomy_category_hierarchy SET position = position - 1
                       WHERE tenant_id = $1 AND parent_term_id IS NULL AND position > $2"#,
                    vec![tenant_id.into(), source_position.into()],
                ),
            };
            txn.execute_raw(compact_source).await.map_err(BlogError::from)?;

            // Shift target siblings
            let shift_target = match new_parent_id {
                Some(p) => Statement::from_sql_and_values(
                    backend,
                    r#"UPDATE taxonomy_category_hierarchy SET position = position + 1
                       WHERE tenant_id = $1 AND parent_term_id = $2 AND term_id != $3 AND position >= $4"#,
                    vec![tenant_id.into(), p.into(), category_id.into(), target_position.into()],
                ),
                None => Statement::from_sql_and_values(
                    backend,
                    r#"UPDATE taxonomy_category_hierarchy SET position = position + 1
                       WHERE tenant_id = $1 AND parent_term_id IS NULL AND term_id != $2 AND position >= $3"#,
                    vec![tenant_id.into(), category_id.into(), target_position.into()],
                ),
            };
            txn.execute_raw(shift_target).await.map_err(BlogError::from)?;
        } else if source_position < target_position {
            let shift = match source_parent_id {
                Some(p) => Statement::from_sql_and_values(
                    backend,
                    r#"UPDATE taxonomy_category_hierarchy SET position = position - 1
                       WHERE tenant_id = $1 AND parent_term_id = $2 AND term_id != $3 AND position > $4 AND position <= $5"#,
                    vec![tenant_id.into(), p.into(), category_id.into(), source_position.into(), target_position.into()],
                ),
                None => Statement::from_sql_and_values(
                    backend,
                    r#"UPDATE taxonomy_category_hierarchy SET position = position - 1
                       WHERE tenant_id = $1 AND parent_term_id IS NULL AND term_id != $2 AND position > $3 AND position <= $4"#,
                    vec![tenant_id.into(), category_id.into(), source_position.into(), target_position.into()],
                ),
            };
            txn.execute_raw(shift).await.map_err(BlogError::from)?;
        } else if source_position > target_position {
            let shift = match source_parent_id {
                Some(p) => Statement::from_sql_and_values(
                    backend,
                    r#"UPDATE taxonomy_category_hierarchy SET position = position + 1
                       WHERE tenant_id = $1 AND parent_term_id = $2 AND term_id != $3 AND position >= $4 AND position < $5"#,
                    vec![tenant_id.into(), p.into(), category_id.into(), target_position.into(), source_position.into()],
                ),
                None => Statement::from_sql_and_values(
                    backend,
                    r#"UPDATE taxonomy_category_hierarchy SET position = position + 1
                       WHERE tenant_id = $1 AND parent_term_id IS NULL AND term_id != $2 AND position >= $3 AND position < $4"#,
                    vec![tenant_id.into(), category_id.into(), target_position.into(), source_position.into()],
                ),
            };
            txn.execute_raw(shift).await.map_err(BlogError::from)?;
        }

        // Update category placement
        let update_cat = Statement::from_sql_and_values(
            backend,
            r#"UPDATE taxonomy_category_hierarchy SET parent_term_id = $1, position = $2
               WHERE tenant_id = $3 AND term_id = $4"#,
            vec![
                new_parent_id.into(),
                target_position.into(),
                tenant_id.into(),
                category_id.into(),
            ],
        );
        txn.execute_raw(update_cat).await.map_err(BlogError::from)?;

        // Return updated placements for all affected categories in tenant
        let fetch_affected = Statement::from_sql_and_values(
            backend,
            r#"SELECT term_id, parent_term_id, position FROM taxonomy_category_hierarchy
               WHERE tenant_id = $1"#,
            vec![tenant_id.into()],
        );
        let rows = txn.query_all_raw(fetch_affected).await.map_err(BlogError::from)?;
        let mut placements = Vec::new();
        for r in rows {
            let cat_id = r.try_get::<Uuid>("", "term_id").map_err(BlogError::from)?;
            let p_id = r.try_get::<Option<Uuid>>("", "parent_term_id").ok().flatten();
            let pos = r.try_get::<i32>("", "position").unwrap_or(0);
            placements.push(BlogTaxonomyPlacement {
                category_id: cat_id,
                parent_id: p_id,
                position: pos,
            });
        }
        Ok(placements)
    }

    async fn reorder_category_siblings_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        _parent_id: Option<Uuid>,
        ordered_category_ids: &[Uuid],
    ) -> Result<(), BlogError> {
        let backend = txn.get_database_backend();
        for (index, &cat_id) in ordered_category_ids.iter().enumerate() {
            let stmt = Statement::from_sql_and_values(
                backend,
                r#"UPDATE taxonomy_category_hierarchy SET position = $1 WHERE tenant_id = $2 AND term_id = $3"#,
                vec![(index as i32).into(), tenant_id.into(), cat_id.into()],
            );
            txn.execute_raw(stmt).await.map_err(BlogError::from)?;
        }
        Ok(())
    }

    async fn delete_category_placement_and_compact_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> Result<(), BlogError> {
        let backend = txn.get_database_backend();

        // 1. Invariant: non-leaf category cannot be deleted
        let check_children = Statement::from_sql_and_values(
            backend,
            r#"SELECT term_id FROM taxonomy_category_hierarchy WHERE tenant_id = $1 AND parent_term_id = $2 LIMIT 1"#,
            vec![tenant_id.into(), category_id.into()],
        );
        let has_children = txn
            .query_one_raw(check_children)
            .await
            .map_err(BlogError::from)?
            .is_some();
        if has_children {
            return Err(BlogError::validation(
                "Category must be a leaf before deletion; move or delete its children first",
            ));
        }

        // 2. Fetch current placement
        let get_placement = Statement::from_sql_and_values(
            backend,
            r#"SELECT parent_term_id, position FROM taxonomy_category_hierarchy WHERE tenant_id = $1 AND term_id = $2"#,
            vec![tenant_id.into(), category_id.into()],
        );
        let placement = txn.query_one_raw(get_placement).await.map_err(BlogError::from)?;

        if let Some(row) = placement {
            let parent_id = row.try_get::<Option<Uuid>>("", "parent_term_id").ok().flatten();
            let pos = row.try_get::<i32>("", "position").unwrap_or(0);

            // 3. Delete placement
            let del_stmt = Statement::from_sql_and_values(
                backend,
                r#"DELETE FROM taxonomy_category_hierarchy WHERE tenant_id = $1 AND term_id = $2"#,
                vec![tenant_id.into(), category_id.into()],
            );
            txn.execute_raw(del_stmt).await.map_err(BlogError::from)?;

            // 4. Compact remaining siblings
            let compact_stmt = match parent_id {
                Some(p) => Statement::from_sql_and_values(
                    backend,
                    r#"UPDATE taxonomy_category_hierarchy SET position = position - 1
                       WHERE tenant_id = $1 AND parent_term_id = $2 AND position > $3"#,
                    vec![tenant_id.into(), p.into(), pos.into()],
                ),
                None => Statement::from_sql_and_values(
                    backend,
                    r#"UPDATE taxonomy_category_hierarchy SET position = position - 1
                       WHERE tenant_id = $1 AND parent_term_id IS NULL AND position > $2"#,
                    vec![tenant_id.into(), pos.into()],
                ),
            };
            txn.execute_raw(compact_stmt).await.map_err(BlogError::from)?;
        }

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
        let txn = conn.begin().await.map_err(BlogError::from)?;
        cleanup.cleanup_in_tx(&txn, tenant_id, category_id).await?;
        let backend = txn.get_database_backend();
        let delete_term = Statement::from_sql_and_values(
            backend,
            r#"DELETE FROM taxonomy_terms WHERE tenant_id = $1 AND id = $2"#,
            vec![tenant_id.into(), category_id.into()],
        );
        txn.execute_raw(delete_term).await.map_err(BlogError::from)?;
        txn.commit().await.map_err(BlogError::from)?;
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
        if tag_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let backend = match conn {
            BlogTaxonomyConnection::Connection(c) => c.get_database_backend(),
            BlogTaxonomyConnection::Transaction(tx) => tx.get_database_backend(),
        };

        let mut map = HashMap::new();
        for &id in tag_ids {
            let select_query = Statement::from_sql_and_values(
                backend,
                r#"SELECT t.canonical_key, t.scope_type, t.scope_value, t.created_at, tr.name, tr.slug, tr.locale
                   FROM taxonomy_terms t
                   LEFT JOIN taxonomy_term_translations tr ON tr.term_id = t.id AND (tr.locale = $3 OR tr.locale = $4)
                   WHERE t.tenant_id = $1 AND t.id = $2
                   ORDER BY CASE WHEN tr.locale = $3 THEN 0 ELSE 1 END
                   LIMIT 1"#,
                vec![
                    tenant_id.into(),
                    id.into(),
                    locale.into(),
                    fallback_locale.unwrap_or(locale).into(),
                ],
            );

            let row_res = match conn {
                BlogTaxonomyConnection::Connection(c) => c.query_one_raw(select_query).await,
                BlogTaxonomyConnection::Transaction(tx) => tx.query_one_raw(select_query).await,
            };

            if let Ok(Some(row)) = row_res {
                let canonical_key = row
                    .try_get::<String>("", "canonical_key")
                    .unwrap_or_else(|_| id.to_string());
                let name = row
                    .try_get::<String>("", "name")
                    .unwrap_or_else(|_| canonical_key.clone());
                let slug = row
                    .try_get::<String>("", "slug")
                    .unwrap_or_else(|_| canonical_key.clone());
                let eff_locale = row
                    .try_get::<String>("", "locale")
                    .unwrap_or_else(|_| locale.to_string());
                let scope_type = row
                    .try_get::<String>("", "scope_type")
                    .unwrap_or_else(|_| "module".to_string());
                let scope_value = row.try_get::<Option<String>>("", "scope_value").ok().flatten();
                let created_at = row
                    .try_get::<chrono::DateTime<chrono::FixedOffset>>("", "created_at")
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());

                map.insert(
                    id,
                    BlogTaxonomyTagSnapshot {
                        id,
                        tenant_id,
                        canonical_key,
                        name,
                        slug,
                        scope_type,
                        scope_value,
                        locale: locale.to_string(),
                        effective_locale: eff_locale,
                        created_at,
                    },
                );
            } else {
                map.insert(
                    id,
                    BlogTaxonomyTagSnapshot {
                        id,
                        tenant_id,
                        canonical_key: id.to_string(),
                        name: format!("tag-{id}"),
                        slug: format!("tag-{id}"),
                        scope_type: "module".to_string(),
                        scope_value: Some("blog".to_string()),
                        locale: locale.to_string(),
                        effective_locale: locale.to_string(),
                        created_at: chrono::Utc::now(),
                    },
                );
            }
        }
        Ok(map)
    }

    async fn load_scoped_tags(
        &self,
        _conn: BlogTaxonomyConnection<'_>,
        _tenant_id: Uuid,
        _tag_ids: Option<&[Uuid]>,
        _locale: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<Vec<BlogTaxonomyTagSnapshot>, BlogError> {
        Ok(Vec::new())
    }

    async fn resolve_tag_id_for_module(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
        tag: &str,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Option<Uuid>, BlogError> {
        let backend = match conn {
            BlogTaxonomyConnection::Connection(c) => c.get_database_backend(),
            BlogTaxonomyConnection::Transaction(tx) => tx.get_database_backend(),
        };
        let query = Statement::from_sql_and_values(
            backend,
            r#"SELECT t.id
               FROM taxonomy_terms t
               JOIN taxonomy_term_translations tr ON tr.term_id = t.id
               WHERE t.tenant_id = $1 AND t.kind = 'tag'
                 AND (t.scope_type = 'global' OR (t.scope_type = 'module' AND t.scope_value = 'blog'))
                 AND (tr.locale = $2 OR tr.locale = $3)
                 AND (tr.name = $4 OR tr.slug = $4 OR t.canonical_key = $4)
               LIMIT 1"#,
            vec![
                tenant_id.into(),
                locale.into(),
                fallback_locale.unwrap_or(locale).into(),
                tag.into(),
            ],
        );
        let row_res = match conn {
            BlogTaxonomyConnection::Connection(c) => c.query_one_raw(query).await,
            BlogTaxonomyConnection::Transaction(tx) => tx.query_one_raw(query).await,
        };
        if let Ok(Some(row)) = row_res {
            return Ok(row.try_get::<Uuid>("", "id").ok());
        }
        Ok(None)
    }

    async fn find_all_valid_tag_ids(
        &self,
        conn: BlogTaxonomyConnection<'_>,
        tenant_id: Uuid,
    ) -> Result<HashSet<Uuid>, BlogError> {
        let backend = match conn {
            BlogTaxonomyConnection::Connection(c) => c.get_database_backend(),
            BlogTaxonomyConnection::Transaction(tx) => tx.get_database_backend(),
        };
        let query = Statement::from_sql_and_values(
            backend,
            r#"SELECT id FROM taxonomy_terms WHERE tenant_id = $1 AND kind = 'tag' AND (scope_type = 'global' OR (scope_type = 'module' AND scope_value = 'blog'))"#,
            vec![tenant_id.into()],
        );
        let rows_res = match conn {
            BlogTaxonomyConnection::Connection(c) => c.query_all_raw(query).await,
            BlogTaxonomyConnection::Transaction(tx) => tx.query_all_raw(query).await,
        };
        if let Ok(rows) = rows_res {
            let mut set = HashSet::new();
            for r in rows {
                set.insert(r.try_get::<Uuid>("", "id")?);
            }
            if !set.is_empty() {
                return Ok(set);
            }
        }
        let usage_query = Statement::from_sql_and_values(
            backend,
            r#"SELECT tag_id FROM blog_tag_usage WHERE tenant_id = $1"#,
            vec![tenant_id.into()],
        );
        let usage_res = match conn {
            BlogTaxonomyConnection::Connection(c) => c.query_all_raw(usage_query).await,
            BlogTaxonomyConnection::Transaction(tx) => tx.query_all_raw(usage_query).await,
        };
        if let Ok(rows) = usage_res {
            let mut set = HashSet::new();
            for r in rows {
                set.insert(r.try_get::<Uuid>("", "tag_id")?);
            }
            return Ok(set);
        }
        Ok(HashSet::new())
    }

    async fn ensure_module_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        tag_names: &[String],
        _allow_create: bool,
    ) -> Result<Vec<Uuid>, BlogError> {
        let backend = txn.get_database_backend();
        let mut ids = Vec::with_capacity(tag_names.len());
        for name in tag_names {
            let slug = crate::normalization::normalize_term_route_key(name)
                .unwrap_or_else(|| name.to_ascii_lowercase());

            let select_term = Statement::from_sql_and_values(
                backend,
                r#"SELECT id FROM taxonomy_terms
                   WHERE tenant_id = $1 AND kind = 'tag'
                     AND (scope_type = 'global' OR (scope_type = 'module' AND scope_value = 'blog'))
                     AND (canonical_key = $2 OR id IN (SELECT term_id FROM taxonomy_term_translations WHERE name = $3))
                   LIMIT 1"#,
                vec![tenant_id.into(), slug.clone().into(), name.clone().into()],
            );
            let existing_id = match txn.query_one_raw(select_term).await {
                Ok(Some(row)) => row.try_get::<Uuid>("", "id").ok(),
                _ => None,
            };

            let id = if let Some(existing) = existing_id {
                existing
            } else {
                let new_id = Uuid::new_v4();
                let insert_term = Statement::from_sql_and_values(
                    backend,
                    r#"INSERT INTO taxonomy_terms (id, tenant_id, kind, scope_type, scope_value, canonical_key, created_at, updated_at)
                       VALUES ($1, $2, 'tag', 'module', 'blog', $3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
                    vec![new_id.into(), tenant_id.into(), slug.clone().into()],
                );
                txn.execute_raw(insert_term)
                    .await
                    .map_err(BlogError::from)?;
                let tr_id = Uuid::new_v4();
                let insert_tr = Statement::from_sql_and_values(
                    backend,
                    r#"INSERT INTO taxonomy_term_translations (id, term_id, tenant_id, locale, name, slug, created_at, updated_at)
                       VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
                    vec![
                        tr_id.into(),
                        new_id.into(),
                        tenant_id.into(),
                        locale.into(),
                        name.clone().into(),
                        slug.into(),
                    ],
                );
                txn.execute_raw(insert_tr)
                    .await
                    .map_err(BlogError::from)?;
                new_id
            };
            ids.push(id);
        }
        Ok(ids)
    }

    async fn create_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: BlogTaxonomyCreateTagInput,
    ) -> Result<BlogTaxonomyCreatedTag, BlogError> {
        let id = Uuid::new_v4();
        let canonical_key = input.slug.clone().unwrap_or_else(|| {
            crate::normalization::normalize_term_route_key(&input.name)
                .unwrap_or_else(|| id.to_string())
        });
        let slug = input.slug.clone().unwrap_or_else(|| {
            crate::normalization::normalize_term_route_key(&input.name)
                .unwrap_or_else(|| id.to_string())
        });

        let backend = txn.get_database_backend();
        let insert_term = Statement::from_sql_and_values(
            backend,
            r#"INSERT INTO taxonomy_terms (id, tenant_id, kind, scope_type, scope_value, canonical_key, created_at, updated_at)
               VALUES ($1, $2, 'tag', 'module', 'blog', $3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
            vec![id.into(), tenant_id.into(), canonical_key.clone().into()],
        );
        txn.execute_raw(insert_term)
            .await
            .map_err(BlogError::from)?;
        let tr_id = Uuid::new_v4();
        let insert_tr = Statement::from_sql_and_values(
            backend,
            r#"INSERT INTO taxonomy_term_translations (id, term_id, tenant_id, locale, name, slug, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
            vec![
                tr_id.into(),
                id.into(),
                tenant_id.into(),
                input.locale.into(),
                input.name.into(),
                slug.into(),
            ],
        );
        txn.execute_raw(insert_tr)
            .await
            .map_err(BlogError::from)?;

        Ok(BlogTaxonomyCreatedTag { id, canonical_key })
    }

    async fn update_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        tag_id: Uuid,
        _security: &SecurityContext,
        input: BlogTaxonomyUpdateTagInput,
    ) -> Result<BlogTaxonomyTagSnapshot, BlogError> {
        let backend = txn.get_database_backend();

        let select_tr = Statement::from_sql_and_values(
            backend,
            r#"SELECT tr.name, tr.slug, t.canonical_key, t.scope_type, t.scope_value, t.created_at
               FROM taxonomy_terms t
               LEFT JOIN taxonomy_term_translations tr ON tr.term_id = t.id AND tr.locale = $3
               WHERE t.tenant_id = $1 AND t.id = $2
               LIMIT 1"#,
            vec![tenant_id.into(), tag_id.into(), input.locale.clone().into()],
        );
        let existing_row = txn.query_one_raw(select_tr).await.map_err(BlogError::from)?;

        let mut existing_name = format!("tag-{tag_id}");
        let mut existing_slug = format!("tag-{tag_id}");
        let mut canonical_key = tag_id.to_string();
        let mut scope_type = "module".to_string();
        let mut scope_value = Some("blog".to_string());
        let mut created_at = chrono::Utc::now();

        if let Some(ref row) = existing_row {
            if let Ok(name) = row.try_get::<String>("", "name") {
                existing_name = name;
            }
            if let Ok(slug) = row.try_get::<String>("", "slug") {
                existing_slug = slug;
            }
            if let Ok(ck) = row.try_get::<String>("", "canonical_key") {
                canonical_key = ck;
            }
            if let Ok(st) = row.try_get::<String>("", "scope_type") {
                scope_type = st;
            }
            if let Ok(sv) = row.try_get::<Option<String>>("", "scope_value") {
                scope_value = sv;
            }
            if let Ok(ca) = row.try_get::<chrono::DateTime<chrono::FixedOffset>>("", "created_at") {
                created_at = ca.with_timezone(&chrono::Utc);
            }
        }

        let new_name = input.name.unwrap_or(existing_name);
        let new_slug = if let Some(slug) = input.slug {
            crate::normalization::normalize_term_route_key(&slug).unwrap_or(slug)
        } else if new_name != format!("tag-{tag_id}") {
            crate::normalization::normalize_term_route_key(&new_name)
                .unwrap_or_else(|| new_name.to_ascii_lowercase())
        } else {
            existing_slug
        };

        let update_term = Statement::from_sql_and_values(
            backend,
            r#"UPDATE taxonomy_terms SET updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND id = $2"#,
            vec![tenant_id.into(), tag_id.into()],
        );
        txn.execute_raw(update_term).await.map_err(BlogError::from)?;

        let select_existing_tr = Statement::from_sql_and_values(
            backend,
            r#"SELECT id FROM taxonomy_term_translations WHERE term_id = $1 AND locale = $2"#,
            vec![tag_id.into(), input.locale.clone().into()],
        );
        let tr_exists = txn.query_one_raw(select_existing_tr).await.map_err(BlogError::from)?.is_some();

        if tr_exists {
            let update_tr = Statement::from_sql_and_values(
                backend,
                r#"UPDATE taxonomy_term_translations SET name = $1, slug = $2, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $3 AND term_id = $4 AND locale = $5"#,
                vec![
                    new_name.clone().into(),
                    new_slug.clone().into(),
                    tenant_id.into(),
                    tag_id.into(),
                    input.locale.clone().into(),
                ],
            );
            txn.execute_raw(update_tr).await.map_err(BlogError::from)?;
        } else {
            let insert_tr = Statement::from_sql_and_values(
                backend,
                r#"INSERT INTO taxonomy_term_translations (id, term_id, tenant_id, locale, name, slug, created_at, updated_at)
                   VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
                vec![
                    Uuid::new_v4().into(),
                    tag_id.into(),
                    tenant_id.into(),
                    input.locale.clone().into(),
                    new_name.clone().into(),
                    new_slug.clone().into(),
                ],
            );
            txn.execute_raw(insert_tr).await.map_err(BlogError::from)?;
        }

        Ok(BlogTaxonomyTagSnapshot {
            id: tag_id,
            tenant_id,
            canonical_key,
            name: new_name,
            slug: new_slug,
            scope_type,
            scope_value,
            locale: input.locale.clone(),
            effective_locale: input.locale,
            created_at,
        })
    }

    async fn delete_tag_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        tag_id: Uuid,
        _security: &SecurityContext,
    ) -> Result<(), BlogError> {
        let backend = txn.get_database_backend();
        let delete_term = Statement::from_sql_and_values(
            backend,
            r#"DELETE FROM taxonomy_terms WHERE tenant_id = $1 AND id = $2"#,
            vec![tenant_id.into(), tag_id.into()],
        );
        txn.execute_raw(delete_term).await.map_err(BlogError::from)?;
        let delete_usage = Statement::from_sql_and_values(
            backend,
            r#"DELETE FROM blog_tag_usage WHERE tenant_id = $1 AND tag_id = $2"#,
            vec![tenant_id.into(), tag_id.into()],
        );
        txn.execute_raw(delete_usage).await.map_err(BlogError::from)?;
        Ok(())
    }

    async fn lock_tag_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _tag_id: Uuid,
    ) -> Result<(), BlogError> {
        Ok(())
    }
}
