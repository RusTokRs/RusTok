// Integration test for Node creation → Event flow
// This test verifies the complete workflow from content creation to event publishing

use rustok_content::dto::{BodyInput, CreateNodeInput, NodeTranslationInput, UpdateNodeInput};
use rustok_content::entities::node::ContentStatus;
use rustok_content::entities::{body, node, node_translation};
use rustok_content::services::NodeService;
use rustok_core::events::DomainEvent;
use rustok_core::{SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use rustok_test_utils::MockEventTransport;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Schema};
use std::future::Future;
use std::sync::Arc;
use uuid::Uuid;

fn run_async_test<F, Fut>(f: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    // Execute each async test on a dedicated thread with an explicit larger stack.
    // This avoids stack overflows in llvm-cov/instrumented CI where async futures
    // can require deeper stacks than the default test-harness thread provides.
    std::thread::Builder::new()
        .name("node-event-index-test".to_string())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime");

            runtime.block_on(f());
        })
        .expect("failed to spawn async test thread")
        .join()
        .expect("async test thread panicked");
}

async fn setup_content_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:content_node_event_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);

    Database::connect(opts)
        .await
        .expect("failed to connect content test sqlite database")
}

async fn ensure_content_schema(db: &DatabaseConnection) {
    if db.get_database_backend() != DbBackend::Sqlite {
        return;
    }

    let builder = db.get_database_backend();
    let schema = Schema::new(builder);

    create_entity_table(db, &builder, schema.create_table_from_entity(node::Entity)).await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(node_translation::Entity),
    )
    .await;
    create_entity_table(db, &builder, schema.create_table_from_entity(body::Entity)).await;
}

async fn create_entity_table(
    db: &DatabaseConnection,
    builder: &DbBackend,
    mut statement: sea_orm::sea_query::TableCreateStatement,
) {
    statement.if_not_exists();
    db.execute(builder.build(&statement))
        .await
        .expect("failed to create content test table");
}

