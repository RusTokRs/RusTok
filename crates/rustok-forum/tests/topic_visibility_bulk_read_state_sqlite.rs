use std::sync::Arc;

use chrono::{Duration, Utc};
use rustok_api::{PortActor, PortContext};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::entities::{forum_topic, forum_topic_read_state};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints,
    ForumCategoryAudiencePolicyService, ForumError, ForumModule,
    ForumVisibilityScopedReadStateService, MarkForumTopicsReadBatchInput,
    SetForumCategoryAudiencePolicyInput, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectionTrait,
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectOptions, Database, DatabaseConnection,
    EntityTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let db_url = format!(
        "sqlite:file:forum_topic_visibility_bulk_read_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum visible bulk read sqlite database should connect");
    let schema = SchemaManager::new(&db);
        for migration in OutboxModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("outbox migration should apply");
    }
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("taxonomy migration should apply");
    }
        db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        );",
    )
    .await
    .expect("users table fixture should apply");
    for migration in ForumModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("forum migration should apply");
    }
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (db, event_bus)
}

async fn create_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    security: SecurityContext,
    slug: &str,
) -> Uuid {
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
        .id
}

async fn create_topic(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    category_id: Uuid,
    security: SecurityContext,
    title: &str,
    channel_slug: &str,
) -> Uuid {
    TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".into(),
                category_id,
                title: title.into(),
                slug: Some(title.to_ascii_lowercase().replace(' ', "-")),
                body: rustok_api::RichTextDocument::single_paragraph(format!("{title} body")),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: Some(vec![channel_slug.into()]),
            },
        )
        .await
        .expect("topic should be created")
        .id
}

async fn set_topic_cursor_time(db: &DatabaseConnection, topic_id: Uuid, seconds_before_now: i64) {
    let model = forum_topic::Entity::find_by_id(topic_id)
        .one(db)
        .await
        .expect("topic cursor row should load")
        .expect("topic cursor row should exist");
    let timestamp = Utc::now() - Duration::seconds(seconds_before_now);
    let mut active: forum_topic::ActiveModel = model.into();
    active.created_at = Set(timestamp.into());
    active.updated_at = Set(timestamp.into());
    active
        .update(db)
        .await
        .expect("topic cursor timestamp should update");
}

fn read_context(tenant_id: Uuid, user_id: Uuid, channel_slug: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        format!("visible-bulk-{channel_slug}"),
    )
    .with_channel(channel_slug)
}

#[tokio::test]
async fn visible_bulk_read_advances_raw_cursor_without_marking_denied_topics() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let reader_id = Uuid::new_v4();
    let reader = SecurityContext::new(UserRole::Customer, Some(reader_id));
    let category = create_category(&db, tenant_id, admin.clone(), "general").await;

    let hidden_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category,
        admin.clone(),
        "First hidden topic",
        "members",
    )
    .await;
    let visible_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category,
        admin.clone(),
        "Second visible topic",
        "web",
    )
    .await;
    set_topic_cursor_time(&db, hidden_topic, 2).await;
    set_topic_cursor_time(&db, visible_topic, 1).await;

    let service = ForumVisibilityScopedReadStateService::new(db.clone());
    let first = service
        .mark_all_read_with_audience_context(
            tenant_id,
            reader.clone(),
            read_context(tenant_id, reader_id, "web"),
            MarkForumTopicsReadBatchInput {
                cursor: None,
                limit: Some(1),
            },
        )
        .await
        .expect("first visible bulk page should succeed");
    assert_eq!(first.processed, 0);
    assert!(first.has_more);
    let cursor = first.next_cursor.expect("hidden raw page should advance");

    let cross_channel = service
        .mark_all_read_with_audience_context(
            tenant_id,
            reader.clone(),
            read_context(tenant_id, reader_id, "members"),
            MarkForumTopicsReadBatchInput {
                cursor: Some(cursor.clone()),
                limit: Some(1),
            },
        )
        .await;
    assert!(matches!(
        cross_channel,
        Err(ForumError::Validation(message)) if message.contains("visibility-scoped bulk read cursor")
    ));

    let second = service
        .mark_all_read_with_audience_context(
            tenant_id,
            reader.clone(),
            read_context(tenant_id, reader_id, "web"),
            MarkForumTopicsReadBatchInput {
                cursor: Some(cursor),
                limit: Some(1),
            },
        )
        .await
        .expect("second visible bulk page should succeed");
    assert_eq!(second.processed, 1);
    assert!(!second.has_more);
    assert_eq!(second.next_cursor, None);

    let states = forum_topic_read_state::Entity::find()
        .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_read_state::Column::UserId.eq(reader_id))
        .all(&db)
        .await
        .expect("read states should load");
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].topic_id, visible_topic);
    assert_ne!(states[0].topic_id, hidden_topic);

    ForumCategoryAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            category,
            admin,
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    deny_user_ids: vec![reader_id],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("category deny policy should persist");

    assert!(matches!(
        service
            .mark_category_read_with_audience_context(
                tenant_id,
                category,
                reader,
                read_context(tenant_id, reader_id, "web"),
                MarkForumTopicsReadBatchInput::default(),
            )
            .await,
        Err(ForumError::CategoryNotFound(id)) if id == category
    ));
}
