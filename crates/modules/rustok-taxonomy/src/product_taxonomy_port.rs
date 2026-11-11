/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Adapter implementing `rustok_product::ports::ProductTaxonomyPort` for the Product catalog.
//!
//! Provides term name resolution, tag registration, category translation synchronization,
//! and category snapshot reading without exposing private Taxonomy entities or table structures to Product.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_product::CommerceError;
use rustok_product::ports::{
    ProductTaxonomyCategorySnapshot, ProductTaxonomyCategoryTranslationInput, ProductTaxonomyPort,
    TaxonomyConnection,
};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use crate::error::TaxonomyError;
use crate::owner_category_read::TaxonomyOwnerCategoryReader;
use crate::owner_category_sync::{SyncModuleCategoryInput, sync_module_category_in_tx};
use crate::services::TaxonomyService;
use crate::{TaxonomyScopeType, TaxonomyTermKind};

/// Adapter providing ProductTaxonomyPort backed by the native Taxonomy database.
pub struct TaxonomyProductTaxonomyPortAdapter {
    _db: DatabaseConnection,
    service: TaxonomyService,
}

impl TaxonomyProductTaxonomyPortAdapter {
    pub fn new(db: DatabaseConnection) -> Self {
        let service = TaxonomyService::new(db.clone());
        Self { _db: db, service }
    }
}

fn map_taxonomy_error(err: TaxonomyError) -> CommerceError {
    match err {
        TaxonomyError::Database(db_err) => CommerceError::Database(db_err),
        TaxonomyError::Validation(msg) => CommerceError::Validation(msg),
        TaxonomyError::Forbidden(msg) => CommerceError::Validation(format!("Forbidden: {msg}")),
        TaxonomyError::TermNotFound(id) => {
            CommerceError::Validation(format!("Taxonomy term not found: {id}"))
        }
        TaxonomyError::DuplicateCanonicalKey(msg)
        | TaxonomyError::DuplicateSlug(msg)
        | TaxonomyError::DuplicateAlias(msg)
        | TaxonomyError::Conflict(msg) => CommerceError::Validation(format!("Conflict: {msg}")),
        TaxonomyError::TranslationRevisionExhausted { term_id, locale } => {
            CommerceError::Validation(format!(
                "Translation revision exhausted for term {term_id} locale {locale}"
            ))
        }
        TaxonomyError::Internal(msg) => CommerceError::Validation(msg),
    }
}

#[async_trait]
impl ProductTaxonomyPort for TaxonomyProductTaxonomyPortAdapter {
    async fn resolve_term_names(
        &self,
        conn: TaxonomyConnection<'_>,
        tenant_id: Uuid,
        term_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, String>, CommerceError> {
        match conn {
            TaxonomyConnection::Connection(c) => self
                .service
                .resolve_term_names_in(c, tenant_id, term_ids, locale, fallback_locale)
                .await
                .map_err(map_taxonomy_error),
            TaxonomyConnection::Transaction(tx) => self
                .service
                .resolve_term_names_in(tx, tenant_id, term_ids, locale, fallback_locale)
                .await
                .map_err(map_taxonomy_error),
        }
    }

    async fn ensure_product_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        tag_names: &[String],
    ) -> Result<Vec<Uuid>, CommerceError> {
        self.service
            .ensure_terms_for_module_in_tx(
                txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                "product",
                locale,
                tag_names,
            )
            .await
            .map_err(map_taxonomy_error)
    }

    async fn sync_category_translation_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: ProductTaxonomyCategoryTranslationInput,
    ) -> Result<(), CommerceError> {
        let sync_input = SyncModuleCategoryInput {
            category_id: input.category_id,
            module_scope: input.module_scope,
            canonical_key: input.canonical_key,
            locale: input.locale,
            name: input.name,
            slug: input.slug,
            aliases: input.aliases,
            description: input.description,
            parent_id: input.parent_id,
            position: input.position,
            icon_key: None,
            color: None,
        };

        sync_module_category_in_tx(txn, tenant_id, sync_input)
            .await
            .map_err(map_taxonomy_error)?;
        Ok(())
    }

    async fn load_scoped_categories(
        &self,
        conn: TaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: Option<&[Uuid]>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<Vec<ProductTaxonomyCategorySnapshot>, CommerceError> {
        let categories = match conn {
            TaxonomyConnection::Connection(c) => {
                TaxonomyOwnerCategoryReader::load_scoped_categories_in(
                    c,
                    tenant_id,
                    TaxonomyScopeType::Module,
                    Some("product"),
                    category_ids,
                    locale,
                    fallback_locale,
                )
                .await
            }
            TaxonomyConnection::Transaction(tx) => {
                TaxonomyOwnerCategoryReader::load_scoped_categories_in(
                    tx,
                    tenant_id,
                    TaxonomyScopeType::Module,
                    Some("product"),
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
            .map(|c| ProductTaxonomyCategorySnapshot {
                id: c.id,
                parent_id: c.parent_id,
                canonical_key: c.canonical_key,
                scope_type: c.scope_type.to_string(),
                scope_value: c.scope_value,
                slug: c.slug,
                name: c.name,
                description: c.description,
                position: c.position,
                available_locales: c.available_locales,
            })
            .collect())
    }
}

/// Constructs an in-process `ProductTaxonomyPort` adapter backed by the native Taxonomy module.
pub fn in_process_product_taxonomy_port(
    db: DatabaseConnection,
) -> Arc<dyn ProductTaxonomyPort> {
    Arc::new(TaxonomyProductTaxonomyPortAdapter::new(db))
}
