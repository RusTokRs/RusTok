use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_forum::entities::forum_topic_revision;
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
    ForumReadModelService, ForumTopicReadStateService, ForumModule, MarkForumTopicReadInput,
    ModerationService, ReplyService, TopicService, TopicUnreadCursorQuery, UpdateTopicInput,
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
        "sqlite:file:forum_topic_unread_projection_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum topic unread sqlite database should connect");
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

async fn create_topic(
    topics: &TopicService,
    tenant_id: Uuid,
    category_id: Uuid,
    author: SecurityContext,
    title: &str,
) -> Uuid {
    topics
        .create(
            tenant_id,
            author,
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

#[tokio::test]
async fn unread_projection_is_bounded_visibility_aware_and_cursor_correct() {
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
                name: "Unread projection".into(),
                slug: "unread-projection".into(),
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

    let topics = TopicService::new(db.clone(), event_bus.clone());
    let replies = ReplyService::new(db.clone(), event_bus.clone());
    let moderation = ModerationService::new(db.clone(), event_bus);
    let read_state = ForumTopicReadStateService::new(db.clone());
    let projection = ForumReadModelService::new(db.clone());

    let unseen_topic = create_topic(
        &topics,
        tenant_id,
        category.id,
        author.clone(),
        "Unseen topic",
    )
    .await;

    let reply_topic = create_topic(
        &topics,
        tenant_id,
        category.id,
        author.clone(),
        "Reply topic",
    )
    .await;
    replies
        .create(
            tenant_id,
            reader.clone(),
            reply_topic,
            CreateReplyInput {
                locale: "en".into(),
                content: "First approved reply".into(),
                content_format: "markdown".into(),
                content_json: None,
                parent_reply_id: None,
            },
        )
        .await
        .expect("first reply should be created");
    let second_reply = replies
        .create(
            tenant_id,
            reader.clone(),
            reply_topic,
            CreateReplyInput {
                locale: "en".into(),
                content: "Second approved reply".into(),
                content_format: "markdown".into(),
                content_json: None,
                parent_reply_id: None,
            },
        )
        .await
        .expect("second reply should be created");
    read_state
        .mark_topic_read(
            tenant_id,
            reply_topic,
            reader.clone(),
            MarkForumTopicReadInput {
                last_read_position: 1,
                last_read_revision: 0,
            },
        )
        .await
        .expect("reader should persist the first reply position");

    let revision_topic = create_topic(
        &topics,
        tenant_id,
        category.id,
        author.clone(),
        "Revision topic",
    )
    .await;
    replies
        .create(
            tenant_id,
            reader.clone(),
            revision_topic,
            CreateReplyInput {
                locale: "en".into(),
                content: "Read before edit".into(),
                content_format: "markdown".into(),
                content_json: None,
                parent_reply_id: None,
            },
        )
        .await
        .expect("revision topic reply should be created");
    read_state
        .mark_topic_read(
            tenant_id,
            revision_topic,
            reader.clone(),
            MarkForumTopicReadInput {
                last_read_position: 1,
                last_read_revision: 0,
            },
        )
        .await
        .expect("revision topic should initially be fully read");
    topics
        .update(
            tenant_id,
            revision_topic,
            author.clone(),
            UpdateTopicInput {
                locale: "en".into(),
                title: Some("Revision topic updated".into()),
                body: Some("Updated after the reader opened the topic".into()),
                body_format: Some("markdown".into()),
                content_json: None,
                metadata: None,
                tags: None,
                channel_slugs: None,
            },
        )
        .await
        .expect("topic edit should create an immutable revision");
    let latest_revision = forum_topic_revision::Entity::find()
        .filter(forum_topic_revision::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_revision::Column::TopicId.eq(revision_topic))
        .order_by_desc(forum_topic_revision::Column::Id)
        .one(&db)
        .await
        .expect("revision query should succeed")
        .expect("topic edit should record a revision")
        .id;

    let all = projection
        .list_topics_with_unread(
            tenant_id,
            reader.clone(),
            TopicUnreadCursorQuery {
                category_id: Some(category.id),
                limit: Some(20),
                ..Default::default()
            },
        )
        .await
        .expect("unread projection should load");
    let by_id = all
        .items
        .iter()
        .map(|item| (item.topic.id, item))
        .collect::<HashMap<_, _>>();

    let unseen = by_id.get(&unseen_topic).expect("unseen topic should project");
    assert!(!unseen.read_state_explicit);
    assert_eq!(unseen.unread_count, 0);
    assert!(!unseen.has_unread_topic_revision);
    assert!(unseen.is_unread, "an unseen topic is unread even without replies");

    let reply_unread = by_id.get(&reply_topic).expect("reply topic should project");
    assert!(reply_unread.read_state_explicit);
    assert_eq!(reply_unread.last_read_position, 1);
    assert_eq!(reply_unread.unread_count, 1);
    assert!(!reply_unread.has_unread_topic_revision);
    assert!(reply_unread.is_unread);

    let revision_unread = by_id
        .get(&revision_topic)
        .expect("revision topic should project");
    assert!(revision_unread.read_state_explicit);
    assert_eq!(revision_unread.unread_count, 0);
    assert!(revision_unread.has_unread_topic_revision);
    assert!(revision_unread.is_unread);

    moderation
        .hide_reply(
            tenant_id,
            second_reply.id,
            reply_topic,
            author.clone(),
        )
        .await
        .expect("moderator should hide the unread reply");
    let after_hide = projection
        .list_topics_with_unread(
            tenant_id,
            reader.clone(),
            TopicUnreadCursorQuery {
                category_id: Some(category.id),
                limit: Some(20),
                ..Default::default()
            },
        )
        .await
        .expect("projection after hide should load");
    let hidden_summary = after_hide
        .items
        .iter()
        .find(|item| item.topic.id == reply_topic)
        .expect("reply topic should remain visible");
    assert_eq!(hidden_summary.unread_count, 0);
    assert!(!hidden_summary.is_unread, "hidden replies must not inflate unread state");

    let first_page = projection
        .list_topics_with_unread(
            tenant_id,
            reader.clone(),
            TopicUnreadCursorQuery {
                category_id: Some(category.id),
                limit: Some(1),
                unread_only: true,
                ..Default::default()
            },
        )
        .await
        .expect("first unread page should load");
    assert_eq!(first_page.items.len(), 1);
    assert!(first_page.has_more);
    let second_page = projection
        .list_topics_with_unread(
            tenant_id,
            reader.clone(),
            TopicUnreadCursorQuery {
                cursor: first_page.next_cursor.clone(),
                category_id: Some(category.id),
                limit: Some(1),
                unread_only: true,
                ..Default::default()
            },
        )
        .await
        .expect("second unread page should load");
    assert_eq!(second_page.items.len(), 1);
    assert!(!second_page.has_more);
    let unread_ids = first_page
        .items
        .iter()
        .chain(second_page.items.iter())
        .map(|item| item.topic.id)
        .collect::<HashSet<_>>();
    assert_eq!(unread_ids, HashSet::from([unseen_topic, revision_topic]));

    read_state
        .mark_topic_read(
            tenant_id,
            unseen_topic,
            reader.clone(),
            MarkForumTopicReadInput {
                last_read_position: 0,
                last_read_revision: 0,
            },
        )
        .await
        .expect("opening an empty topic should persist explicit zero state");
    read_state
        .mark_topic_read(
            tenant_id,
            revision_topic,
            reader.clone(),
            MarkForumTopicReadInput {
                last_read_position: 1,
                last_read_revision: latest_revision,
            },
        )
        .await
        .expect("reader should advance to the latest topic revision");
    let empty = projection
        .list_topics_with_unread(
            tenant_id,
            reader.clone(),
            TopicUnreadCursorQuery {
                category_id: Some(category.id),
                unread_only: true,
                ..Default::default()
            },
        )
        .await
        .expect("fully read projection should load");
    assert!(empty.items.is_empty());
    assert!(!empty.has_more);
    assert_eq!(empty.next_cursor, None);

    let anonymous_result = projection
        .list_topics_with_unread(
            tenant_id,
            anonymous,
            TopicUnreadCursorQuery {
                category_id: Some(category.id),
                ..Default::default()
            },
        )
        .await;
    assert!(
        anonymous_result.is_err(),
        "personal unread projection must require an authenticated user"
    );
}
