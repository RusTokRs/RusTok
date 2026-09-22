use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumModule, ForumTopicMergeService,
    ForumTopicRouteDisposition, ForumTopicRouteService, MergeForumTopicInput, TopicService,
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
        "sqlite:file:forum_topic_merge_route_alias_{}?mode=memory&cache=shared",
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
                name: "Merge route aliases".to_string(),
                slug: "merge-route-aliases".to_string(),
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
    locale: &str,
    title: &str,
    slug: &str,
) -> TestResult<Uuid> {
    Ok(TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: locale.to_string(),
                category_id,
                title: title.to_string(),
                slug: Some(slug.to_string()),
                body: rustok_api::RichTextDocument::single_paragraph(format!("{title} body")),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id)
}

#[tokio::test]
async fn merge_persists_one_redirect_alias_and_replay_does_not_duplicate_it() -> TestResult<()> {
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
        "ru",
        "Исходная тема",
        "source-route",
    )
    .await?;
    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "en",
        "Retained topic",
        "target-route",
    )
    .await?;

    let operation_id = Uuid::new_v4();
    let reason = "Consolidate one localized duplicate topic";
    let input = MergeForumTopicInput {
        operation_id,
        source_topic_id,
        reason: reason.to_string(),
    };
    let merge_service = ForumTopicMergeService::new(db.clone(), event_bus.clone());
    let first = merge_service
        .merge_topic(tenant_id, target_topic_id, admin.clone(), input.clone())
        .await?;
    let replay = merge_service
        .merge_topic(tenant_id, target_topic_id, admin.clone(), input)
        .await?;
    assert_eq!(replay, first);

    let alias = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT alias_id, locale, slug, disposition, target_topic_id, target_locale, reason \
             FROM forum_topic_route_aliases \
             WHERE tenant_id = ? AND topic_id = ?",
            vec![tenant_id.into(), source_topic_id.into()],
        ))
        .await?
        .expect("merge route alias");
    let alias_id: Uuid = alias.try_get("", "alias_id")?;
    assert_eq!(alias.try_get::<String>("", "locale")?, "ru");
    assert_eq!(alias.try_get::<String>("", "slug")?, "source-route");
    assert_eq!(alias.try_get::<String>("", "disposition")?, "redirect");
    assert_eq!(
        alias.try_get::<Option<Uuid>>("", "target_topic_id")?,
        Some(target_topic_id)
    );
    assert_eq!(
        alias.try_get::<Option<String>>("", "target_locale")?,
        Some("en".to_string())
    );
    assert_eq!(alias.try_get::<String>("", "reason")?, reason);

    let count = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS alias_count FROM forum_topic_route_aliases \
             WHERE tenant_id = ? AND topic_id = ?",
            vec![tenant_id.into(), source_topic_id.into()],
        ))
        .await?
        .expect("alias count")
        .try_get::<i64>("", "alias_count")?;
    assert_eq!(count, 1);

    TopicService::new(db.clone(), event_bus)
        .delete(tenant_id, source_topic_id, admin)
        .await?;

    let resolved = ForumTopicRouteService::new(db)
        .resolve(
            tenant_id,
            "ru",
            &ForumTopicRouteService::short_identity(source_topic_id),
            "source-route",
        )
        .await?;
    assert_eq!(resolved.disposition, ForumTopicRouteDisposition::Redirect);
    assert_eq!(resolved.alias_id, Some(alias_id));
    let canonical = resolved.canonical.expect("canonical redirect target");
    assert_eq!(canonical.topic_id, target_topic_id);
    assert_eq!(canonical.locale, "en");
    assert_eq!(canonical.slug, "target-route");

    Ok(())
}
