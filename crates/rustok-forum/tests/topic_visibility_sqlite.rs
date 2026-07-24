use std::sync::Arc;

use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumModule,
    ForumTopicVisibilityScope, ForumTopicVisibilityService, ListTopicsFilter, ModerationService,
    TopicService,
};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let db_url = format!(
        "sqlite:file:forum_topic_visibility_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum topic visibility sqlite database should connect");
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
    slug: &str,
    channel_slugs: Option<Vec<String>>,
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
                body: "Visibility fixture".into(),
                body_format: "markdown".into(),
                content_json: None,
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs,
            },
        )
        .await
        .expect("topic should be created")
        .id
}

fn topic_filter(category_id: Uuid) -> ListTopicsFilter {
    ListTopicsFilter {
        category_id: Some(category_id),
        status: None,
        locale: Some("en".into()),
        page: 1,
        per_page: 20,
    }
}

#[tokio::test]
async fn exact_visibility_scope_is_bounded_ordered_and_non_oracular() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let category_id = create_category(&db, tenant_id, admin.clone(), "visibility").await;
    let foreign_category_id =
        create_category(&db, foreign_tenant_id, admin.clone(), "foreign-visibility").await;

    let public_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "public-topic",
        None,
    )
    .await;
    let mobile_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "mobile-topic",
        Some(vec!["MOBILE".into()]),
    )
    .await;
    let web_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "web-topic",
        Some(vec!["web".into()]),
    )
    .await;
    let closed_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "closed-topic",
        None,
    )
    .await;
    ModerationService::new(db.clone(), event_bus.clone())
        .close_topic(tenant_id, closed_topic, admin.clone())
        .await
        .expect("topic should close");
    let foreign_topic = create_topic(
        &db,
        &event_bus,
        foreign_tenant_id,
        foreign_category_id,
        admin,
        "foreign-topic",
        None,
    )
    .await;

    let missing_topic = Uuid::new_v4();
    let service = ForumTopicVisibilityService::new(db);
    let public_scope = ForumTopicVisibilityScope::storefront(None).expect("public scope");
    let public_ids = service
        .filter_visible_topic_ids(
            tenant_id,
            &[
                mobile_topic,
                public_topic,
                public_topic,
                closed_topic,
                foreign_topic,
                missing_topic,
                web_topic,
            ],
            &public_scope,
        )
        .await
        .expect("public exact scope should resolve");
    assert_eq!(public_ids, vec![public_topic]);

    let mobile_scope =
        ForumTopicVisibilityScope::storefront(Some("  MOBILE  ")).expect("mobile scope");
    let mobile_ids = service
        .filter_visible_topic_ids(
            tenant_id,
            &[mobile_topic, public_topic, web_topic, closed_topic],
            &mobile_scope,
        )
        .await
        .expect("mobile exact scope should resolve");
    assert_eq!(mobile_ids, vec![mobile_topic, public_topic]);
    assert!(
        service
            .is_topic_visible(tenant_id, mobile_topic, &mobile_scope)
            .await
            .expect("mobile visibility should resolve")
    );
    for unavailable in [web_topic, closed_topic, foreign_topic, missing_topic] {
        assert!(
            !service
                .is_topic_visible(tenant_id, unavailable, &mobile_scope)
                .await
                .expect("unavailable target should resolve as absent")
        );
    }

    let oversized = vec![public_topic; 101];
    let error = service
        .filter_visible_topic_ids(tenant_id, &oversized, &public_scope)
        .await
        .expect_err("raw candidate input above the owner bound should fail");
    assert!(error.to_string().contains("must not exceed 100"));
    assert!(ForumTopicVisibilityScope::storefront(Some("not/a/channel")).is_err());
    assert!(ForumTopicVisibilityScope::storefront(Some(&"x".repeat(129))).is_err());
}

#[tokio::test]
async fn storefront_topic_facade_is_guarded_by_the_exact_owner_scope() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let reader = SecurityContext::public_read();
    let category_id = create_category(&db, tenant_id, admin.clone(), "facade-visibility").await;
    let public_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "facade-public",
        None,
    )
    .await;
    let mobile_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin,
        "facade-mobile",
        Some(vec!["mobile".into()]),
    )
    .await;

    let topic_service = TopicService::new(db.clone(), event_bus);
    let (public_page, _) = topic_service
        .list_storefront_visible_with_locale_fallback(
            tenant_id,
            reader.clone(),
            topic_filter(category_id),
            Some("en"),
            None,
        )
        .await
        .expect("public storefront page should resolve");
    assert_eq!(
        public_page.iter().map(|topic| topic.id).collect::<Vec<_>>(),
        vec![public_topic]
    );

    let (mobile_page, _) = topic_service
        .list_storefront_visible_with_locale_fallback(
            tenant_id,
            reader.clone(),
            topic_filter(category_id),
            Some("en"),
            Some("mobile"),
        )
        .await
        .expect("mobile storefront page should resolve");
    let mobile_ids = mobile_page
        .iter()
        .map(|topic| topic.id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(mobile_ids.len(), 2);
    assert!(mobile_ids.contains(&public_topic));
    assert!(mobile_ids.contains(&mobile_topic));

    assert!(
        topic_service
            .get_storefront_visible_with_locale_fallback(
                tenant_id,
                reader.clone(),
                mobile_topic,
                "en",
                Some("en"),
                None,
            )
            .await
            .expect("public target check should resolve")
            .is_none()
    );
    assert!(
        topic_service
            .get_storefront_visible_with_locale_fallback(
                tenant_id,
                reader,
                mobile_topic,
                "en",
                Some("en"),
                Some("mobile"),
            )
            .await
            .expect("matching channel target should resolve")
            .is_some()
    );
}
