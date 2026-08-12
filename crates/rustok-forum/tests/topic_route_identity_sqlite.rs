use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumError, ForumModule,
    ForumTopicRouteDisposition, ForumTopicRouteService, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn setup() -> TestResult<(DatabaseConnection, TransactionalEventBus)> {
    let db_url = format!(
        "sqlite:file:forum_topic_route_identity_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    db.execute_unprepared(
        "CREATE TABLE users (\
            id TEXT NOT NULL PRIMARY KEY, \
            tenant_id TEXT NOT NULL, \
            UNIQUE (tenant_id, id)\
        )",
    )
    .await?;
    let schema = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration.up(&schema).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&schema).await?;
    }
    for migration in ForumModule.migrations() {
        migration.up(&schema).await?;
    }
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    Ok((db, event_bus))
}

async fn insert_user(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) -> TestResult<()> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
        vec![user_id.into(), tenant_id.into()],
    ))
    .await?;
    Ok(())
}

async fn create_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    security: SecurityContext,
) -> TestResult<Uuid> {
    Ok(CategoryService::new(db.clone())
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Route identity".to_string(),
                slug: "route-identity".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?
        .id)
}

async fn create_topic(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    category_id: Uuid,
    security: SecurityContext,
    slug: &str,
) -> TestResult<Uuid> {
    Ok(TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id,
                title: format!("Route {slug}"),
                slug: Some(slug.to_string()),
                body: rustok_api::RichTextDocument::single_paragraph(format!("Route {slug} body")),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id)
}

async fn insert_alias(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    alias_id: Uuid,
    topic_id: Uuid,
    slug: &str,
    disposition: &str,
    target_topic_id: Option<Uuid>,
) -> TestResult<()> {
    let short_id = ForumTopicRouteService::short_identity(topic_id);
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO forum_topic_route_aliases (\
            tenant_id, alias_id, topic_id, locale, short_id, slug, disposition, \
            target_topic_id, target_locale, reason, created_at\
        ) VALUES (?, ?, ?, 'en', ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            alias_id.into(),
            topic_id.into(),
            short_id.into(),
            slug.into(),
            disposition.into(),
            target_topic_id.into(),
            target_topic_id.map(|_| "en".to_string()).into(),
            "Route identity fixture".into(),
        ],
    ))
    .await?;
    Ok(())
}

#[tokio::test]
async fn current_alias_and_tombstone_routes_resolve_without_slug_identity() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = create_category(&db, tenant_id, admin.clone()).await?;
    let source_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "source-route",
    )
    .await?;
    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "target-route",
    )
    .await?;

    let service = ForumTopicRouteService::new(db.clone());
    let target = service
        .canonical_descriptor(tenant_id, target_topic_id, " EN ")
        .await?;
    assert_eq!(target.topic_id, target_topic_id);
    assert_eq!(target.locale, "en");
    assert!(target.path.ends_with("/target-route"));

    let wrong_slug = service
        .resolve(tenant_id, "en", &target.short_id, "old-target-slug")
        .await?;
    assert_eq!(wrong_slug.disposition, ForumTopicRouteDisposition::Redirect);
    assert_eq!(wrong_slug.canonical.as_ref(), Some(&target));
    assert_eq!(wrong_slug.alias_id, None);

    TopicService::new(db.clone(), event_bus.clone())
        .delete(tenant_id, source_topic_id, admin)
        .await?;

    let redirect_alias_id = Uuid::new_v4();
    insert_alias(
        &db,
        tenant_id,
        redirect_alias_id,
        source_topic_id,
        "legacy-source",
        "redirect",
        Some(target_topic_id),
    )
    .await?;
    let redirected = service
        .resolve(
            tenant_id,
            "en",
            &ForumTopicRouteService::short_identity(source_topic_id),
            "legacy-source",
        )
        .await?;
    assert_eq!(redirected.disposition, ForumTopicRouteDisposition::Redirect);
    assert_eq!(redirected.alias_id, Some(redirect_alias_id));
    assert_eq!(
        redirected.canonical.as_ref().map(|route| route.topic_id),
        Some(target_topic_id)
    );

    let gone_alias_id = Uuid::new_v4();
    insert_alias(
        &db,
        tenant_id,
        gone_alias_id,
        source_topic_id,
        "removed-source",
        "gone",
        None,
    )
    .await?;
    let gone = service
        .resolve(
            tenant_id,
            "en",
            &ForumTopicRouteService::short_identity(source_topic_id),
            "removed-source",
        )
        .await?;
    assert_eq!(gone.disposition, ForumTopicRouteDisposition::Gone);
    assert_eq!(gone.alias_id, Some(gone_alias_id));
    assert!(gone.canonical.is_none());

    let missing = service
        .resolve(tenant_id, "en", "000000000000", "missing")
        .await
        .expect_err("unknown route must not resolve");
    assert!(matches!(missing, ForumError::TopicRouteNotFound));

    Ok(())
}
