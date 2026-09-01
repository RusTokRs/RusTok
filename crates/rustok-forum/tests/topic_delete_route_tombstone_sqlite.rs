use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumModule,
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
        "sqlite:file:forum_topic_delete_route_tombstone_{}?mode=memory&cache=shared",
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
    db.execute_raw(Statement::from_sql_and_values(
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
                name: "Delete route tombstones".to_string(),
                slug: "delete-route-tombstones".to_string(),
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

#[tokio::test]
async fn delete_records_gone_route_before_soft_delete() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = create_category(&db, tenant_id, admin.clone()).await?;
    let topic_id = TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id,
                title: "Removed route".to_string(),
                slug: Some("removed-route".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Removed route body"),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id;

    let route_service = ForumTopicRouteService::new(db.clone());
    let canonical = route_service
        .canonical_descriptor(tenant_id, topic_id, "en")
        .await?;
    let short_id = canonical.short_id.clone();

    TopicService::new(db.clone(), event_bus)
        .delete(tenant_id, topic_id, admin)
        .await?;

    let alias = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT alias_id, locale, short_id, slug, disposition, target_topic_id, \
                    target_locale, reason \
             FROM forum_topic_route_aliases \
             WHERE tenant_id = ? AND topic_id = ?",
            vec![tenant_id.into(), topic_id.into()],
        ))
        .await?
        .expect("delete route tombstone");
    let alias_id: Uuid = alias.try_get("", "alias_id")?;
    assert_eq!(alias.try_get::<String>("", "locale")?, "en");
    assert_eq!(alias.try_get::<String>("", "short_id")?, short_id);
    assert_eq!(alias.try_get::<String>("", "slug")?, "removed-route");
    assert_eq!(alias.try_get::<String>("", "disposition")?, "gone");
    assert_eq!(alias.try_get::<Option<Uuid>>("", "target_topic_id")?, None);
    assert_eq!(alias.try_get::<Option<String>>("", "target_locale")?, None);
    assert_eq!(alias.try_get::<String>("", "reason")?, "Topic deleted");

    let resolved = ForumTopicRouteService::new(db)
        .resolve(tenant_id, "en", &canonical.short_id, "removed-route")
        .await?;
    assert_eq!(resolved.disposition, ForumTopicRouteDisposition::Gone);
    assert_eq!(resolved.requested_topic_id, Some(topic_id));
    assert_eq!(resolved.canonical, None);
    assert_eq!(resolved.alias_id, Some(alias_id));

    Ok(())
}
