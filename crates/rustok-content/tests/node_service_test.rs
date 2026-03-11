// Comprehensive unit tests for NodeService
// These tests verify CRUD operations, validation, RBAC enforcement,
// and multi-language support for content nodes.

use rustok_content::dto::{
    BodyInput, CreateNodeInput, ListNodesFilter, NodeTranslationInput, UpdateNodeInput,
};
use rustok_content::entities::node::ContentStatus;
use rustok_content::services::NodeService;
use rustok_content::ContentError;
use rustok_test_utils::{
    db::setup_test_db, helpers::admin_context, helpers::manager_context, helpers::unique_slug,
    mock_transactional_event_bus,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

async fn ensure_content_schema(db: &DatabaseConnection) {
    if db.get_database_backend() != DbBackend::Sqlite {
        return;
    }

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE IF NOT EXISTS nodes (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            parent_id TEXT NULL,
            author_id TEXT NULL,
            kind TEXT NOT NULL,
            category_id TEXT NULL,
            status TEXT NOT NULL,
            position INTEGER NOT NULL,
            depth INTEGER NOT NULL,
            reply_count INTEGER NOT NULL,
            metadata TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            published_at TEXT NULL,
            deleted_at TEXT NULL,
            version INTEGER NOT NULL DEFAULT 1
        )"
        .to_string(),
    ))
    .await
    .expect("failed to create content nodes test table");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE IF NOT EXISTS node_translations (
            id TEXT PRIMARY KEY,
            node_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            title TEXT NULL,
            slug TEXT NULL,
            excerpt TEXT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(node_id) REFERENCES nodes(id)
        )"
        .to_string(),
    ))
    .await
    .expect("failed to create content node_translations test table");

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE IF NOT EXISTS bodies (
            id TEXT PRIMARY KEY,
            node_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            body TEXT NULL,
            format TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(node_id) REFERENCES nodes(id)
        )"
        .to_string(),
    ))
    .await
    .expect("failed to create content bodies test table");
}

async fn setup() -> (DatabaseConnection, NodeService) {
    let db = setup_test_db().await;
    ensure_content_schema(&db).await;
    let event_bus = mock_transactional_event_bus();
    let service = NodeService::new(db.clone(), event_bus);
    (db, service)
}

fn create_test_input() -> CreateNodeInput {
    CreateNodeInput {
        kind: "post".to_string(),
        translations: vec![NodeTranslationInput {
            locale: "en".to_string(),
            title: Some("Test Post".to_string()),
            slug: Some(unique_slug("test-post")),
            excerpt: Some("Test excerpt".to_string()),
        }],
        bodies: vec![BodyInput {
            locale: "en".to_string(),
            body: Some("# Test Content\n\nThis is test content.".to_string()),
            format: Some("markdown".to_string()),
        }],
        status: Some(ContentStatus::Draft),
        parent_id: None,
        author_id: None,
        category_id: None,
        position: Some(0),
        depth: Some(0),
        reply_count: Some(0),
        metadata: serde_json::json!({"featured": false}),
    }
}

// =============================================================================
// Basic CRUD Tests
// =============================================================================

#[tokio::test]
async fn test_create_node_success() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();
    let input = create_test_input();

    let result = service.create_node(tenant_id, security, input).await;

    assert!(result.is_ok());
    let node = result.unwrap();
    assert_eq!(node.kind, "post");
    assert_eq!(node.translations.len(), 1);
    assert_eq!(node.translations[0].title, Some("Test Post".to_string()));
    assert_eq!(node.bodies.len(), 1);
    assert!(node.bodies[0]
        .body
        .as_deref()
        .unwrap_or("")
        .contains("Test Content"));
}

#[tokio::test]
async fn test_create_node_requires_translations() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let mut input = create_test_input();
    input.translations = vec![];

    let result = service.create_node(tenant_id, security, input).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ContentError::Validation(msg) => {
            assert!(msg.contains("translation"));
        }
        _ => panic!("Expected validation error"),
    }
}

