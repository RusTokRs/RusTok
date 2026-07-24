use std::sync::Arc;

use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumModule,
    ForumStorefrontReadStateService, ListTopicsFilter, ReplyService, TopicService,
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

fn topic_filter(category_id: Option<Uuid>, per_page: u64) -> ListTopicsFilter {
    ListTopicsFilter {
        category_id,
        status: None,
        locale: Some("en".into()),
        page: 1,
        per_page,
    }
}

async fn create_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    security: SecurityContext,
    slug: &str,
) -> rustok_forum::CategoryResponse {
    CategoryService::new(db.clone())
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".into(),
                name: slug.replace('-', " "),
                slug: slug.into(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category should be created")
}

#[tokio::test]
async fn visible_topic_summary_and_current_mark_share_owner_policy() {
    let (db, event_bus, tenant_id) = setup().await;
    let author = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let reader = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let anonymous = SecurityContext::new(UserRole::Customer, None);

    let category = create_category(&db, tenant_id, author.clone(), "storefront-read-state").await;
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
    ReplyService::new(db.clone(), event_bus.clone())
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

    let service = ForumStorefrontReadStateService::new(db, event_bus);
    let unseen = service
        .list_topics_with_unread(
            tenant_id,
            reader.clone(),
            topic_filter(Some(category.id), 20),
            Some("en"),
            None,
        )
        .await
        .expect("visible topic page should summarize");
    assert_eq!(unseen.items.len(), 1);
    assert_eq!(unseen.items[0].topic.id, topic.id);
    assert!(unseen.items[0].is_unread);
    assert!(unseen.items[0].unread_count > 0 || unseen.items[0].has_unread_topic_revision);

    let state = service
        .mark_topic_read_current_visible(
            tenant_id,
            topic.id,
            reader.clone(),
            "en",
            Some("en"),
            None,
        )
        .await
        .expect("current visible topic should be marked read");
    assert!(state.explicit);

    let read = service
        .list_topics_with_unread(
            tenant_id,
            reader,
            topic_filter(Some(category.id), 20),
            Some("en"),
            None,
        )
        .await
        .expect("marked topic should summarize");
    assert_eq!(read.items.len(), 1);
    assert!(!read.items[0].is_unread);
    assert_eq!(read.items[0].unread_count, 0);
    assert!(!read.items[0].has_unread_topic_revision);

    assert!(
        service
            .list_topics_with_unread(
                tenant_id,
                anonymous.clone(),
                topic_filter(Some(category.id), 20),
                Some("en"),
                None,
            )
            .await
            .is_err()
    );
    assert!(
        service
            .mark_topic_read_current_visible(
                tenant_id,
                topic.id,
                anonymous,
                "en",
                Some("en"),
                None,
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn channel_restricted_topic_cannot_be_enriched_or_marked_outside_its_channel() {
    let (db, event_bus, tenant_id) = setup().await;
    let author = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let reader = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let category = create_category(&db, tenant_id, author.clone(), "restricted-channel").await;
    let topic = TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            author,
            CreateTopicInput {
                locale: "en".into(),
                category_id: category.id,
                title: "Mobile only".into(),
                slug: Some("mobile-only".into()),
                body: "Restricted body".into(),
                body_format: "markdown".into(),
                content_json: None,
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: Some(vec!["mobile".into()]),
            },
        )
        .await
        .expect("restricted topic should be created");
    let service = ForumStorefrontReadStateService::new(db, event_bus);

    let public_page = service
        .list_topics_with_unread(
            tenant_id,
            reader.clone(),
            topic_filter(Some(category.id), 20),
            Some("en"),
            None,
        )
        .await
        .expect("nonmatching storefront scope should still return a page");
    assert!(public_page.items.is_empty());
    assert!(
        service
            .mark_topic_read_current_visible(
                tenant_id,
                topic.id,
                reader.clone(),
                "en",
                Some("en"),
                None,
            )
            .await
            .is_err()
    );

    let mobile_page = service
        .list_topics_with_unread(
            tenant_id,
            reader.clone(),
            topic_filter(Some(category.id), 20),
            Some("en"),
            Some("mobile"),
        )
        .await
        .expect("matching channel should expose the topic");
    assert_eq!(mobile_page.items.len(), 1);
    assert_eq!(mobile_page.items[0].topic.id, topic.id);
    service
        .mark_topic_read_current_visible(
            tenant_id,
            topic.id,
            reader,
            "en",
            Some("en"),
            Some("mobile"),
        )
        .await
        .expect("matching channel should permit the read-state mutation");
}

#[tokio::test]
async fn storefront_unread_page_rejects_unbounded_requests_before_querying() {
    let (db, event_bus, tenant_id) = setup().await;
    let reader = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let error = ForumStorefrontReadStateService::new(db, event_bus)
        .list_topics_with_unread(tenant_id, reader, topic_filter(None, 101), Some("en"), None)
        .await
        .expect_err("unbounded storefront unread page should fail");
    assert!(error.to_string().contains("between 1 and 100"));
}
