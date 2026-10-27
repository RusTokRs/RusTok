use rustok_core::events::EventHandler;
use rustok_events::DomainEvent;
use rustok_search::{
    SearchIngestionHandler, SearchProjectionDocument, SearchProjectionPage, SearchProjectionSource,
    SearchResultItem, canonical_search_result_url,
};
use sea_orm::Database;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug)]
struct MockPagesSource;

#[async_trait::async_trait]
impl SearchProjectionSource for MockPagesSource {
    fn source_module(&self) -> &'static str {
        "pages"
    }

    async fn list_public_documents(
        &self,
        _tenant_id: Uuid,
        _after: Option<String>,
        _limit: usize,
    ) -> Result<SearchProjectionPage, rustok_core::Error> {
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
    ) -> Result<Vec<SearchProjectionDocument>, rustok_core::Error> {
        Ok(Vec::new())
    }
}

async fn handler_without_pages() -> SearchIngestionHandler {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should initialize routing-only handler");
    SearchIngestionHandler::new(db)
}

async fn handler_with_pages() -> SearchIngestionHandler {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should initialize routing-only handler");
    SearchIngestionHandler::with_all_projection_sources(
        db,
        None,
        None,
        Some(Arc::new(MockPagesSource)),
        None,
    )
}

#[tokio::test]
async fn every_page_lifecycle_event_is_owned_by_search_ingestion() {
    let handler = handler_with_pages().await;
    let node_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let events = [
        DomainEvent::NodeCreated {
            node_id,
            kind: "page".to_string(),
            author_id: Some(author_id),
        },
        DomainEvent::NodeUpdated {
            node_id,
            kind: "page".to_string(),
        },
        DomainEvent::NodePublished {
            node_id,
            kind: "page".to_string(),
        },
        DomainEvent::NodeUnpublished {
            node_id,
            kind: "page".to_string(),
        },
        DomainEvent::NodeDeleted {
            node_id,
            kind: "page".to_string(),
        },
    ];

    for event in events {
        assert!(
            handler.handles(&event),
            "search ingestion must own page event {}",
            event.event_type()
        );
    }
}

#[tokio::test]
async fn pages_reindex_supports_targeted_and_full_scope_requests() {
    let handler = handler_with_pages().await;
    let node_id = Uuid::new_v4();

    assert!(handler.handles(&DomainEvent::ReindexRequested {
        target_type: "pages".to_string(),
        target_id: Some(node_id),
    }));
    assert!(handler.handles(&DomainEvent::ReindexRequested {
        target_type: "pages".to_string(),
        target_id: None,
    }));
    assert!(handler.handles(&DomainEvent::ReindexRequested {
        target_type: "page".to_string(),
        target_id: Some(node_id),
    }));
    assert!(handler.handles(&DomainEvent::ReindexRequested {
        target_type: "page".to_string(),
        target_id: None,
    }));

    let unconfigured_handler = handler_without_pages().await;
    assert!(
        !unconfigured_handler.handles(&DomainEvent::ReindexRequested {
            target_type: "pages".to_string(),
            target_id: Some(node_id),
        })
    );
}

#[tokio::test]
async fn pages_module_enable_and_disable_are_owned_lifecycle_events() {
    let handler = handler_with_pages().await;
    let tenant_id = Uuid::new_v4();

    assert!(handler.handles(&DomainEvent::TenantModuleToggled {
        tenant_id,
        module_slug: "pages".to_string(),
        enabled: false,
    }));
    assert!(handler.handles(&DomainEvent::TenantModuleToggled {
        tenant_id,
        module_slug: "pages".to_string(),
        enabled: true,
    }));

    let unconfigured_handler = handler_without_pages().await;
    assert!(
        !unconfigured_handler.handles(&DomainEvent::TenantModuleToggled {
            tenant_id,
            module_slug: "pages".to_string(),
            enabled: false,
        })
    );
}

#[test]
fn pages_canonical_search_url_is_storefront_page_route() {
    let page_id = Uuid::new_v4();
    let item = SearchResultItem {
        id: page_id,
        entity_type: "page".to_string(),
        source_module: "pages".to_string(),
        title: "Special Landing Offer".to_string(),
        snippet: None,
        score: 1.0,
        locale: Some("en".to_string()),
        payload: serde_json::json!({
            "slug": "landing-offer-2026",
            "title": "Special Landing Offer",
        }),
    };

    let url = canonical_search_result_url(&item);
    assert_eq!(
        url.as_deref(),
        Some("/modules/pages?slug=landing-offer-2026")
    );
}
