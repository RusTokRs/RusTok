/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical integration seams and capability bridges for finance.
//!
//! This module provides uniform conventions for interacting with cross-cutting
//! platform capabilities: Caching, Search indexing, Media attachments, and Translation.

pub mod cache {
    //! Cache key helpers and invalidation conventions.
    //!
    //! Standard Redis / in-memory cache key format:
    //! `tenant:{tenant_id}:finance:{entity_name}:{entity_id}`

    use uuid::Uuid;

    /// Formats a canonical tenant-scoped cache key for a record.
    pub fn record_cache_key(tenant_id: Uuid, id: Uuid) -> String {
        format!("tenant:{tenant_id}:finance:record:{id}")
    }

    /// Formats a tenant-scoped cache prefix for bulk invalidation sweeps.
    pub fn tenant_cache_prefix(tenant_id: Uuid) -> String {
        format!("tenant:{tenant_id}:finance:")
    }
}

pub mod search {
    //! Search & discovery projection seam.
    //!
    //! All discoverable domain entities emit search projection documents into `rustok-search`.

    use std::sync::Arc;
    use async_trait::async_trait;
    use rustok_core::search_projection::{
        SearchProjectionDocument, SearchProjectionPage,
        SearchProjectionSource, SearchProjectionSourceFactory,
    };
    use rustok_core::{ModuleRuntimeExtensions, Result};
    use sea_orm::DatabaseConnection;
    use uuid::Uuid;

    pub const FINANCE_SOURCE_MODULE: &str = "finance";

    #[derive(Clone, Default)]
    pub struct FinanceSearchProjectionSourceFactory;

    impl SearchProjectionSourceFactory for FinanceSearchProjectionSourceFactory {
        fn source_module(&self) -> &'static str {
            FINANCE_SOURCE_MODULE
        }

        fn build(&self, db: DatabaseConnection) -> Arc<dyn SearchProjectionSource> {
            Arc::new(FinanceSearchProjectionSource { _db: db })
        }

        fn build_with_extensions(
            &self,
            db: DatabaseConnection,
            _extensions: &ModuleRuntimeExtensions,
        ) -> Arc<dyn SearchProjectionSource> {
            Arc::new(FinanceSearchProjectionSource { _db: db })
        }
    }

    #[derive(Clone)]
    pub struct FinanceSearchProjectionSource {
        _db: DatabaseConnection,
    }

    #[async_trait]
    impl SearchProjectionSource for FinanceSearchProjectionSource {
        fn source_module(&self) -> &'static str {
            FINANCE_SOURCE_MODULE
        }

        async fn list_public_documents(
            &self,
            _tenant_id: Uuid,
            _after: Option<String>,
            _limit: usize,
        ) -> Result<SearchProjectionPage> {
            // Domain query mapping for search indexation
            Ok(SearchProjectionPage {
                documents: Vec::new(),
                next_cursor: None,
            })
        }

        async fn load_public_entity(
            &self,
            _tenant_id: Uuid,
            _entity_type: &str,
            _entity_id: Uuid,
        ) -> Result<Vec<SearchProjectionDocument>> {
            Ok(Vec::new())
        }
    }
}

pub mod media {
    //! Media asset attachment conventions.
    //!
    //! Assets (images, documents, video) are sovereignly owned by `rustok-media`.
    //! When finance entities associate with media:
    //! - Store referenced media asset IDs as `Option<uuid::Uuid>` or `Vec<uuid::Uuid>`.
    //! - Invariant: Verify tenant ownership of the media asset before linking.
    //! - Side-effect: When deleting an entity, emit a deletion event so media references can be reclaimed.
}

pub mod translation {
    //! Multi-language translation target seam.
    //!
    //! When finance domain entities contain localized strings (e.g. title, description, content),
    //! implement `TranslationTargetProvider` from `rustok_translation_targets` and register
    //! the provider inside `FinanceModule::register_runtime_extensions`.
    //!
    //! See `crates/modules/rustok-taxonomy/src/translation_target.rs` as canonical reference.
}
