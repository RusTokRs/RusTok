use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumModule,
    ForumTopicRouteDisposition, ForumTopicRouteService, RenameForumTopicSlugInput, TopicService,
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
        "sqlite:file:forum_topic_slug_rename_{}?mode=memory&cache=shared",
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
                name: "Slug rename".to_string(),
                slug: "slug-rename".to_string(),
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
async fn rename_records_one_alias_and_old_route_becomes_gone_after_delete() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = create_category(&db, tenant_id, admin.clone()).await?;
    let service = TopicService::new(db.clone(), event_bus.clone());
    let topic_id = service
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id,
                title: "Stable identity".to_string(),
                slug: Some("old-route".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Stable identity body"),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id;

    let short_id = ForumTopicRouteService::short_identity(topic_id);
    let renamed = service
        .rename_slug(
            tenant_id,
            topic_id,
            admin.clone(),
            RenameForumTopicSlugInput {
                locale: "en".to_string(),
                slug: " New Route ".to_string(),
            },
        )
        .await?;
    assert!(renamed.changed);
    assert_eq!(renamed.previous_slug, "old-route");
    assert_eq!(renamed.slug, "new-route");
    assert_eq!(
        renamed.previous_path,
        format!("/en/forum/t/{short_id}/old-route")
    );
    assert_eq!(
        renamed.canonical.path,
        format!("/en/forum/t/{short_id}/new-route")
    );
    let alias_id = renamed.alias_id.expect("rename alias");

    let replay = service
        .rename_slug(
            tenant_id,
            topic_id,
            admin.clone(),
            RenameForumTopicSlugInput {
                locale: "en".to_string(),
                slug: "new-route".to_string(),
            },
        )
        .await?;
    assert!(!replay.changed);
    assert_eq!(replay.alias_id, None);

    let alias_count = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS alias_count FROM forum_topic_route_aliases \
             WHERE tenant_id = ? AND topic_id = ?",
            vec![tenant_id.into(), topic_id.into()],
        ))
        .await?
        .expect("alias count")
        .try_get::<i64>("", "alias_count")?;
    assert_eq!(alias_count, 1);

    let alias = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT disposition, target_topic_id, target_locale, reason \
             FROM forum_topic_route_aliases WHERE tenant_id = ? AND alias_id = ?",
            vec![tenant_id.into(), alias_id.into()],
        ))
        .await?
        .expect("rename alias row");
    assert_eq!(alias.try_get::<String>("", "disposition")?, "redirect");
    assert_eq!(
        alias.try_get::<Option<Uuid>>("", "target_topic_id")?,
        Some(topic_id)
    );
    assert_eq!(
        alias.try_get::<Option<String>>("", "target_locale")?,
        Some("en".to_string())
    );
    assert_eq!(alias.try_get::<String>("", "reason")?, "Topic slug changed");

    let route_service = ForumTopicRouteService::new(db.clone());
    let old_active = route_service
        .resolve(tenant_id, "en", &short_id, "old-route")
        .await?;
    assert_eq!(old_active.disposition, ForumTopicRouteDisposition::Redirect);
    assert_eq!(
        old_active.canonical.expect("new canonical").slug,
        "new-route"
    );

    service.delete(tenant_id, topic_id, admin).await?;

    let old_deleted = route_service
        .resolve(tenant_id, "en", &short_id, "old-route")
        .await?;
    assert_eq!(old_deleted.disposition, ForumTopicRouteDisposition::Gone);
    assert_eq!(old_deleted.alias_id, Some(alias_id));
    assert_eq!(old_deleted.canonical, None);

    let current_deleted = route_service
        .resolve(tenant_id, "en", &short_id, "new-route")
        .await?;
    assert_eq!(
        current_deleted.disposition,
        ForumTopicRouteDisposition::Gone
    );

    Ok(())
}