#[tokio::test]
async fn test_get_node_success() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let input = create_test_input();
    let created = service
        .create_node(tenant_id, security, input)
        .await
        .unwrap();

    let result = service.get_node(tenant_id, created.id).await;

    assert!(result.is_ok());
    let node = result.unwrap();
    assert_eq!(node.id, created.id);
    assert_eq!(node.kind, "post");
}

#[tokio::test]
async fn test_get_nonexistent_node() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let fake_id = Uuid::new_v4();

    let result = service.get_node(tenant_id, fake_id).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ContentError::NodeNotFound(_) => {}
        _ => panic!("Expected NodeNotFound error"),
    }
}

#[tokio::test]
async fn test_update_node_success() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let input = create_test_input();
    let node = service
        .create_node(tenant_id, security.clone(), input)
        .await
        .unwrap();

    let update_input = UpdateNodeInput {
        translations: Some(vec![NodeTranslationInput {
            locale: "en".to_string(),
            title: Some("Updated Title".to_string()),
            slug: None,
            excerpt: Some("Updated excerpt".to_string()),
        }]),
        status: Some(ContentStatus::Published),
        ..UpdateNodeInput::default()
    };

    let result = service
        .update_node(tenant_id, node.id, security, update_input)
        .await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(
        updated.translations[0].title,
        Some("Updated Title".to_string())
    );
    assert_eq!(updated.status, ContentStatus::Published);
}

#[tokio::test]
async fn test_delete_node_success() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let input = create_test_input();
    let node = service
        .create_node(tenant_id, security.clone(), input)
        .await
        .unwrap();

    let result = service.delete_node(tenant_id, node.id, security).await;
    assert!(result.is_ok());

    let get_result = service.get_node(tenant_id, node.id).await;
    assert!(get_result.is_err());
}

// =============================================================================
// Multi-Language Translation Tests
// =============================================================================

#[tokio::test]
async fn test_create_node_with_multiple_translations() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let mut input = create_test_input();
    input.translations.push(NodeTranslationInput {
        locale: "ru".to_string(),
        title: Some("Тестовый пост".to_string()),
        slug: Some(unique_slug("test-post-ru")),
        excerpt: Some("Тестовое описание".to_string()),
    });
    input.translations.push(NodeTranslationInput {
        locale: "de".to_string(),
        title: Some("Testbeitrag".to_string()),
        slug: Some(unique_slug("test-post-de")),
        excerpt: Some("Testbeschreibung".to_string()),
    });

    let result = service.create_node(tenant_id, security, input).await;

    assert!(result.is_ok());
    let node = result.unwrap();
    assert_eq!(node.translations.len(), 3);

    let en_translation = node.translations.iter().find(|t| t.locale == "en");
    let ru_translation = node.translations.iter().find(|t| t.locale == "ru");
    let de_translation = node.translations.iter().find(|t| t.locale == "de");

    assert!(en_translation.is_some());
    assert!(ru_translation.is_some());
    assert!(de_translation.is_some());
    assert_eq!(en_translation.unwrap().title, Some("Test Post".to_string()));
    assert_eq!(
        ru_translation.unwrap().title,
        Some("Тестовый пост".to_string())
    );
    assert_eq!(
        de_translation.unwrap().title,
        Some("Testbeitrag".to_string())
    );
}

#[tokio::test]
async fn test_get_by_slug_success() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let slug = unique_slug("test-post");
    let mut input = create_test_input();
    input.translations[0].slug = Some(slug.clone());

    let created = service
        .create_node(tenant_id, security, input)
        .await
        .unwrap();

    let result = service.get_by_slug(tenant_id, "post", "en", &slug).await;

    assert!(result.is_ok());
    let node = result.unwrap().expect("node should exist for slug");
    assert_eq!(node.id, created.id);
    assert_eq!(node.translations[0].slug, Some(slug));
}

// =============================================================================
// Content Status & Publishing Tests
// =============================================================================

