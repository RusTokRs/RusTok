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

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_product::CommerceError;
use rustok_product::ports::{
    ProductTaxonomyCategorySnapshot, ProductTaxonomyCategoryTranslationInput, ProductTaxonomyPort,
    TaxonomyConnection,
};
use rustok_taxonomy::{
    SyncModuleCategoryInput, TaxonomyError, TaxonomyOwnerCategoryReader, TaxonomyScopeType,
    TaxonomyService, TaxonomyTermKind, sync_module_category_copy_in_tx,
};
use rustok_taxonomy_ports::{ReadTaxonomyTermNamesRequest, TaxonomyTermNamesReadPort};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

/// Adapter providing ProductTaxonomyPort backed by the native Taxonomy database.
struct TaxonomyProductTaxonomyPortAdapter {
    service: TaxonomyService,
}

/// Sync accepts user intent; its authoritative domain refusals retain their class.
fn map_taxonomy_sync_error(error: TaxonomyError) -> CommerceError {
    use rustok_core::error::Error as CoreError;
    match error {
        TaxonomyError::Database(error) => CommerceError::Database(error),
        TaxonomyError::Internal(_) => CommerceError::Core(CoreError::Internal(
            "Taxonomy category synchronization failed".into(),
        )),
        TaxonomyError::Forbidden(_) => CommerceError::Core(CoreError::Forbidden(
            "Taxonomy category access is denied".into(),
        )),
        TaxonomyError::TermNotFound(_) => CommerceError::Core(CoreError::NotFound(
            "Referenced Taxonomy category was not found".into(),
        )),
        TaxonomyError::DuplicateCanonicalKey(_)
        | TaxonomyError::DuplicateSlug(_)
        | TaxonomyError::DuplicateAlias(_)
        | TaxonomyError::Conflict(_)
        | TaxonomyError::TranslationRevisionExhausted { .. } => {
            CommerceError::Conflict("Taxonomy category conflicts with current state".into())
        }
        TaxonomyError::Validation(_) => {
            CommerceError::Validation("Taxonomy category request is invalid".into())
        }
    }
}

/// A read projection does not accept a Taxonomy mutation: domain-shaped read failures
/// other than permission denial indicate broken owner/projection state, not invalid user input.
fn map_taxonomy_read_error(error: TaxonomyError) -> CommerceError {
    use rustok_core::error::Error as CoreError;
    match error {
        TaxonomyError::Database(error) => CommerceError::Database(error),
        TaxonomyError::Forbidden(_) => CommerceError::Core(CoreError::Forbidden(
            "Taxonomy category access is denied".into(),
        )),
        TaxonomyError::Internal(_)
        | TaxonomyError::TermNotFound(_)
        | TaxonomyError::DuplicateCanonicalKey(_)
        | TaxonomyError::DuplicateSlug(_)
        | TaxonomyError::DuplicateAlias(_)
        | TaxonomyError::Conflict(_)
        | TaxonomyError::TranslationRevisionExhausted { .. }
        | TaxonomyError::Validation(_) => {
            taxonomy_projection_error("Taxonomy category projection could not be read safely")
        }
    }
}

fn taxonomy_projection_error(message: &'static str) -> CommerceError {
    CommerceError::Core(rustok_core::error::Error::Internal(message.into()))
}

#[async_trait]
impl TaxonomyTermNamesReadPort for TaxonomyProductTaxonomyPortAdapter {
    async fn read_term_names(
        &self,
        context: PortContext,
        request: ReadTaxonomyTermNamesRequest,
    ) -> Result<BTreeMap<Uuid, String>, PortError> {
        self.service.read_term_names(context, request).await
    }
}

#[async_trait]
impl ProductTaxonomyPort for TaxonomyProductTaxonomyPortAdapter {
    async fn resolve_term_names_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        term_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, String>, CommerceError> {
        self.service
            .resolve_term_names_in(txn, tenant_id, term_ids, locale, fallback_locale)
            .await
            .map_err(map_taxonomy_read_error)
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
            .map_err(map_taxonomy_sync_error)
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

        sync_module_category_copy_in_tx(txn, tenant_id, sync_input)
            .await
            .map_err(map_taxonomy_sync_error)?;
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
        .map_err(map_taxonomy_read_error)?;

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
pub fn in_process_product_taxonomy_port(db: DatabaseConnection) -> Arc<dyn ProductTaxonomyPort> {
    Arc::new(TaxonomyProductTaxonomyPortAdapter {
        service: TaxonomyService::new(db),
    })
}
