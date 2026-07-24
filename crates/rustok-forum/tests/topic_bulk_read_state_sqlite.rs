use std::collections::HashSet;
use std::sync::Arc;

use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_forum::entities::forum_topic_read_state;
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumModule,
    ForumReadModelService, ForumTopicReadStateService, MarkForumTopicReadInput,
    MarkForumTopicsReadBatchInput, ModerationService, ReplyService, TopicService,
    TopicUnreadCursorQuery, UpdateTopicInput,
};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TransactionalEventBus, Uuid) {
    let db_url = format!(
        "sqlite:file:forum_topic_bulk_read_state_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum topic bulk read sqlite database should connect");
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

async fn create_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    security: SecurityContext,
    name: &str,
    parent_id: Option<Uuid>,
    moderated: bool,
) -> Uuid {
    CategoryService::new(db.clone())
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".into(),
                name: name.into(),
                slug: name.to_ascii_lowercase().replace(' ', "-"),
                description: None,
                icon: None,
                color: None,
                parent_id,
                position: None,
                moderated,
            },
        )
        .await
        .expect("category should be created")
        .id
}

async fn create_topic(
    topics: &TopicService,
    tenant_id: Uuid,
    category_id: Uuid,
    security: SecurityContext,
    title: &str,
) -> Uuid {
    topics
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".into(),
                category_id,
                title: title.into(),
                slug: Some(title.to_ascii_lowercase().replace(' ', "-")),
                body: format!("{title} body"),
                body_format: "markdown".into(),
                content_json: None,
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created")
        .id
}

async fn add_two_public_replies_and_revision(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    topic_id: Uuid,
    author: SecurityContext,
    reader: SecurityContext,
) {
    let replies = ReplyService::new(db.clone(), event_bus.clone());
    for content in ["First reply", "Second reply"] {
        replies
            .create(
                tenant_id,
                reader.clone(),
                topic_id,
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
    TopicService::new(db.clone(), event_bus.clone())
        .update(
            tenant_id,
            topic_id,
            author,
            UpdateTopicInput {
                locale: "en".into(),
                title: Some(format!("Updated {topic_id}")),
                body: Some("Updated topic body".into()),
                body_format: Some("markdown".into()),
                content_json: None,
                metadata: None,
                tags: None,
                channel_slugs: None,
            },
        )
        .await
        .expect("topic revision should be created");
}

async fn finish_category_read(
    service: &ForumTopicReadStateService,
    tenant_id: Uuid,
    category_id: Uuid,
    reader: SecurityContext,
    first_cursor: Option<String>,
) -> u64 {
    let mut processed = 0;
    let mut cursor = first_cursor;
    loop {
        let page = service
            .mark_category_read(
                tenant_id,
                category_id,
                reader.clone(),
                MarkForumTopicsReadBatchInput {
                    cursor,
                    limit: Some(1),
                },
            )
            .await
            .expect("category read page should succeed");
        processed += page.processed;
        if !page.has_more {
            assert_eq!(page.next_cursor, None);
            break;
        }
        cursor = page.next_cursor;
    }
    processed
}

async fn finish_all_read(
    service: &ForumTopicReadStateService,
    tenant_id: Uuid,
    reader: SecurityContext,
) -> u64 {
    let mut processed = 0;
    let mut cursor = None;
    loop {
        let page = service
            .mark_all_read(
                tenant_id,
                reader.clone(),
                MarkForumTopicsReadBatchInput {
                    cursor,
                    limit: Some(2),
                },
            )
            .await
            .expect("tenant read page should succeed");
        processed += page.processed;
        if !page.has_more {
            assert_eq!(page.next_cursor, None);
            break;
        }
        cursor = page.next_cursor;
    }
    processed
}

#[tokio::test]
async fn category_and_all_read_are_bounded_resumable_and_scope_safe() {
    let (db, event_bus, tenant_id) = setup().await;
    let author = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let reader_id = Uuid::new_v4();
    let reader = SecurityContext::new(UserRole::Customer, Some(reader_id));
    let anonymous = SecurityContext::new(UserRole::Customer, None);

    let root = create_category(
        &db,
        tenant_id,
        author.clone(),
        "Root category",
        None,
        false,
    )
    .await;
    let child = create_category(
        &db,
        tenant_id,
        author.clone(),
        "Child category",
        Some(root),
        false,
    )
    .await;
    let other = create_category(
        &db,
        tenant_id,
        author.clone(),
        "Other category",
        None,
        false,
    )
    .await;

    let topics = TopicService::new(db.clone(), event_bus.clone());
    let root_topic = create_topic(
        &topics,
        tenant_id,
        root,
        author.clone(),
        "Root topic",
    )
    .await;
    let child_topic_one = create_topic(
        &topics,
        tenant_id,
        child,
        author.clone(),
        "Child topic one",
    )
    .await;
    let child_topic_two = create_topic(
        &topics,
        tenant_id,
        child,
        author.clone(),
        "Child topic two",
    )
    .await;
    let other_topic = create_topic(
        &topics,
        tenant_id,
        other,
        author.clone(),
        "Other topic",
    )
    .await;
    for topic_id in [root_topic, child_topic_one, child_topic_two, other_topic] {
        add_two_public_replies_and_revision(
            &db,
            &event_bus,
            tenant_id,
            topic_id,
            author.clone(),
            reader.clone(),
        )
        .await;
    }

    let service = ForumTopicReadStateService::new(db.clone());
    let first = service
        .mark_category_read(
            tenant_id,
            root,
            reader.clone(),
            MarkForumTopicsReadBatchInput {
                cursor: None,
                limit: Some(1),
            },
        )
        .await
        .expect("first category read page should succeed");
    assert_eq!(first.processed, 1);
    assert!(first.has_more);
    let category_cursor = first
        .next_cursor
        .clone()
        .expect("first category page should continue");

    let late_topic = create_topic(
        &topics,
        tenant_id,
        root,
        author.clone(),
        "Created after category snapshot",
    )
    .await;
    let remaining_processed = finish_category_read(
        &service,
        tenant_id,
        root,
        reader.clone(),
        Some(category_cursor.clone()),
    )
    .await;
    assert_eq!(first.processed + remaining_processed, 3);

    let category_states = forum_topic_read_state::Entity::find()
        .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_read_state::Column::UserId.eq(reader_id))
        .order_by_asc(forum_topic_read_state::Column::TopicId)
        .all(&db)
        .await
        .expect("category read states should load");
    let category_topic_ids = category_states
        .iter()
        .map(|state| state.topic_id)
        .collect::<HashSet<_>>();
    assert_eq!(
        category_topic_ids,
        HashSet::from([root_topic, child_topic_one, child_topic_two])
    );
    assert!(!category_topic_ids.contains(&other_topic));
    assert!(!category_topic_ids.contains(&late_topic));
    for state in &category_states {
        assert_eq!(state.last_read_position, 2);
        assert!(state.last_read_revision > 0);
    }

    let cross_scope_cursor = service
        .mark_all_read(
            tenant_id,
            reader.clone(),
            MarkForumTopicsReadBatchInput {
                cursor: Some(category_cursor),
                limit: Some(2),
            },
        )
        .await;
    assert!(
        cross_scope_cursor.is_err(),
        "category cursors must not be accepted by the tenant-wide command"
    );

    let all_processed = finish_all_read(&service, tenant_id, reader.clone()).await;
    assert_eq!(all_processed, 5);
    let all_states = forum_topic_read_state::Entity::find()
        .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_read_state::Column::UserId.eq(reader_id))
        .all(&db)
        .await
        .expect("tenant read states should load");
    assert_eq!(all_states.len(), 5);

    let replay_processed = finish_all_read(&service, tenant_id, reader.clone()).await;
    assert_eq!(replay_processed, 5);
    assert_eq!(
        forum_topic_read_state::Entity::find()
            .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_read_state::Column::UserId.eq(reader_id))
            .all(&db)
            .await
            .expect("replayed states should load")
            .len(),
        5
    );

    let anonymous_result = service
        .mark_all_read(
            tenant_id,
            anonymous,
            MarkForumTopicsReadBatchInput::default(),
        )
        .await;
    assert!(anonymous_result.is_err());
    let oversized_result = service
        .mark_all_read(
            tenant_id,
            reader.clone(),
            MarkForumTopicsReadBatchInput {
                cursor: None,
                limit: Some(101),
            },
        )
        .await;
    assert!(oversized_result.is_err());
    let foreign_category_result = service
        .mark_category_read(
            Uuid::new_v4(),
            root,
            reader,
            MarkForumTopicsReadBatchInput::default(),
        )
        .await;
    assert!(foreign_category_result.is_err());
}

