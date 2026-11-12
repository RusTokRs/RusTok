/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Forum-owned port boundary for taxonomy collaboration (categories, hierarchy, and tags).
//!
//! Provides transport-neutral and persistence-neutral contracts for forum category
//! hierarchy synchronization, localized copy loading, route resolution, and topic tag
//! lifecycle management without direct coupling to Taxonomy entities or tables.

use std::collections::HashMap;

use async_trait::async_trait;
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use crate::error::ForumError;

/// Either an open database transaction or a connection pool handle.
#[derive(Copy, Clone)]
pub enum ForumTaxonomyConnection<'a> {
    Connection(&'a DatabaseConnection),
    Transaction(&'a DatabaseTransaction),
    None,
}

impl<'a> From<&'a DatabaseConnection> for ForumTaxonomyConnection<'a> {
    fn from(c: &'a DatabaseConnection) -> Self {
        ForumTaxonomyConnection::Connection(c)
    }
}

impl<'a> From<&'a DatabaseTransaction> for ForumTaxonomyConnection<'a> {
    fn from(tx: &'a DatabaseTransaction) -> Self {
        ForumTaxonomyConnection::Transaction(tx)
    }
}

pub trait AsForumTaxonomyConnection {
    fn as_forum_taxonomy_connection(&self) -> ForumTaxonomyConnection<'_>;
}

impl AsForumTaxonomyConnection for DatabaseConnection {
    fn as_forum_taxonomy_connection(&self) -> ForumTaxonomyConnection<'_> {
        ForumTaxonomyConnection::Connection(self)
    }
}

impl AsForumTaxonomyConnection for DatabaseTransaction {
    fn as_forum_taxonomy_connection(&self) -> ForumTaxonomyConnection<'_> {
        ForumTaxonomyConnection::Transaction(self)
    }
}

/// Category metadata projected from the taxonomy provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTaxonomyCategorySnapshot {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    pub canonical_key: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub icon_key: Option<String>,
    pub color: Option<String>,
    pub available_locales: Vec<String>,
    pub requested_locale: String,
    pub effective_locale: String,
}

/// Localized copy for an existing category term in taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTaxonomyCategoryLocaleCopy {
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
}

/// Input for synchronizing a forum category translation and placement into taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTaxonomySyncCategoryInput {
    pub category_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    pub icon_key: Option<String>,
    pub color: Option<String>,
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
}

/// Structural category placement response after a hierarchy move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTaxonomyPlacement {
    pub term_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub position: i32,
}

/// Category route resolution result from taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTaxonomyRouteMatch {
    pub term_id: Uuid,
    pub matched_locale: String,
    pub alias_id: Option<Uuid>,
}

/// Transactional and projection port for Forum taxonomy operations.
#[async_trait]
pub trait ForumTaxonomyPort: Send + Sync {
    // --- Hierarchy & Lock methods ---

    /// Locks category hierarchy writer within the transaction to serialize structural modifications.
    async fn lock_category_hierarchy_writer_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> Result<(), ForumError>;

    /// Loads parent mappings for a batch of category IDs.
    async fn load_category_parents(
        &self,
        conn: ForumTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Option<Uuid>>, ForumError>;

    /// Loads sibling category IDs under a parent in transaction.
    async fn load_category_siblings_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        parent_id: Option<Uuid>,
    ) -> Result<Vec<Uuid>, ForumError>;

    /// Shifts siblings for insert in transaction.
    async fn shift_category_siblings_for_insert_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        parent_id: Option<Uuid>,
        requested_position: i32,
    ) -> Result<(), ForumError>;

    /// Repositions a category in the hierarchy within a transaction.
    async fn move_category_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        parent_id: Option<Uuid>,
        position: i32,
    ) -> Result<Vec<ForumTaxonomyPlacement>, ForumError>;

    /// Reorders sibling categories under a parent in transaction.
    async fn reorder_category_siblings_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        parent_id: Option<Uuid>,
        ordered_ids: &[Uuid],
    ) -> Result<(), ForumError>;

    // --- Category Localized Copy & Projection methods ---

    /// Loads localized category snapshots for the specified IDs and locale.
    async fn load_scoped_categories(
        &self,
        conn: ForumTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: Option<&[Uuid]>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Vec<ForumTaxonomyCategorySnapshot>, ForumError>;

    /// Loads the localized copy of a single category term in transaction.
    async fn load_category_locale_copy_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
    ) -> Result<Option<ForumTaxonomyCategoryLocaleCopy>, ForumError>;

    /// Synchronizes a created or updated forum category copy and hierarchy into taxonomy in transaction.
    async fn sync_category_copy_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: ForumTaxonomySyncCategoryInput,
    ) -> Result<(), ForumError>;

    // --- Route Resolution ---

    /// Resolves category term route and alias by slug and locale.
    async fn resolve_category_route(
        &self,
        conn: ForumTaxonomyConnection<'_>,
        tenant_id: Uuid,
        requested_locale: &str,
        requested_slug: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Option<ForumTaxonomyRouteMatch>, ForumError>;

    // --- Tag Dictionary methods ---

    /// Resolves human-readable term names for topic tags by tenant and locale.
    async fn resolve_term_names(
        &self,
        conn: ForumTaxonomyConnection<'_>,
        tenant_id: Uuid,
        term_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, String>, ForumError>;

    /// Ensures topic tag terms exist in taxonomy in transaction, returning term UUIDs.
    async fn ensure_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        tags: &[String],
    ) -> Result<Vec<Uuid>, ForumError>;
}