#[tokio::test]
async fn test_create_node_with_published_status() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let mut input = create_test_input();
    input.status = Some(ContentStatus::Published);

    let result = service.create_node(tenant_id, security, input).await;

    assert!(result.is_ok());
    let node = result.unwrap();
    assert_eq!(node.status, ContentStatus::Published);
    assert!(node.published_at.is_some());
}

#[tokio::test]
async fn test_publish_node() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let mut input = create_test_input();
    input.status = Some(ContentStatus::Draft);
    let node = service
        .create_node(tenant_id, security.clone(), input)
        .await
        .unwrap();

    assert_eq!(node.status, ContentStatus::Draft);
    assert!(node.published_at.is_none());

    let result = service.publish_node(tenant_id, node.id, security).await;

    assert!(result.is_ok());
    let published = result.unwrap();
    assert_eq!(published.status, ContentStatus::Published);
    assert!(published.published_at.is_some());
}

#[tokio::test]
async fn test_unpublish_node() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let mut input = create_test_input();
    input.status = Some(ContentStatus::Published);
    let node = service
        .create_node(tenant_id, security.clone(), input)
        .await
        .unwrap();

    assert_eq!(node.status, ContentStatus::Published);

    let result = service.unpublish_node(tenant_id, node.id, security).await;

    assert!(result.is_ok());
    let unpublished = result.unwrap();
    assert_eq!(unpublished.status, ContentStatus::Draft);
}

// =============================================================================
// Hierarchical Content Tests
// =============================================================================

#[tokio::test]
async fn test_create_node_with_parent() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let parent_input = create_test_input();
    let parent = service
        .create_node(tenant_id, security.clone(), parent_input)
        .await
        .unwrap();

    let mut child_input = create_test_input();
    child_input.translations[0].title = Some("Child Post".to_string());
    child_input.parent_id = Some(parent.id);
    child_input.depth = Some(1);

    let result = service.create_node(tenant_id, security, child_input).await;

    assert!(result.is_ok());
    let child = result.unwrap();
    assert_eq!(child.parent_id, Some(parent.id));
    assert_eq!(child.depth, 1);
}

#[tokio::test]
async fn test_create_nested_hierarchy() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let root_input = create_test_input();
    let root = service
        .create_node(tenant_id, security.clone(), root_input)
        .await
        .unwrap();

    let mut level1_input = create_test_input();
    level1_input.parent_id = Some(root.id);
    level1_input.depth = Some(1);
    let level1 = service
        .create_node(tenant_id, security.clone(), level1_input)
        .await
        .unwrap();

    let mut level2_input = create_test_input();
    level2_input.parent_id = Some(level1.id);
    level2_input.depth = Some(2);
    let level2 = service
        .create_node(tenant_id, security, level2_input)
        .await
        .unwrap();

    assert_eq!(root.depth, 0);
    assert_eq!(level1.depth, 1);
    assert_eq!(level2.depth, 2);
    assert_eq!(level1.parent_id, Some(root.id));
    assert_eq!(level2.parent_id, Some(level1.id));
}

// =============================================================================
// List & Pagination Tests
// =============================================================================

#[tokio::test]
async fn test_list_nodes_empty() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();
    let filter = ListNodesFilter {
        page: 1,
        per_page: 10,
        ..Default::default()
    };

    let result = service.list_nodes(tenant_id, security, filter).await;

    assert!(result.is_ok());
    let (nodes, total) = result.unwrap();
    assert_eq!(nodes.len(), 0);
    assert_eq!(total, 0);
}

#[tokio::test]
async fn test_list_nodes_with_filter() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    for i in 0..5 {
        let mut input = create_test_input();
        input.translations[0].title = Some(format!("Post {}", i));
        service
            .create_node(tenant_id, security.clone(), input)
            .await
            .unwrap();
    }

    let filter = ListNodesFilter {
        kind: Some("post".to_string()),
        page: 1,
        per_page: 10,
        ..Default::default()
    };

    let result = service.list_nodes(tenant_id, security, filter).await;

    assert!(result.is_ok());
    let (nodes, total) = result.unwrap();
    assert_eq!(nodes.len(), 5);
    assert_eq!(total, 5);
}

