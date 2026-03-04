// Integration tests for multi-tenant content isolation.

use migration::Migrator;
use rustok_content::dto::{BodyInput, CreateNodeInput, ListNodesFilter, NodeTranslationInput};
use rustok_content::services::NodeService;
use rustok_core::{SecurityContext, UserRole};
use rustok_test_utils::{db::setup_test_db_with_migrations, events::mock_transactional_event_bus};
use uuid::Uuid;

fn build_post_input(title: &str, slug: &str) -> CreateNodeInput {
    CreateNodeInput {
        kind: "post".to_string(),
        translations: vec![NodeTranslationInput {
            locale: "en".to_string(),
            title: Some(title.to_string()),
            slug: Some(slug.to_string()),
            excerpt: Some(format!("Excerpt for {title}")),
        }],
        bodies: vec![BodyInput {
            locale: "en".to_string(),
            body: Some(format!("Body for {title}")),
            format: Some("markdown".to_string()),
        }],
        status: None,
        parent_id: None,
        author_id: None,
        category_id: None,
        position: None,
        depth: None,
        reply_count: None,
        metadata: serde_json::json!({}),
    }
}

#[tokio::test]
async fn list_nodes_is_scoped_by_tenant() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let event_bus = mock_transactional_event_bus();
    let service = NodeService::new(db, event_bus);

    let tenant1_id = Uuid::new_v4();
    let tenant2_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(user_id));

    let node1 = service
        .create_node(
            tenant1_id,
            security.clone(),
            build_post_input("Tenant 1 Post", "tenant-1-post"),
        )
        .await
        .expect("tenant1 node should be created");

    let node2 = service
        .create_node(
            tenant2_id,
            security.clone(),
            build_post_input("Tenant 2 Post", "tenant-2-post"),
        )
        .await
        .expect("tenant2 node should be created");

    let filter = ListNodesFilter {
        kind: Some("post".to_string()),
        status: None,
        parent_id: None,
        author_id: None,
        category_id: None,
        locale: Some("en".to_string()),
        page: 1,
        per_page: 10,
        include_deleted: false,
    };

    let (tenant1_nodes, tenant1_total) = service
        .list_nodes(tenant1_id, security.clone(), filter.clone())
        .await
        .expect("tenant1 list should succeed");

    let (tenant2_nodes, tenant2_total) = service
        .list_nodes(tenant2_id, security, filter)
        .await
        .expect("tenant2 list should succeed");

    assert_eq!(tenant1_total, 1);
    assert_eq!(tenant2_total, 1);
    assert_eq!(tenant1_nodes[0].id, node1.id);
    assert_eq!(tenant2_nodes[0].id, node2.id);
    assert_ne!(tenant1_nodes[0].title, tenant2_nodes[0].title);
}

#[tokio::test]
async fn get_by_slug_is_scoped_by_tenant() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let event_bus = mock_transactional_event_bus();
    let service = NodeService::new(db, event_bus);

    let tenant1_id = Uuid::new_v4();
    let tenant2_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(user_id));

    let node1 = service
        .create_node(
            tenant1_id,
            security.clone(),
            build_post_input("Tenant 1 Unique", "unique-tenant-1"),
        )
        .await
        .expect("tenant1 node should be created");

    let node2 = service
        .create_node(
            tenant2_id,
            security,
            build_post_input("Tenant 2 Unique", "unique-tenant-2"),
        )
        .await
        .expect("tenant2 node should be created");

    let tenant1_node = service
        .get_by_slug(tenant1_id, "post", "en", "unique-tenant-1")
        .await
        .expect("tenant1 lookup should succeed")
        .expect("tenant1 node should exist");

    let tenant2_node = service
        .get_by_slug(tenant2_id, "post", "en", "unique-tenant-2")
        .await
        .expect("tenant2 lookup should succeed")
        .expect("tenant2 node should exist");

    assert_eq!(tenant1_node.id, node1.id);
    assert_eq!(tenant2_node.id, node2.id);

    let cross_tenant_lookup = service
        .get_by_slug(tenant1_id, "post", "en", "unique-tenant-2")
        .await
        .expect("cross-tenant lookup should execute");

    assert!(cross_tenant_lookup.is_none());
}
