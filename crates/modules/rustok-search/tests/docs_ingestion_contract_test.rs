/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::Database;
use uuid::Uuid;

use rustok_core::events::EventHandler;
use rustok_core::search_projection::{
    SearchProjectionDocument, SearchProjectionPage, SearchProjectionSource,
};
use rustok_events::DomainEvent;
use rustok_search::engine::{SearchResultItem, canonical_search_result_url};
use rustok_search::ingestion::SearchIngestionHandler;

struct MockDocsSource;

#[async_trait]
impl SearchProjectionSource for MockDocsSource {
    fn source_module(&self) -> &'static str {
        "docs"
    }

    async fn list_public_documents(
        &self,
        _tenant_id: Uuid,
        _after: Option<String>,
        _limit: usize,
    ) -> rustok_core::Result<SearchProjectionPage> {
        Ok(SearchProjectionPage::default())
    }

    async fn load_public_entity(
        &self,
        _tenant_id: Uuid,
        _entity_type: &str,
        _entity_id: Uuid,
    ) -> rustok_core::Result<Vec<SearchProjectionDocument>> {
        Ok(Vec::new())
    }
}

async fn handler_without_docs() -> SearchIngestionHandler {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should initialize routing-only handler");
    SearchIngestionHandler::new(db)
}

async fn handler_with_docs() -> SearchIngestionHandler {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should initialize routing-only handler");
    SearchIngestionHandler::with_all_projection_sources(
        db,
        None,
        None,
        None,
        Some(Arc::new(MockDocsSource)),
    )
}

#[tokio::test]
async fn docs_reindex_supports_targeted_and_full_scope_requests() {
    let handler = handler_with_docs().await;
    let doc_id = Uuid::new_v4();

    assert!(handler.handles(&DomainEvent::ReindexRequested {
        target_type: "docs".to_string(),
        target_id: Some(doc_id),
    }));
    assert!(handler.handles(&DomainEvent::ReindexRequested {
        target_type: "docs".to_string(),
        target_id: None,
    }));
    assert!(handler.handles(&DomainEvent::ReindexRequested {
        target_type: "doc".to_string(),
        target_id: Some(doc_id),
    }));
    assert!(handler.handles(&DomainEvent::ReindexRequested {
        target_type: "doc".to_string(),
        target_id: None,
    }));
    assert!(handler.handles(&DomainEvent::ReindexRequested {
        target_type: "doc_page".to_string(),
        target_id: Some(doc_id),
    }));
    assert!(handler.handles(&DomainEvent::ReindexRequested {
        target_type: "doc_page".to_string(),
        target_id: None,
    }));

    let unconfigured_handler = handler_without_docs().await;
    assert!(!unconfigured_handler.handles(&DomainEvent::ReindexRequested {
        target_type: "docs".to_string(),
        target_id: Some(doc_id),
    }));
}

#[tokio::test]
async fn docs_module_enable_and_disable_are_owned_lifecycle_events() {
    let handler = handler_with_docs().await;
    let tenant_id = Uuid::new_v4();

    assert!(handler.handles(&DomainEvent::TenantModuleToggled {
        tenant_id,
        module_slug: "docs".to_string(),
        enabled: false,
    }));
    assert!(handler.handles(&DomainEvent::TenantModuleToggled {
        tenant_id,
        module_slug: "docs".to_string(),
        enabled: true,
    }));

    let unconfigured_handler = handler_without_docs().await;
    assert!(!unconfigured_handler.handles(&DomainEvent::TenantModuleToggled {
        tenant_id,
        module_slug: "docs".to_string(),
        enabled: false,
    }));
}

#[test]
fn docs_canonical_search_url_is_storefront_doc_route() {
    let doc_id = Uuid::new_v4();
    let item = SearchResultItem {
        id: doc_id,
        entity_type: "doc_page".to_string(),
        source_module: "docs".to_string(),
        title: "CRDT Architecture Guide".to_string(),
        snippet: None,
        score: 1.0,
        locale: Some("en".to_string()),
        payload: serde_json::json!({
            "slug": "architecture/crdt",
            "title": "CRDT Architecture Guide",
        }),
    };

    let url = canonical_search_result_url(&item);
    assert_eq!(url.as_deref(), Some("/modules/docs?slug=architecture/crdt"));
}