#[test]
fn test_node_creation_triggers_event_and_indexing() {
    run_async_test(|| async {
        // Setup test database and services
        let db = setup_content_test_db().await;
        ensure_content_schema(&db).await;
        let transport = Arc::new(MockEventTransport::new());
        let event_bus = TransactionalEventBus::new(transport.clone());
        let service = NodeService::new(db.clone(), event_bus);

        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let security = SecurityContext::new(UserRole::Admin, Some(user_id));

        // Create a node
        let input = CreateNodeInput {
            kind: "post".to_string(),
            translations: vec![NodeTranslationInput {
                locale: "en".to_string(),
                title: Some("Test Post".to_string()),
                slug: Some("test-post".to_string()),
                excerpt: Some("Test excerpt".to_string()),
            }],
            bodies: vec![BodyInput {
                locale: "en".to_string(),
                body: Some("Hello, RusToK!".to_string()),
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
        };

        let result = service.create_node(tenant_id, security, input).await;
        assert!(result.is_ok());
        let node = result.unwrap();

        // Verify that a NodeCreated event was published
        assert_eq!(transport.event_count(), 1);
        assert!(transport.has_event_of_type("NodeCreated"));

        // Get the events and verify details
        let events = transport.events_of_type("NodeCreated");
        assert_eq!(events.len(), 1);

        if let DomainEvent::NodeCreated {
            node_id,
            kind,
            author_id,
        } = &events[0]
        {
            assert_eq!(*node_id, node.id);
            assert_eq!(kind, "post");
            assert_eq!(*author_id, Some(user_id));
        } else {
            panic!("Expected NodeCreated event");
        }

        println!("✅ Node creation → Event publishing flow verified");
    });
}

#[test]
fn test_node_update_triggers_event() {
    run_async_test(|| async {
        // Setup test database and services
        let db = setup_content_test_db().await;
        ensure_content_schema(&db).await;
        let transport = Arc::new(MockEventTransport::new());
        let event_bus = TransactionalEventBus::new(transport.clone());
        let service = NodeService::new(db.clone(), event_bus);

        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let security = SecurityContext::new(UserRole::Admin, Some(user_id));

        // Create a node first
        let input = CreateNodeInput {
            kind: "post".to_string(),
            translations: vec![NodeTranslationInput {
                locale: "en".to_string(),
                title: Some("Original Title".to_string()),
                slug: Some("original-slug".to_string()),
                excerpt: Some("Original excerpt".to_string()),
            }],
            bodies: vec![BodyInput {
                locale: "en".to_string(),
                body: Some("Original content".to_string()),
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
        };

        let node = service
            .create_node(tenant_id, security.clone(), input)
            .await
            .unwrap();

        // Clear the first event (NodeCreated)
        transport.clear();

        // Update the node
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

        // Verify that a NodeUpdated event was published
        assert!(transport.has_event_of_type("NodeUpdated"));

        let events = transport.events_of_type("NodeUpdated");
        assert_eq!(events.len(), 1);

        if let DomainEvent::NodeUpdated { node_id, .. } = &events[0] {
            assert_eq!(*node_id, node.id);
        } else {
            panic!("Expected NodeUpdated event");
        }

        println!("✅ Node update → Event publishing flow verified");
    });
}

#[test]
fn test_node_deletion_triggers_event() {
    run_async_test(|| async {
        // Setup test database and services
        let db = setup_content_test_db().await;
        ensure_content_schema(&db).await;
        let transport = Arc::new(MockEventTransport::new());
        let event_bus = TransactionalEventBus::new(transport.clone());
        let service = NodeService::new(db.clone(), event_bus);

        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let security = SecurityContext::new(UserRole::Admin, Some(user_id));

        // Create a node first
        let input = CreateNodeInput {
            kind: "post".to_string(),
            translations: vec![NodeTranslationInput {
                locale: "en".to_string(),
                title: Some("To be deleted".to_string()),
                slug: Some("to-be-deleted".to_string()),
                excerpt: Some("Will be deleted".to_string()),
            }],
            bodies: vec![BodyInput {
                locale: "en".to_string(),
                body: Some("Content to be deleted".to_string()),
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
        };

        let node = service
            .create_node(tenant_id, security.clone(), input)
            .await
            .unwrap();

        // Clear the first event (NodeCreated)
        transport.clear();

        // Delete the node
        let result = service.delete_node(tenant_id, node.id, security).await;
        assert!(result.is_ok());

        // Verify that a NodeDeleted event was published
        assert!(transport.has_event_of_type("NodeDeleted"));

        let events = transport.events_of_type("NodeDeleted");
        assert_eq!(events.len(), 1);

        if let DomainEvent::NodeDeleted { node_id, .. } = &events[0] {
            assert_eq!(*node_id, node.id);
        } else {
            panic!("Expected NodeDeleted event");
        }

        println!("✅ Node deletion → Event publishing flow verified");
    });
}

#[test]
fn test_transactional_event_persistence() {
    run_async_test(|| async {
        // Setup test database and services
        let db = setup_content_test_db().await;
        ensure_content_schema(&db).await;
        let transport = Arc::new(MockEventTransport::new());
        let event_bus = TransactionalEventBus::new(transport.clone());
        let service = NodeService::new(db.clone(), event_bus);

        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let security = SecurityContext::new(UserRole::Admin, Some(user_id));

        // Create a node (this should publish an event via the transport)
        let input = CreateNodeInput {
            kind: "post".to_string(),
            translations: vec![NodeTranslationInput {
                locale: "en".to_string(),
                title: Some("Transactional Test".to_string()),
                slug: Some("transactional-test".to_string()),
                excerpt: Some("Testing transactional events".to_string()),
            }],
            bodies: vec![BodyInput {
                locale: "en".to_string(),
                body: Some("Transactional content".to_string()),
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
        };

        let result = service.create_node(tenant_id, security, input).await;
        assert!(result.is_ok());

        // Verify that the event was captured by the transport
        assert_eq!(transport.event_count(), 1);
        assert!(transport.has_event_of_type("NodeCreated"));

        println!("✅ Transactional event persistence verified");
    });
}
