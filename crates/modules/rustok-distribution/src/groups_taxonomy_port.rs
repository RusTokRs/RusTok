/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Adapter implementing `rustok_groups::ports::GroupTaxonomyPort` for Social Groups.
//!
//! Provides category metadata resolution, term name lookup,
//! and group tag creation without exposing private Taxonomy entities or table structures to Groups.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_groups::error::GroupsError;
use rustok_groups::ports::{
    GroupTaxonomyCategorySnapshot, GroupTaxonomyConnection, GroupTaxonomyPort,
    SharedGroupTaxonomyPort,
};
use rustok_taxonomy::{
    TaxonomyError, TaxonomyOwnerCategoryReader, TaxonomyScopeType, TaxonomyService,
    TaxonomyTermKind,
};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

/// Adapter providing GroupTaxonomyPort backed by the native Taxonomy database.
struct TaxonomyGroupTaxonomyPortAdapter {
    db: DatabaseConnection,
    service: TaxonomyService,
}

impl TaxonomyGroupTaxonomyPortAdapter {
    fn new(db: DatabaseConnection) -> Self {
        let service = TaxonomyService::new(db.clone());
        Self { db, service }
    }
}

fn map_taxonomy_error(err: TaxonomyError) -> GroupsError {
    match err {
        TaxonomyError::Database(error) => {
            tracing::warn!(%error, "Groups taxonomy persistence failed");
            GroupsError::Persistence("Groups taxonomy provider is unavailable".into())
        }
        TaxonomyError::Validation(_) => GroupsError::Validation("Invalid taxonomy request".into()),
        TaxonomyError::Forbidden(_) => GroupsError::Forbidden("Taxonomy access denied".into()),
        TaxonomyError::TermNotFound(_) => GroupsError::NotFound,
        TaxonomyError::DuplicateCanonicalKey(_)
        | TaxonomyError::DuplicateSlug(_)
        | TaxonomyError::DuplicateAlias(_)
        | TaxonomyError::Conflict(_)
        | TaxonomyError::TranslationRevisionExhausted { .. } => {
            GroupsError::Conflict("Taxonomy request conflicts with current state".into())
        }
        TaxonomyError::Internal(_) => GroupsError::Invariant("Taxonomy operation failed".into()),
    }
}

#[async_trait]
impl GroupTaxonomyPort for TaxonomyGroupTaxonomyPortAdapter {
    async fn load_categories(
        &self,
        conn: GroupTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, GroupTaxonomyCategorySnapshot>, GroupsError> {
        let categories = match conn {
            GroupTaxonomyConnection::Connection(c) => {
                TaxonomyOwnerCategoryReader::load_scoped_categories_in(
                    c,
                    tenant_id,
                    TaxonomyScopeType::Module,
                    Some("groups"),
                    Some(category_ids),
                    locale,
                    fallback_locale,
                )
                .await
            }
            GroupTaxonomyConnection::Transaction(tx) => {
                TaxonomyOwnerCategoryReader::load_scoped_categories_in(
                    tx,
                    tenant_id,
                    TaxonomyScopeType::Module,
                    Some("groups"),
                    Some(category_ids),
                    locale,
                    fallback_locale,
                )
                .await
            }
            GroupTaxonomyConnection::None => {
                TaxonomyOwnerCategoryReader::load_scoped_categories_in(
                    &self.db,
                    tenant_id,
                    TaxonomyScopeType::Module,
                    Some("groups"),
                    Some(category_ids),
                    locale,
                    fallback_locale,
                )
                .await
            }
        }
        .map_err(map_taxonomy_error)?;

        Ok(categories
            .into_iter()
            .map(|c| {
                (
                    c.id,
                    GroupTaxonomyCategorySnapshot {
                        id: c.id,
                        parent_id: c.parent_id,
                        canonical_key: c.canonical_key,
                        slug: c.slug,
                        name: c.name,
                        description: c.description,
                        position: c.position,
                        icon_key: c.icon_key,
                        color: c.color,
                        available_locales: c.available_locales,
                        requested_locale: c.requested_locale,
                        effective_locale: c.effective_locale,
                    },
                )
            })
            .collect())
    }

    async fn resolve_term_names(
        &self,
        conn: GroupTaxonomyConnection<'_>,
        tenant_id: Uuid,
        term_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Result<HashMap<Uuid, String>, GroupsError> {
        match conn {
            GroupTaxonomyConnection::Connection(c) => self
                .service
                .resolve_term_names_in(c, tenant_id, term_ids, locale, fallback_locale)
                .await
                .map_err(map_taxonomy_error),
            GroupTaxonomyConnection::Transaction(tx) => self
                .service
                .resolve_term_names_in(tx, tenant_id, term_ids, locale, fallback_locale)
                .await
                .map_err(map_taxonomy_error),
            GroupTaxonomyConnection::None => self
                .service
                .resolve_term_names_in(&self.db, tenant_id, term_ids, locale, fallback_locale)
                .await
                .map_err(map_taxonomy_error),
        }
    }

    async fn ensure_group_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        tags: &[String],
    ) -> Result<Vec<Uuid>, GroupsError> {
        self.service
            .ensure_terms_for_module_in_tx(
                txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                "groups",
                locale,
                tags,
            )
            .await
            .map_err(map_taxonomy_error)
    }
}

/// Constructs an in-process `GroupTaxonomyPort` adapter backed by the native Taxonomy module.
pub(crate) fn in_process_group_taxonomy_port(db: DatabaseConnection) -> SharedGroupTaxonomyPort {
    Arc::new(TaxonomyGroupTaxonomyPortAdapter::new(db))
}
