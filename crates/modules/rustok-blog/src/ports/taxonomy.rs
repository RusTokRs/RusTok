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
use sea_orm::{DatabaseConnection, DatabaseTransaction};
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
        _conn: BlogTaxonomyConnection<'_>,
        _tenant_id: Uuid,
        _tag_ids: &[Uuid],
        _locale: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, BlogTaxonomyTagSnapshot>, BlogError> {
        Ok(HashMap::new())
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
        _conn: BlogTaxonomyConnection<'_>,
        _tenant_id: Uuid,
        _tag: &str,
        _locale: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<Option<Uuid>, BlogError> {
        Ok(None)
    }

    async fn find_all_valid_tag_ids(
        &self,
        _conn: BlogTaxonomyConnection<'_>,
        _tenant_id: Uuid,
    ) -> Result<HashSet<Uuid>, BlogError> {
        Ok(HashSet::new())
    }

    async fn ensure_module_tags_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _locale: &str,
        tag_names: &[String],
        _allow_create: bool,
    ) -> Result<Vec<Uuid>, BlogError> {
        Ok(tag_names.iter().map(|_| Uuid::new_v4()).collect())
    }

    async fn create_tag_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        input: BlogTaxonomyCreateTagInput,
    ) -> Result<BlogTaxonomyCreatedTag, BlogError> {
        let id = Uuid::new_v4();
        Ok(BlogTaxonomyCreatedTag {
            id,
            canonical_key: input.slug.unwrap_or_else(|| id.to_string()),
        })
    }

    async fn update_tag_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        tag_id: Uuid,
        _security: &SecurityContext,
        input: BlogTaxonomyUpdateTagInput,
    ) -> Result<BlogTaxonomyTagSnapshot, BlogError> {
        Ok(BlogTaxonomyTagSnapshot {
            id: tag_id,
            tenant_id: _tenant_id,
            canonical_key: tag_id.to_string(),
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