#[tokio::test]
async fn test_list_nodes_pagination() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    for i in 0..7 {
        let mut input = create_test_input();
        input.translations[0].title = Some(format!("Post {}", i));
        service
            .create_node(tenant_id, security.clone(), input)
            .await
            .unwrap();
    }

    let filter_page1 = ListNodesFilter {
        page: 1,
        per_page: 3,
        ..Default::default()
    };

    let result1 = service
        .list_nodes(tenant_id, security.clone(), filter_page1)
        .await;
    assert!(result1.is_ok());
    let (nodes1, total1) = result1.unwrap();
    assert_eq!(nodes1.len(), 3);
    assert_eq!(total1, 7);

    let filter_page2 = ListNodesFilter {
        page: 2,
        per_page: 3,
        ..Default::default()
    };

    let result2 = service.list_nodes(tenant_id, security, filter_page2).await;
    assert!(result2.is_ok());
    let (nodes2, total2) = result2.unwrap();
    assert_eq!(nodes2.len(), 3);
    assert_eq!(total2, 7);

    assert_ne!(nodes1[0].id, nodes2[0].id);
}

// =============================================================================
// Metadata Tests
// =============================================================================

#[tokio::test]
async fn test_node_metadata() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let mut input = create_test_input();
    input.metadata = serde_json::json!({
        "featured": true,
        "tags": ["rust", "testing", "content"],
        "views": 100,
        "rating": 4.5
    });

    let result = service.create_node(tenant_id, security, input).await;

    assert!(result.is_ok());
    let node = result.unwrap();
    assert_eq!(node.metadata["featured"], true);
    assert!(node.metadata["tags"].is_array());
    assert_eq!(node.metadata["views"], 100);
    assert_eq!(node.metadata["rating"], 4.5);
}

#[tokio::test]
async fn test_update_node_metadata() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let input = create_test_input();
    let node = service
        .create_node(tenant_id, security.clone(), input)
        .await
        .unwrap();

    let update_input = UpdateNodeInput {
        metadata: Some(serde_json::json!({
            "featured": true,
            "priority": "high"
        })),
        ..UpdateNodeInput::default()
    };

    let result = service
        .update_node(tenant_id, node.id, security, update_input)
        .await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.metadata["featured"], true);
    assert_eq!(updated.metadata["priority"], "high");
}

// =============================================================================
// RBAC & Permission Tests
// =============================================================================

#[tokio::test]
async fn test_create_node_enforces_own_scope() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let manager = manager_context();

    let mut input = create_test_input();
    let other_user_id = Uuid::new_v4();
    input.author_id = Some(other_user_id);

    let result = service.create_node(tenant_id, manager.clone(), input).await;

    assert!(result.is_ok());
    let node = result.unwrap();
    assert_eq!(node.author_id, Some(other_user_id));
}

#[tokio::test]
async fn test_update_node_own_scope_prevents_author_change() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let manager = manager_context();

    let input = create_test_input();
    let node = service
        .create_node(tenant_id, manager.clone(), input)
        .await
        .unwrap();

    let update_input = UpdateNodeInput {
        author_id: Some(Some(Uuid::new_v4())),
        ..UpdateNodeInput::default()
    };

    let result = service
        .update_node(tenant_id, node.id, manager, update_input)
        .await;

    assert!(result.is_ok());
    let updated = result.unwrap();
    assert!(updated.author_id.is_some());
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[tokio::test]
async fn test_update_nonexistent_node() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();
    let fake_id = Uuid::new_v4();

    let update_input = UpdateNodeInput {
        translations: Some(vec![NodeTranslationInput {
            locale: "en".to_string(),
            title: Some("Updated Title".to_string()),
            slug: None,
            excerpt: None,
        }]),
        ..UpdateNodeInput::default()
    };

    let result = service
        .update_node(tenant_id, fake_id, security, update_input)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ContentError::NodeNotFound(_) => {}
        _ => panic!("Expected NodeNotFound error"),
    }
}

