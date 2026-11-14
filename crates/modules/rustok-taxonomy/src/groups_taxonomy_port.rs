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
//! Provides category existence validation, category metadata resolution, term name lookup,
//! and group tag creation without exposing private Taxonomy entities or table structures to Groups.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_groups::error::GroupsError;
use rustok_groups::ports::{
    GroupTaxonomyCategorySnapshot, GroupTaxonomyConnection, GroupTaxonomyPort,
};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use crate::error::TaxonomyError;
use crate::owner_category_read::TaxonomyOwnerCategoryReader;
use crate::services::TaxonomyService;
use crate::{TaxonomyScopeType, TaxonomyTermKind};

/// Adapter providing GroupTaxonomyPort backed by the native Taxonomy database.
pub struct TaxonomyGroupTaxonomyPortAdapter {
    db: DatabaseConnection,
    service: TaxonomyService,
}

impl TaxonomyGroupTaxonomyPortAdapter {
    pub fn new(db: DatabaseConnection) -> Self {
        let service = TaxonomyService::new(db.clone());
        Self { db, service }
    }
}

fn map_taxonomy_error(err: TaxonomyError) -> GroupsError {
    match err {
        TaxonomyError::Database(db_err) => GroupsError::Persistence(db_err.to_string()),
        TaxonomyError::Validation(msg) => GroupsError::Validation(msg),
        TaxonomyError::Forbidden(msg) => GroupsError::Forbidden(msg),
        TaxonomyError::TermNotFound(_) => GroupsError::NotFound,
        TaxonomyError::DuplicateCanonicalKey(msg)
        | TaxonomyError::DuplicateSlug(msg)
        | TaxonomyError::DuplicateAlias(msg)
        | TaxonomyError::Conflict(msg) => GroupsError::Conflict(msg),
        TaxonomyError::TranslationRevisionExhausted { term_id, locale } => GroupsError::Conflict(
            format!("Translation revision exhausted for term {term_id} locale {locale}"),
        ),
        TaxonomyError::Internal(msg) => GroupsError::Invariant(msg),
    }
}

#[async_trait]
impl GroupTaxonomyPort for TaxonomyGroupTaxonomyPortAdapter {
    async fn validate_category_exists(
        &self,
        conn: GroupTaxonomyConnection<'_>,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> Result<bool, GroupsError> {
        let categories = match conn {
            GroupTaxonomyConnection::Connection(c) => {
                TaxonomyOwnerCategoryReader::load_scoped_categories_in(
                    c,
                    tenant_id,
                    TaxonomyScopeType::Module,
                    Some("groups"),
                    Some(&[category_id]),
                    rustok_api::PLATFORM_FALLBACK_LOCALE,
                    Some(rustok_api::PLATFORM_FALLBACK_LOCALE),
                )
                .await
            }
            GroupTaxonomyConnection::Transaction(tx) => {
                TaxonomyOwnerCategoryReader::load_scoped_categories_in(
                    tx,
                    tenant_id,
                    TaxonomyScopeType::Module,
                    Some("groups"),
                    Some(&[category_id]),
                    rustok_api::PLATFORM_FALLBACK_LOCALE,
                    Some(rustok_api::PLATFORM_FALLBACK_LOCALE),
                )
                .await
            }
            GroupTaxonomyConnection::None => {
                TaxonomyOwnerCategoryReader::load_scoped_categories_in(
                    &self.db,
                    tenant_id,
                    TaxonomyScopeType::Module,
                    Some("groups"),
                    Some(&[category_id]),
                    rustok_api::PLATFORM_FALLBACK_LOCALE,
                    Some(rustok_api::PLATFORM_FALLBACK_LOCALE),
                )
                .await
            }
        }
        .map_err(map_taxonomy_error)?;

        Ok(!categories.is_empty())
    }

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
pub fn in_process_group_taxonomy_port(db: DatabaseConnection) -> Arc<dyn GroupTaxonomyPort> {
    Arc::new(TaxonomyGroupTaxonomyPortAdapter::new(db))
}
