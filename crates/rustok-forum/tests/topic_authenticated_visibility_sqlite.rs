use std::collections::HashSet;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumCategoryVisibility,
    ForumCategoryVisibilityPolicyService, ForumModule, ForumTopicVisibilityScope,
    ForumTopicVisibilityService, ListTopicsFilter, SetForumCategoryVisibilityPolicyInput,
    TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectionTrait,ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let db_url = format!(
        "sqlite:file:forum_topic_authenticated_visibility_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum authenticated visibility sqlite database should connect");
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
                body: rustok_api::RichTextDocument::single_paragraph(
                    "Authenticated visibility fixture",
                ),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created")
        .id
}

fn topic_filter(category_id: Option<Uuid>) -> ListTopicsFilter {
    ListTopicsFilter {
        category_id,
        status: None,
        locale: Some("en".into()),
        page: 1,
        per_page: 20,
    }
}

#[tokio::test]
async fn inherited_authenticated_categories_filter_before_storefront_pagination() {
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

    let visibility = ForumTopicVisibilityService::new(db.clone());
    let public_scope = ForumTopicVisibilityScope::storefront(None).expect("public scope");
    let authenticated_scope =
        ForumTopicVisibilityScope::storefront_for_viewer(None, true).expect("authenticated scope");
    assert_eq!(
        visibility
            .filter_visible_topic_ids(tenant_id, &[restricted_topic, public_topic], &public_scope,)
            .await
            .expect("public exact visibility should resolve"),
        vec![public_topic]
    );
    assert_eq!(
        visibility
            .filter_visible_topic_ids(
                tenant_id,
                &[restricted_topic, public_topic],
                &authenticated_scope,
            )
            .await
            .expect("authenticated exact visibility should resolve"),
        vec![restricted_topic, public_topic]
    );

    let topics = TopicService::new(db.clone(), event_bus);
    let (public_page, public_total) = topics
        .list_storefront_visible_with_locale_fallback(
            tenant_id,
            public.clone(),
            topic_filter(None),
            Some("en"),
            None,
        )
        .await
        .expect("public storefront list should resolve");
    assert_eq!(public_total, 1);
    assert_eq!(
        public_page
            .iter()
            .map(|topic| topic.id)
            .collect::<HashSet<_>>(),
        HashSet::from([public_topic])
    );

    let (authenticated_page, authenticated_total) = topics
        .list_storefront_visible_with_locale_fallback(
            tenant_id,
            authenticated.clone(),
            topic_filter(None),
            Some("en"),
            None,
        )
        .await
        .expect("authenticated storefront list should resolve");
    assert_eq!(authenticated_total, 2);
    assert_eq!(
        authenticated_page
            .iter()
            .map(|topic| topic.id)
            .collect::<HashSet<_>>(),
        HashSet::from([public_topic, restricted_topic])
    );

    let (public_restricted_page, public_restricted_total) = topics
        .list_storefront_visible_with_locale_fallback(
            tenant_id,
            public.clone(),
            topic_filter(Some(restricted_child)),
            Some("en"),
            None,
        )
        .await
        .expect("public restricted-category page should resolve as empty");
    assert!(public_restricted_page.is_empty());
    assert_eq!(public_restricted_total, 0);

    let (authenticated_restricted_page, authenticated_restricted_total) = topics
        .list_storefront_visible_with_locale_fallback(
            tenant_id,
            authenticated.clone(),
            topic_filter(Some(restricted_child)),
            Some("en"),
            None,
        )
        .await
        .expect("authenticated restricted-category page should resolve");
    assert_eq!(authenticated_restricted_total, 1);
    assert_eq!(authenticated_restricted_page[0].id, restricted_topic);

    assert!(
        topics
            .get_storefront_visible_with_locale_fallback(
                tenant_id,
                public,
                restricted_topic,
                "en",
                Some("en"),
                None,
            )
            .await
            .expect("public restricted topic lookup should resolve")
            .is_none()
    );
    assert!(
        topics
            .get_storefront_visible_with_locale_fallback(
                tenant_id,
                authenticated,
                restricted_topic,
                "en",
                Some("en"),
                None,
            )
            .await
            .expect("authenticated restricted topic lookup should resolve")
            .is_some()
    );
}
