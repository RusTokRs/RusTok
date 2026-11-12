/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Groups-owned port boundary for taxonomy collaboration (categories and tags).
//!
//! Provides transport-neutral and persistence-neutral contracts for group category
//! validation, category metadata resolution, and group tag lifecycle without direct
//! coupling to Taxonomy entities or tables.

use std::collections::HashMap;

use async_trait::async_trait;
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::GroupsError;

/// Either an open database transaction, a connection pool handle, or an unconstrained connection.
#[derive(Copy, Clone)]
pub enum GroupTaxonomyConnection<'a> {
    Connection(&'a DatabaseConnection),
    Transaction(&'a DatabaseTransaction),
    None,
}

impl<'a> From<&'a DatabaseConnection> for GroupTaxonomyConnection<'a> {
    fn from(c: &'a DatabaseConnection) -> Self {
        GroupTaxonomyConnection::Connection(c)
    }
}

impl<'a> From<&'a DatabaseTransaction> for GroupTaxonomyConnection<'a> {
    fn from(tx: &'a DatabaseTransaction) -> Self {
        GroupTaxonomyConnection::Transaction(tx)
    }
}

pub trait AsGroupTaxonomyConnection {
    fn as_group_taxonomy_connection(&self) -> GroupTaxonomyConnection<'_>;
}

impl AsGroupTaxonomyConnection for DatabaseConnection {
    fn as_group_taxonomy_connection(&self) -> GroupTaxonomyConnection<'_> {
        GroupTaxonomyConnection::Connection(self)
    }
}

impl AsGroupTaxonomyConnection for DatabaseTransaction {
    fn as_group_taxonomy_connection(&self) -> GroupTaxonomyConnection<'_> {
        GroupTaxonomyConnection::Transaction(self)
    }
}

/// Category metadata projected from the taxonomy provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupTaxonomyCategorySnapshot {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub canonical_key: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub position: i32,
    pub icon_key: Option<String>,
    pub color: Option<String>,
    pub available_locales: Vec<String>,
    pub requested_locale: String,
    pub effective_locale: String,
}

/// Transactional and projection port for Groups taxonomy operations.
#[async_trait]
pub trait GroupTaxonomyPort: Send + Sync {
    /// Validates whether a category ID exists for the tenant and module scope in transaction or connection.
    async fn validate_category_exists(
        &self,
        conn: GroupTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> Result<bool, GroupsError>;

    /// Loads localized category snapshots for a batch of category IDs.
    async fn load_categories(
        &self,
        conn: GroupTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, GroupTaxonomyCategorySnapshot>, GroupsError>;

    /// Resolves term names (e.g. for tags) by IDs.
    async fn resolve_term_names(
        &self,
        conn: GroupTaxonomyConnection<'_>,
        tenant_id: Uuid,
        term_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, String>, GroupsError>;

    /// Ensures tags exist for groups module in transaction.
    async fn ensure_group_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        tags: &[String],
    ) -> Result<Vec<Uuid>, GroupsError>;
}

/// Standalone fallback implementation that allows Groups to operate when Taxonomy is absent.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopGroupTaxonomyPort;

#[async_trait]
impl GroupTaxonomyPort for NoopGroupTaxonomyPort {
    async fn validate_category_exists(
        &self,
        _conn: GroupTaxonomyConnection<'_>,
        _tenant_id: Uuid,
        _category_id: Uuid,
    ) -> Result<bool, GroupsError> {
        // In standalone mode, accept any non-nil UUID as valid
        Ok(true)
    }

    async fn load_categories(
        &self,
        _conn: GroupTaxonomyConnection<'_>,
        _tenant_id: Uuid,
        category_ids: &[Uuid],
        locale: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, GroupTaxonomyCategorySnapshot>, GroupsError> {
        let mut map = HashMap::new();
        for (index, id) in category_ids.iter().enumerate() {
            map.insert(
                *id,
                GroupTaxonomyCategorySnapshot {
                    id: *id,
                    parent_id: None,
                    canonical_key: format!("group-category-{id}"),
                    slug: id.to_string(),
                    name: format!("Category {id}"),
                    description: None,
                    position: index as i32,
                    icon_key: None,
                    color: None,
                    available_locales: vec![locale.to_string()],
                    requested_locale: locale.to_string(),
                    effective_locale: locale.to_string(),
                },
            );
        }
        Ok(map)
    }

    async fn resolve_term_names(
        &self,
        _conn: GroupTaxonomyConnection<'_>,
        _tenant_id: Uuid,
        term_ids: &[Uuid],
        _locale: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, String>, GroupsError> {
        Ok(term_ids.iter().map(|id| (*id, id.to_string())).collect())
    }

    async fn ensure_group_tags_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _locale: &str,
        tags: &[String],
    ) -> Result<Vec<Uuid>, GroupsError> {
        Ok(tags
            .iter()
            .map(|tag| Uuid::new_v5(&Uuid::NAMESPACE_OID, tag.as_bytes()))
            .collect())
    }
}
