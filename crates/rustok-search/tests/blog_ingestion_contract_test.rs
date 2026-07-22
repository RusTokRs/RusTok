use rustok_core::events::EventHandler;
use rustok_events::DomainEvent;
use rustok_search::SearchIngestionHandler;
use sea_orm::Database;
use uuid::Uuid;

async fn handler() -> SearchIngestionHandler {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should initialize routing-only handler");
    SearchIngestionHandler::new(db)
}

#[tokio::test]
async fn every_blog_post_lifecycle_event_is_owned_by_search_ingestion() {
    let handler = handler().await;
    let post_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let events = [
        DomainEvent::BlogPostCreated {
            post_id,
            author_id: Some(author_id),
            locale: "en".to_string(),
        },
        DomainEvent::BlogPostPublished {
            post_id,
            author_id: Some(author_id),
        },
        DomainEvent::BlogPostUnpublished { post_id },
        DomainEvent::BlogPostUpdated {
            post_id,
            locale: "ru".to_string(),
        },
        DomainEvent::BlogPostArchived {
            post_id,
            reason: Some("superseded".to_string()),
        },
        DomainEvent::BlogPostDeleted { post_id },
    ];

    for event in events {
        assert!(
            handler.handles(&event),
            "search ingestion must own {}",
            event.event_type()
        );
    }
}

#[tokio::test]
async fn blog_reindex_supports_targeted_and_full_scope_requests() {
    let handler = handler().await;
    let post_id = Uuid::new_v4();

    assert!(handler.handles(&DomainEvent::ReindexRequested {
        target_type: "blog".to_string(),
        target_id: Some(post_id),
    }));
    assert!(handler.handles(&DomainEvent::ReindexRequested {
        target_type: "blog".to_string(),
        target_id: None,
    }));
}

#[tokio::test]
async fn blog_module_enable_and_disable_are_owned_lifecycle_events() {
    let handler = handler().await;
    let tenant_id = Uuid::new_v4();

    assert!(handler.handles(&DomainEvent::TenantModuleToggled {
        tenant_id,
        module_slug: "blog".to_string(),
        enabled: false,
    }));
    assert!(handler.handles(&DomainEvent::TenantModuleToggled {
        tenant_id,
        module_slug: "blog".to_string(),
        enabled: true,
    }));
    assert!(!handler.handles(&DomainEvent::TenantModuleToggled {
        tenant_id,
        module_slug: "forum".to_string(),
        enabled: false,
    }));
}

#[tokio::test]
async fn unrelated_reindex_target_is_not_claimed_by_search_ingestion() {
    let handler = handler().await;

    assert!(!handler.handles(&DomainEvent::ReindexRequested {
        target_type: "forum".to_string(),
        target_id: Some(Uuid::new_v4()),
    }));
}
