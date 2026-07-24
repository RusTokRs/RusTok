use std::sync::Arc;

use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_forum::entities::forum_topic_read_state;
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumModule,
    ForumTopicReadStateService, MarkForumTopicReadInput, ReplyService, TopicService,
};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait,
    QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TransactionalEventBus, Uuid) {
    let db_url = format!(
        "sqlite:file:forum_topic_read_state_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum topic read state sqlite database should connect");
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

async fn create_topic_with_two_public_replies(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    author: SecurityContext,
    reader: SecurityContext,
) -> Uuid {
    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            author.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "Read tracking".into(),
                slug: "read-tracking".into(),
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
            author,
            CreateTopicInput {
                locale: "en".into(),
                category_id: category.id,
                title: "Monotonic state".into(),
                slug: Some("monotonic-state".into()),
                body: "Body".into(),
                body_format: "markdown".into(),
                content_json: None,
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");
    let replies = ReplyService::new(db.clone(), event_bus.clone());
    for content in ["First", "Second"] {
        replies
            .create(
                tenant_id,
                reader.clone(),
                topic.id,
                CreateReplyInput {
                    locale: "en".into(),
                    content: content.into(),
                    content_format: "markdown".into(),
                    content_json: None,
                    parent_reply_id: None,
                },
            )
            .await
            .expect("public reply should be created");
    }
    topic.id
}

#[tokio::test]
async fn topic_read_state_is_bounded_authenticated_and_monotonic() {
    let (db, event_bus, tenant_id) = setup().await;
    let author_id = Uuid::new_v4();
    let reader_id = Uuid::new_v4();
    let author = SecurityContext::new(UserRole::Admin, Some(author_id));
    let reader = SecurityContext::new(UserRole::Customer, Some(reader_id));
    let anonymous = SecurityContext::new(UserRole::Customer, None);
    let topic_id = create_topic_with_two_public_replies(
        &db,
        &event_bus,
        tenant_id,
        author,
        reader.clone(),
    )
    .await;
    let service = ForumTopicReadStateService::new(db.clone());

    let anonymous_state = service
        .get_topic_read_state(tenant_id, topic_id, anonymous.clone())
        .await
        .expect("anonymous read state should be implicit");
    assert!(!anonymous_state.explicit);
    assert_eq!(anonymous_state.user_id, None);
    assert_eq!(anonymous_state.last_read_position, 0);

    let anonymous_write = service
        .mark_topic_read(
            tenant_id,
            topic_id,
            anonymous,
            MarkForumTopicReadInput {
                last_read_position: 1,
                last_read_revision: 0,
            },
        )
        .await;
    assert!(anonymous_write.is_err(), "anonymous views must not create read rows");
    assert_eq!(
        forum_topic_read_state::Entity::find()
            .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_read_state::Column::TopicId.eq(topic_id))
            .count(&db)
            .await
            .expect("read state count should load"),
        0
    );

    let first = service
        .mark_topic_read(
            tenant_id,
            topic_id,
            reader.clone(),
            MarkForumTopicReadInput {
                last_read_position: 1,
                last_read_revision: 0,
            },
        )
        .await
        .expect("first read state should persist");
    assert!(first.explicit);
    assert_eq!(first.last_read_position, 1);
    assert_eq!(first.last_read_revision, 0);

    let advanced = service
        .mark_topic_read(
            tenant_id,
            topic_id,
            reader.clone(),
            MarkForumTopicReadInput {
                last_read_position: 2,
                last_read_revision: 0,
            },
        )
        .await
        .expect("read state should advance");
    assert_eq!(advanced.last_read_position, 2);

    let regressed_input = service
        .mark_topic_read(
            tenant_id,
            topic_id,
            reader.clone(),
            MarkForumTopicReadInput {
                last_read_position: 1,
                last_read_revision: 0,
            },
        )
        .await
        .expect("stale device progress should become a no-op");
    assert_eq!(regressed_input.last_read_position, 2);

    let future_position = service
        .mark_topic_read(
            tenant_id,
            topic_id,
            reader.clone(),
            MarkForumTopicReadInput {
                last_read_position: 3,
                last_read_revision: 0,
            },
        )
        .await;
    assert!(future_position.is_err(), "future public positions must be rejected");

    let future_revision = service
        .mark_topic_read(
            tenant_id,
            topic_id,
            reader,
            MarkForumTopicReadInput {
                last_read_position: 2,
                last_read_revision: 1,
            },
        )
        .await;
    assert!(future_revision.is_err(), "future topic revisions must be rejected");

    let direct_regression = forum_topic_read_state::Entity::update_many()
        .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_read_state::Column::TopicId.eq(topic_id))
        .filter(forum_topic_read_state::Column::UserId.eq(reader_id))
        .set(forum_topic_read_state::ActiveModel {
            last_read_position: Set(0),
            ..Default::default()
        })
        .exec(&db)
        .await;
    assert!(
        direct_regression.is_err(),
        "database guard must reject direct read-position regression"
    );
}