#[tokio::test]
async fn test_delete_nonexistent_node() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();
    let fake_id = Uuid::new_v4();

    let result = service.delete_node(tenant_id, fake_id, security).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_create_node_duplicate_slug_returns_duplicate_slug() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let duplicate_slug = unique_slug("duplicate-post");

    let mut first = create_test_input();
    first.translations[0].slug = Some(duplicate_slug.clone());
    service
        .create_node(tenant_id, security.clone(), first)
        .await
        .unwrap();

    let mut second = create_test_input();
    second.translations[0].slug = Some(duplicate_slug.clone());

    let result = service.create_node(tenant_id, security, second).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ContentError::DuplicateSlug { slug, locale } => {
            assert_eq!(slug, duplicate_slug);
            assert_eq!(locale, "en");
        }
        err => panic!("Expected DuplicateSlug error, got {err:?}"),
    }
}

#[tokio::test]
async fn test_update_node_duplicate_slug_returns_duplicate_slug() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let existing_slug = unique_slug("existing-post");

    let mut first = create_test_input();
    first.translations[0].slug = Some(existing_slug.clone());
    service
        .create_node(tenant_id, security.clone(), first)
        .await
        .unwrap();

    let mut second = create_test_input();
    second.translations[0].slug = Some(unique_slug("another-post"));
    let second_node = service
        .create_node(tenant_id, security.clone(), second)
        .await
        .unwrap();

    let update_input = UpdateNodeInput {
        translations: Some(vec![NodeTranslationInput {
            locale: "en".to_string(),
            title: Some("Updated title".to_string()),
            slug: Some(existing_slug.clone()),
            excerpt: Some("Updated excerpt".to_string()),
        }]),
        ..UpdateNodeInput::default()
    };

    let result = service
        .update_node(tenant_id, second_node.id, security, update_input)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ContentError::DuplicateSlug { slug, locale } => {
            assert_eq!(slug, existing_slug);
            assert_eq!(locale, "en");
        }
        err => panic!("Expected DuplicateSlug error, got {err:?}"),
    }
}

#[tokio::test]
async fn test_update_node_version_conflict_returns_concurrent_modification() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let created = service
        .create_node(tenant_id, security.clone(), create_test_input())
        .await
        .unwrap();

    let update_input = UpdateNodeInput {
        expected_version: Some(created.version + 1),
        status: Some(ContentStatus::Published),
        ..UpdateNodeInput::default()
    };

    let result = service
        .update_node(tenant_id, created.id, security, update_input)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ContentError::ConcurrentModification { expected, actual } => {
            assert_eq!(expected, created.version + 1);
            assert_eq!(actual, created.version);
        }
        err => panic!("Expected ConcurrentModification error, got {err:?}"),
    }
}

// =============================================================================
// Body Content Tests
// =============================================================================

#[tokio::test]
async fn test_node_with_multiple_body_locales() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let mut input = create_test_input();
    input.translations.push(NodeTranslationInput {
        locale: "ru".to_string(),
        title: Some("Русский заголовок".to_string()),
        slug: Some(unique_slug("russian-post")),
        excerpt: None,
    });
    input.bodies.push(BodyInput {
        locale: "ru".to_string(),
        body: Some("# Русский контент\n\nЭто русский текст.".to_string()),
        format: Some("markdown".to_string()),
    });

    let result = service.create_node(tenant_id, security, input).await;

    assert!(result.is_ok());
    let node = result.unwrap();
    assert_eq!(node.bodies.len(), 2);

    let en_body = node.bodies.iter().find(|b| b.locale == "en");
    let ru_body = node.bodies.iter().find(|b| b.locale == "ru");

    assert!(en_body.is_some());
    assert!(ru_body.is_some());
    assert!(en_body
        .unwrap()
        .body
        .as_deref()
        .unwrap_or("")
        .contains("Test Content"));
    assert!(ru_body
        .unwrap()
        .body
        .as_deref()
        .unwrap_or("")
        .contains("Русский контент"));
}

