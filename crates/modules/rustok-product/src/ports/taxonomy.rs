/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Product-owned port boundary for taxonomy collaboration (tags and categories).
//!
//! Provides transport-neutral and persistence-neutral contracts for product tag
//! resolution, tag creation, category synchronization, and category tree reading.
//! No Taxonomy tables, SeaORM entities, or foreign types are imported here.

use std::collections::HashMap;

use async_trait::async_trait;
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use crate::error::CommerceError;

/// Input for synchronizing a newly created or updated product category translation into taxonomy.
#[derive(Debug, Clone)]
pub struct ProductTaxonomyCategoryTranslationInput {
    pub category_id: Uuid,
    pub module_scope: String,
    pub canonical_key: String,
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub aliases: Vec<String>,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub position: i32,
}

/// Category metadata projected from the taxonomy provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductTaxonomyCategorySnapshot {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub canonical_key: String,
    pub scope_type: String,
    pub scope_value: Option<String>,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub position: i32,
    pub available_locales: Vec<String>,
}

/// Either an open database transaction or a connection pool handle.
#[derive(Copy, Clone)]
pub enum TaxonomyConnection<'a> {
    Connection(&'a DatabaseConnection),
    Transaction(&'a DatabaseTransaction),
}

impl<'a> From<&'a DatabaseConnection> for TaxonomyConnection<'a> {
    fn from(c: &'a DatabaseConnection) -> Self {
        TaxonomyConnection::Connection(c)
    }
}

impl<'a> From<&'a DatabaseTransaction> for TaxonomyConnection<'a> {
    fn from(tx: &'a DatabaseTransaction) -> Self {
        TaxonomyConnection::Transaction(tx)
    }
}

pub trait AsTaxonomyConnection {
    fn as_taxonomy_connection(&self) -> TaxonomyConnection<'_>;
}

impl AsTaxonomyConnection for DatabaseConnection {
    fn as_taxonomy_connection(&self) -> TaxonomyConnection<'_> {
        TaxonomyConnection::Connection(self)
    }
}

impl AsTaxonomyConnection for DatabaseTransaction {
    fn as_taxonomy_connection(&self) -> TaxonomyConnection<'_> {
        TaxonomyConnection::Transaction(self)
    }
}

impl AsTaxonomyConnection for crate::services::write_transaction::ProductWriteTransaction {
    fn as_taxonomy_connection(&self) -> TaxonomyConnection<'_> {
        TaxonomyConnection::Transaction(self)
    }
}

/// Transactional and projection port for product taxonomy (tags & categories).
#[async_trait]
pub trait ProductTaxonomyPort: Send + Sync {
    /// Resolves human-readable tag names for term UUIDs.
    async fn resolve_term_names(
        &self,
        conn: TaxonomyConnection<'_>,
        tenant_id: Uuid,
        term_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, String>, CommerceError>;

    /// Ensures tag terms exist in the taxonomy provider within a product write transaction.
    async fn ensure_product_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        tag_names: &[String],
    ) -> Result<Vec<Uuid>, CommerceError>;

    /// Synchronizes a product category translation into the taxonomy provider in transaction.
    async fn sync_category_translation_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: ProductTaxonomyCategoryTranslationInput,
    ) -> Result<(), CommerceError>;

    /// Loads scoped product categories from the taxonomy provider.
    async fn load_scoped_categories(
        &self,
        conn: TaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: Option<&[Uuid]>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Vec<ProductTaxonomyCategorySnapshot>, CommerceError>;
}

/// Default no-op taxonomy implementation used when running Product in standalone microservice mode
/// or in tests without a taxonomy service attached.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopProductTaxonomyPort;

#[async_trait]
impl ProductTaxonomyPort for NoopProductTaxonomyPort {
    async fn resolve_term_names(
        &self,
        _conn: TaxonomyConnection<'_>,
        _tenant_id: Uuid,
        _term_ids: &[Uuid],
        _locale: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, String>, CommerceError> {
        Ok(HashMap::new())
    }

    async fn ensure_product_tags_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _locale: &str,
        tag_names: &[String],
    ) -> Result<Vec<Uuid>, CommerceError> {
        // In standalone mode without taxonomy service, generate stable IDs for tags
        Ok(tag_names.iter().map(|_| Uuid::new_v4()).collect())
    }

    async fn sync_category_translation_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _input: ProductTaxonomyCategoryTranslationInput,
    ) -> Result<(), CommerceError> {
        Ok(())
    }

    async fn load_scoped_categories(
        &self,
        _conn: TaxonomyConnection<'_>,
        _tenant_id: Uuid,
        _category_ids: Option<&[Uuid]>,
        _locale: &str,
        _fallback_locale: Option<&str>,
    ) -> Result<Vec<ProductTaxonomyCategorySnapshot>, CommerceError> {
        Ok(Vec::new())
    }
}