/// Fallback no-op taxonomy port when taxonomy module is disabled or unconfigured.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopForumTaxonomyPort;

#[async_trait]
impl ForumTaxonomyPort for NoopForumTaxonomyPort {
    async fn lock_category_hierarchy_writer_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
    ) -> Result<(), ForumError> {
        Ok(())
    }

    async fn load_category_parents(
        &self,
        _conn: ForumTaxonomyConnection<'_>,
        _tenant_id: Uuid,
        category_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Option<Uuid>>, ForumError> {
        Ok(category_ids.iter().map(|id| (*id, None)).collect())
    }

    async fn load_category_siblings_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _parent_id: Option<Uuid>,
    ) -> Result<Vec<Uuid>, ForumError> {
        Ok(Vec::new())
    }

    async fn shift_category_siblings_for_insert_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _category_id: Uuid,
        _parent_id: Option<Uuid>,
        _requested_position: i32,
    ) -> Result<(), ForumError> {
        Ok(())
    }

    async fn move_category_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        category_id: Uuid,
        parent_id: Option<Uuid>,
        position: i32,
    ) -> Result<Vec<ForumTaxonomyPlacement>, ForumError> {
        Ok(vec![ForumTaxonomyPlacement {
            term_id: category_id,
            parent_id,
            position,
        }])
    }

    async fn reorder_category_siblings_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _parent_id: Option<Uuid>,
        _ordered_ids: &[Uuid],
    ) -> Result<(), ForumError> {
        Ok(())
    }

    async fn load_scoped_categories(
        &self,
        _conn: ForumTaxonomyConnection<'_>,
        _tenant_id: Uuid,
        category_ids: Option<&[Uuid]>,
        locale: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<Vec<ForumTaxonomyCategorySnapshot>, ForumError> {
        let Some(ids) = category_ids else {
            return Ok(Vec::new());
        };
        Ok(ids
            .iter()
            .enumerate()
            .map(|(index, id)| ForumTaxonomyCategorySnapshot {
                id: *id,
                parent_id: None,
                position: index as i32,
                canonical_key: format!("forum-category-{id}"),
                slug: id.to_string(),
                name: id.to_string(),
                description: None,
                icon_key: None,
                color: None,
                available_locales: vec![locale.to_string()],
                requested_locale: locale.to_string(),
                effective_locale: locale.to_string(),
            })
            .collect())
    }

    async fn load_category_locale_copy_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _category_id: Uuid,
        _locale: &str,
    ) -> Result<Option<ForumTaxonomyCategoryLocaleCopy>, ForumError> {
        Ok(None)
    }

    async fn sync_category_copy_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _input: ForumTaxonomySyncCategoryInput,
    ) -> Result<(), ForumError> {
        Ok(())
    }

    async fn resolve_category_route(
        &self,
        _conn: ForumTaxonomyConnection<'_>,
        _tenant_id: Uuid,
        _requested_locale: &str,
        _requested_slug: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<Option<ForumTaxonomyRouteMatch>, ForumError> {
        Ok(None)
    }

    async fn resolve_term_names(
        &self,
        _conn: ForumTaxonomyConnection<'_>,
        _tenant_id: Uuid,
        term_ids: &[Uuid],
        _locale: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, String>, ForumError> {
        Ok(term_ids.iter().map(|id| (*id, id.to_string())).collect())
    }

    async fn ensure_tags_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _locale: &str,
        tags: &[String],
    ) -> Result<Vec<Uuid>, ForumError> {
        Ok(tags
            .iter()
            .map(|tag| uuid::Uuid::new_v5(&Uuid::NAMESPACE_OID, tag.as_bytes()))
            .collect())
    }
}
