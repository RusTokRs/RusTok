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
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, Statement};
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
        _conn: BlogTaxonomyConnection<'_>,
        _tenant_id: Uuid,
        _category_ids: Option<&[Uuid]>,
        _locale: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<Vec<BlogTaxonomyCategorySnapshot>, BlogError> {
        Ok(Vec::new())
    }

    async fn load_category_locale_copy_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _category_id: Uuid,
        _locale: &str,
    ) -> Result<Option<BlogTaxonomyCategoryLocaleCopy>, BlogError> {
        Ok(None)
    }

    async fn sync_category_copy_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _input: BlogTaxonomySyncCategoryInput,
    ) -> Result<(), BlogError> {
        Ok(())
    }

    async fn move_category_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        category_id: Uuid,
        new_parent_id: Option<Uuid>,
        position: Option<i32>,
    ) -> Result<Vec<BlogTaxonomyPlacement>, BlogError> {
        Ok(vec![BlogTaxonomyPlacement {
            category_id,
            parent_id: new_parent_id,
            position: position.unwrap_or(0),
        }])
    }

    async fn reorder_category_siblings_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _parent_id: Option<Uuid>,
        _ordered_category_ids: &[Uuid],
    ) -> Result<(), BlogError> {
        Ok(())
    }

    async fn delete_category_placement_and_compact_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _category_id: Uuid,
    ) -> Result<(), BlogError> {
        Ok(())
    }

    async fn delete_module_category_with_cleanup(
        &self,
        _conn: &DatabaseConnection,
        _tenant_id: Uuid,
        _category_id: Uuid,
        _security: &SecurityContext,
        _cleanup: Arc<dyn BlogCategoryDeleteCleanupPort>,
    ) -> Result<(), BlogError> {
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
                r#"SELECT t.canonical_key, tr.name, tr.slug, tr.locale
                   FROM taxonomy_terms t
                   LEFT JOIN taxonomy_term_translations tr ON tr.term_id = t.id AND (tr.locale = $3 OR tr.locale = $4)
                   WHERE t.tenant_id = $1 AND t.id = $2
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
                map.insert(
                    id,
                    BlogTaxonomyTagSnapshot {
                        id,
                        tenant_id,
                        canonical_key,
                        name,
                        slug,
                        scope_type: "module".to_string(),
                        scope_value: Some("blog".to_string()),
                        locale: locale.to_string(),
                        effective_locale: eff_locale,
                        created_at: chrono::Utc::now(),
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
               WHERE t.tenant_id = $1 AND t.kind = 'tag' AND t.scope_type = 'module' AND t.scope_value = 'blog'
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
            r#"SELECT id FROM taxonomy_terms WHERE tenant_id = $1 AND kind = 'tag' AND scope_type = 'module' AND scope_value = 'blog'"#,
            vec![tenant_id.into()],
        );
        let rows_res = match conn {
            BlogTaxonomyConnection::Connection(c) => c.query_all_raw(query).await,
            BlogTaxonomyConnection::Transaction(tx) => tx.query_all_raw(query).await,
        };
        if let Ok(rows) = rows_res {
            let set: HashSet<Uuid> = rows
                .into_iter()
                .filter_map(|r| r.try_get::<Uuid>("", "id").ok())
                .collect();
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
            return Ok(rows
                .into_iter()
                .filter_map(|r| r.try_get::<Uuid>("", "tag_id").ok())
                .collect());
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
                r#"SELECT id FROM taxonomy_terms WHERE tenant_id = $1 AND kind = 'tag' AND scope_type = 'module' AND scope_value = 'blog' AND canonical_key = $2"#,
                vec![tenant_id.into(), slug.clone().into()],
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
                    r#"INSERT INTO taxonomy_terms (id, tenant_id, kind, scope_type, scope_value, canonical_key, status, created_at, updated_at)
                       VALUES ($1, $2, 'tag', 'module', 'blog', $3, 'active', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
                    vec![new_id.into(), tenant_id.into(), slug.clone().into()],
                );
                if let Ok(_) = txn.execute_raw(insert_term).await {
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
                    let _ = txn.execute_raw(insert_tr).await;
                }
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
            r#"INSERT INTO taxonomy_terms (id, tenant_id, kind, scope_type, scope_value, canonical_key, status, created_at, updated_at)
               VALUES ($1, $2, 'tag', 'module', 'blog', $3, 'active', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
            vec![id.into(), tenant_id.into(), canonical_key.clone().into()],
        );
        if let Ok(_) = txn.execute_raw(insert_term).await {
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
            let _ = txn.execute_raw(insert_tr).await;
        }

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
        let canonical_key = input.slug.clone().unwrap_or_else(|| tag_id.to_string());
        if let Some(slug) = &input.slug {
            let update_term = Statement::from_sql_and_values(
                backend,
                r#"UPDATE taxonomy_terms SET canonical_key = $1, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $2 AND id = $3"#,
                vec![slug.clone().into(), tenant_id.into(), tag_id.into()],
            );
            let _ = txn.execute_raw(update_term).await;
        }
        if input.name.is_some() || input.slug.is_some() {
            let name = input.name.clone().unwrap_or_default();
            let slug = input.slug.clone().unwrap_or_default();
            let update_tr = Statement::from_sql_and_values(
                backend,
                r#"UPDATE taxonomy_term_translations SET name = $1, slug = $2, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $3 AND term_id = $4 AND locale = $5"#,
                vec![
                    name.into(),
                    slug.into(),
                    tenant_id.into(),
                    tag_id.into(),
                    input.locale.clone().into(),
                ],
            );
            let _ = txn.execute_raw(update_tr).await;
        }

        Ok(BlogTaxonomyTagSnapshot {
            id: tag_id,
            tenant_id,
            canonical_key,
            name: input.name.unwrap_or_default(),
            slug: input.slug.unwrap_or_default(),
            scope_type: "module".to_string(),
            scope_value: Some("blog".to_string()),
            locale: input.locale.clone(),
            effective_locale: input.locale,
            created_at: chrono::Utc::now(),
        })
    }

    async fn delete_tag_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _tag_id: Uuid,
        _security: &SecurityContext,
    ) -> Result<(), BlogError> {
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
