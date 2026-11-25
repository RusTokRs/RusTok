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
//! Native category and tag integration. Related writes can participate in the same
//! database transaction without exposing Taxonomy entities or tables.

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

/// Distribution binds the native provider to the host's database.
#[derive(Clone, Copy)]
pub struct GroupTaxonomyPortFactory(pub fn(DatabaseConnection) -> super::SharedGroupTaxonomyPort);