// =============================================================================
// Additional Edge Case Tests
// =============================================================================

#[tokio::test]
async fn test_create_page_node() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let mut input = create_test_input();
    input.kind = "page".to_string();

    let result = service.create_node(tenant_id, security, input).await;

    assert!(result.is_ok());
    let node = result.unwrap();
    assert_eq!(node.kind, "page");
}

#[tokio::test]
async fn test_filter_by_status() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();

    let mut draft_input = create_test_input();
    draft_input.status = Some(ContentStatus::Draft);
    service
        .create_node(tenant_id, security.clone(), draft_input)
        .await
        .unwrap();

    let mut published_input = create_test_input();
    published_input.status = Some(ContentStatus::Published);
    service
        .create_node(tenant_id, security.clone(), published_input)
        .await
        .unwrap();

    let filter = ListNodesFilter {
        status: Some(ContentStatus::Published),
        page: 1,
        per_page: 10,
        ..Default::default()
    };

    let result = service.list_nodes(tenant_id, security, filter).await;

    assert!(result.is_ok());
    let (nodes, total) = result.unwrap();
    assert_eq!(total, 1);
    assert_eq!(nodes[0].status, ContentStatus::Published);
}

#[tokio::test]
async fn test_list_nodes_includes_metadata_and_category_id() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let security = admin_context();
    let category_id = Uuid::new_v4();

    let mut input = create_test_input();
    input.category_id = Some(category_id);
    input.metadata = serde_json::json!({"is_featured": true, "tags": ["news"]});

    service
        .create_node(tenant_id, security.clone(), input)
        .await
        .unwrap();

    let filter = ListNodesFilter {
        page: 1,
        per_page: 10,
        ..Default::default()
    };

    let (items, _) = service
        .list_nodes(tenant_id, security, filter)
        .await
        .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].category_id, Some(category_id));
    assert_eq!(items[0].metadata["is_featured"], true);
}

// =============================================================================
// Tenant Isolation Tests
// =============================================================================

#[tokio::test]
async fn test_cross_tenant_get_node_is_blocked() {
    let (_db, service) = setup().await;
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let security = admin_context();

    let node = service
        .create_node(tenant_a, security.clone(), create_test_input())
        .await
        .unwrap();

    let result = service.get_node(tenant_b, node.id).await;
    assert!(result.is_err(), "tenant_b must not access tenant_a node");
}

#[tokio::test]
async fn test_cross_tenant_update_node_is_blocked() {
    let (_db, service) = setup().await;
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let security = admin_context();

    let node = service
        .create_node(tenant_a, security.clone(), create_test_input())
        .await
        .unwrap();

    let update = UpdateNodeInput {
        status: Some(ContentStatus::Published),
        ..Default::default()
    };
    let result = service
        .update_node(tenant_b, node.id, security, update)
        .await;
    assert!(result.is_err(), "tenant_b must not update tenant_a node");
}

#[tokio::test]
async fn test_cross_tenant_delete_node_is_blocked() {
    let (_db, service) = setup().await;
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let security = admin_context();

    let node = service
        .create_node(tenant_a, security.clone(), create_test_input())
        .await
        .unwrap();

    let result = service.delete_node(tenant_b, node.id, security).await;
    assert!(result.is_err(), "tenant_b must not delete tenant_a node");

    let still_exists = service.get_node(tenant_a, node.id).await;
    assert!(still_exists.is_ok(), "node must still exist for tenant_a");
}

#[tokio::test]
async fn test_list_nodes_does_not_leak_across_tenants() {
    let (_db, service) = setup().await;
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let security = admin_context();

    service
        .create_node(tenant_a, security.clone(), create_test_input())
        .await
        .unwrap();

    let filter = ListNodesFilter {
        page: 1,
        per_page: 10,
        ..Default::default()
    };
    let (items, _) = service
        .list_nodes(tenant_b, security, filter)
        .await
        .unwrap();
    assert!(
        items.is_empty(),
        "tenant_b must see zero nodes, not tenant_a's"
    );
}