#[tokio::test]
async fn repeated_topic_mark_clears_late_approval_without_regression() {
    let (db, event_bus, tenant_id) = setup().await;
    let author = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let reader = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let category = create_category(
        &db,
        tenant_id,
        author.clone(),
        "Moderated category",
        None,
        true,
    )
    .await;
    let topics = TopicService::new(db.clone(), event_bus.clone());
    let topic_id = create_topic(
        &topics,
        tenant_id,
        category,
        author.clone(),
        "Late approval topic",
    )
    .await;
    let replies = ReplyService::new(db.clone(), event_bus.clone());
    let first_pending = replies
        .create(
            tenant_id,
            reader.clone(),
            topic_id,
            CreateReplyInput {
                locale: "en".into(),
                content: "Pending position one".into(),
                content_format: "markdown".into(),
                content_json: None,
                parent_reply_id: None,
            },
        )
        .await
        .expect("first pending reply should be created");
    let second_pending = replies
        .create(
            tenant_id,
            reader.clone(),
            topic_id,
            CreateReplyInput {
                locale: "en".into(),
                content: "Pending position two".into(),
                content_format: "markdown".into(),
                content_json: None,
                parent_reply_id: None,
            },
        )
        .await
        .expect("second pending reply should be created");
    let moderation = ModerationService::new(db.clone(), event_bus);
    moderation
        .approve_reply(tenant_id, second_pending.id, topic_id, author.clone())
        .await
        .expect("position two should be approved first");

    let read_state = ForumTopicReadStateService::new(db.clone());
    read_state
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
        .expect("position two should be marked read");
    moderation
        .approve_reply(tenant_id, first_pending.id, topic_id, author)
        .await
        .expect("position one should be approved later");

    let projection = ForumReadModelService::new(db.clone());
    let before_remark = projection
        .list_topics_with_unread(
            tenant_id,
            reader.clone(),
            TopicUnreadCursorQuery {
                category_id: Some(category),
                unread_only: true,
                ..Default::default()
            },
        )
        .await
        .expect("late approval projection should load");
    assert_eq!(before_remark.items.len(), 1);
    assert_eq!(before_remark.items[0].unread_count, 1);

    let remarked = read_state
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
        .expect("equal high-water mark should refresh the read snapshot");
    assert_eq!(remarked.last_read_position, 2);
    assert_eq!(remarked.last_read_revision, 0);

    let after_remark = projection
        .list_topics_with_unread(
            tenant_id,
            reader,
            TopicUnreadCursorQuery {
                category_id: Some(category),
                unread_only: true,
                ..Default::default()
            },
        )
        .await
        .expect("projection after repeated mark should load");
    assert!(after_remark.items.is_empty());
}
