use std::sync::Arc;

use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
    ForumStorefrontReadStateService, ForumModule, ReplyService, TopicService,
};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TransactionalEventBus, Uuid) {
    let db_url = format!(
        "sqlite:file:forum_storefront_read_state_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum storefront read-state sqlite database should connect");
    let schema = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in ForumModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("forum migration should apply");
    }
    let event_bus = TransactionalEventBus::new(Arc::new(MemoryTransport::new()));
    (db, event_bus, Uuid::new_v4())
}

#[tokio::test]
async fn visible_topic_summary_and_current_mark_share_owner_policy() {
    let (db, event_bus, tenant_id) = setup().await;
    let author = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let reader = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let anonymous = SecurityContext::new(UserRole::Customer, None);

    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            author.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "Storefront read state".into(),
                slug: "storefront-read-state".into(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category should be created");
    let topic = TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            author.clone(),
            CreateTopicInput {
                locale: "en".into(),
                category_id: category.id,
                title: "Visible storefront topic".into(),
                slug: Some("visible-storefront-topic".into()),
                body: "Opening body".into(),
                body_format: "markdown".into(),
                content_json: None,
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");
    ReplyService::new(db.clone(), event_bus)
        .create(
            tenant_id,
            author,
            topic.id,
            CreateReplyInput {
                locale: "en".into(),
                content: "Approved reply".into(),
                content_format: "markdown".into(),
                content_json: None,
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    let service = ForumStorefrontReadStateService::new(db);
    let unseen = service
        .summarize_topics(
            tenant_id,
            reader.clone(),
            vec![topic.id, topic.id],
        )
        .await
        .expect("bounded visible topic IDs should summarize");
    assert_eq!(unseen.len(), 1);
    assert_eq!(unseen[0].topic_id, topic.id);
    assert!(unseen[0].is_unread);
    assert!(unseen[0].unread_count > 0 || unseen[0].has_unread_topic_revision);

    let state = service
        .mark_topic_read_current(tenant_id, topic.id, reader.clone())
        .await
        .expect("current visible topic should be marked read");
    assert!(state.explicit);

    let read = service
        .summarize_topics(tenant_id, reader, vec![topic.id])
        .await
        .expect("marked topic should summarize");
    assert_eq!(read.len(), 1);
    assert!(!read[0].is_unread);
    assert_eq!(read[0].unread_count, 0);
    assert!(!read[0].has_unread_topic_revision);

    assert!(
        service
            .summarize_topics(tenant_id, anonymous.clone(), vec![topic.id])
            .await
            .is_err()
    );
    assert!(
        service
            .mark_topic_read_current(tenant_id, topic.id, anonymous)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn storefront_summary_rejects_unbounded_id_sets_before_querying() {
    let (db, _, tenant_id) = setup().await;
    let reader = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let error = ForumStorefrontReadStateService::new(db)
        .summarize_topics(
            tenant_id,
            reader,
            (0..101).map(|_| Uuid::new_v4()).collect(),
        )
        .await
        .expect_err("unbounded topic ID set should fail");
    assert!(error.to_string().contains("limited to 100 topic IDs"));
}
