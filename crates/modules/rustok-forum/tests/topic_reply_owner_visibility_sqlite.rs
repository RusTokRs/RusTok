use std::collections::HashSet;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
    ForumCategoryVisibility, ForumCategoryVisibilityPolicyService, ForumError, ForumModule,
    ListRepliesFilter, ListTopicsFilter, ReplyService, SetForumCategoryVisibilityPolicyInput,
    TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let db_url = format!(
        "sqlite:file:forum_topic_reply_owner_visibility_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum owner visibility sqlite database should connect");
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
    parent_id: Option<Uuid>,
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
                parent_id,
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
    slug: &str,
) -> Uuid {
    TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".into(),
                category_id,
                title: slug.replace('-', " "),
                slug: Some(slug.into()),
                body: rustok_api::RichTextDocument::single_paragraph("Owner visibility fixture"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created")
        .id
}

async fn create_reply(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    topic_id: Uuid,
    security: SecurityContext,
    content: &str,
) -> Uuid {
    ReplyService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            topic_id,
            CreateReplyInput {
                locale: "en".into(),
                content: rustok_api::RichTextDocument::single_paragraph(content),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created")
        .id
}

fn topic_filter() -> ListTopicsFilter {
    ListTopicsFilter {
        category_id: None,
        status: None,
        locale: Some("en".into()),
        page: 1,
        per_page: 20,
    }
}

fn reply_filter() -> ListRepliesFilter {
    ListRepliesFilter {
        locale: Some("en".into()),
        page: 1,
        per_page: 20,
    }
}

#[tokio::test]
async fn inherited_authenticated_floor_guards_topic_and_reply_owner_reads() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let authenticated = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let public = SecurityContext::public_read();

    let public_category = create_category(&db, tenant_id, admin.clone(), "public", None).await;
    let restricted_parent = create_category(&db, tenant_id, admin.clone(), "members", None).await;
    let restricted_child = create_category(
        &db,
        tenant_id,
        admin.clone(),
        "members-child",
        Some(restricted_parent),
    )
    .await;

    let public_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        public_category,
        admin.clone(),
        "public-topic",
    )
    .await;
    let restricted_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        restricted_child,
        admin.clone(),
        "members-topic",
    )
    .await;
    let public_reply = create_reply(
        &db,
        &event_bus,
        tenant_id,
        public_topic,
        admin.clone(),
        "Public reply",
    )
    .await;
    let restricted_reply = create_reply(
        &db,
        &event_bus,
        tenant_id,
        restricted_topic,
        admin.clone(),
        "Members reply",
    )
    .await;

    ForumCategoryVisibilityPolicyService::new(db.clone())
        .set(
            tenant_id,
            restricted_parent,
            admin,
            SetForumCategoryVisibilityPolicyInput {
                visibility: ForumCategoryVisibility::Authenticated,
            },
        )
        .await
        .expect("parent category should narrow to authenticated viewers");

    let topics = TopicService::new(db.clone(), event_bus.clone());
    let (public_topics, public_total) = topics
        .list_with_locale_fallback(tenant_id, public.clone(), topic_filter(), Some("en"))
        .await
        .expect("public owner topic page should resolve");
    assert_eq!(public_total, 1);
    assert_eq!(
        public_topics
            .iter()
            .map(|topic| topic.id)
            .collect::<HashSet<_>>(),
        HashSet::from([public_topic])
    );

    let (authenticated_topics, authenticated_total) = topics
        .list_with_locale_fallback(tenant_id, authenticated.clone(), topic_filter(), Some("en"))
        .await
        .expect("authenticated owner topic page should resolve");
    assert_eq!(authenticated_total, 2);
    assert_eq!(
        authenticated_topics
            .iter()
            .map(|topic| topic.id)
            .collect::<HashSet<_>>(),
        HashSet::from([public_topic, restricted_topic])
    );

    assert!(
        matches!(
            topics
                .get_with_locale_fallback(
                    tenant_id,
                    public.clone(),
                    restricted_topic,
                    "en",
                    Some("en"),
                )
                .await,
            Err(ForumError::TopicNotFound(id)) if id == restricted_topic
        ),
        "public exact topic read must hide an inherited authenticated category"
    );
    assert_eq!(
        topics
            .get_with_locale_fallback(
                tenant_id,
                authenticated.clone(),
                restricted_topic,
                "en",
                Some("en"),
            )
            .await
            .expect("authenticated exact topic read should resolve")
            .id,
        restricted_topic
    );

    let replies = ReplyService::new(db, event_bus);
    assert_eq!(
        replies
            .get_with_locale_fallback(tenant_id, public.clone(), public_reply, "en", Some("en"),)
            .await
            .expect("public reply in public category should resolve")
            .id,
        public_reply
    );
    assert!(
        matches!(
            replies
                .get_with_locale_fallback(
                    tenant_id,
                    public.clone(),
                    restricted_reply,
                    "en",
                    Some("en"),
                )
                .await,
            Err(ForumError::ReplyNotFound(id)) if id == restricted_reply
        ),
        "public exact reply read must not expose the hidden parent topic"
    );
    assert!(
        matches!(
            replies
                .list_response_for_topic_with_locale_fallback(
                    tenant_id,
                    public,
                    restricted_topic,
                    reply_filter(),
                    Some("en"),
                )
                .await,
            Err(ForumError::TopicNotFound(id)) if id == restricted_topic
        ),
        "public reply page must fail as an absent hidden topic before pagination"
    );

    let (authenticated_replies, authenticated_reply_total) = replies
        .list_response_for_topic_with_locale_fallback(
            tenant_id,
            authenticated.clone(),
            restricted_topic,
            reply_filter(),
            Some("en"),
        )
        .await
        .expect("authenticated reply page should resolve");
    assert_eq!(authenticated_reply_total, 1);
    assert_eq!(authenticated_replies[0].id, restricted_reply);
    assert_eq!(
        replies
            .get_with_locale_fallback(tenant_id, authenticated, restricted_reply, "en", Some("en"),)
            .await
            .expect("authenticated exact reply read should resolve")
            .id,
        restricted_reply
    );
}
