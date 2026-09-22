use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    BackfillForumTopicMergeRouteAliasesInput, CategoryService, CreateCategoryInput,
    CreateTopicInput, ForumModule, ForumTopicMergeRouteBackfillService, ForumTopicMergeService,
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
        "sqlite:file:forum_topic_merge_route_backfill_{}?mode=memory&cache=shared",
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
                name: "Historical merge routes".to_string(),
                slug: "historical-merge-routes".to_string(),
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
    title: &str,
    slug: &str,
) -> TestResult<Uuid> {
    Ok(TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".to_string(),
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

async fn merge_topic(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    security: SecurityContext,
    reason: &str,
) -> TestResult<()> {
    ForumTopicMergeService::new(db.clone(), event_bus.clone())
        .merge_topic(
            tenant_id,
            target_topic_id,
            security,
            MergeForumTopicInput {
                operation_id: Uuid::new_v4(),
                source_topic_id,
                reason: reason.to_string(),
            },
        )
        .await?;
    Ok(())
}

async fn remove_composed_aliases_for_historical_fixture(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(
        "DROP TRIGGER IF EXISTS forum_topic_route_alias_delete;\
         DELETE FROM forum_topic_route_aliases;\
         CREATE TRIGGER forum_topic_route_alias_delete \
         BEFORE DELETE ON forum_topic_route_aliases \
         BEGIN \
             SELECT RAISE(ABORT, 'forum topic route aliases are append-only'); \
         END;",
    )
    .await?;
    Ok(())
}

async fn alias_count(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    Ok(db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS alias_count FROM forum_topic_route_aliases WHERE tenant_id = ?",
            vec![tenant_id.into()],
        ))
        .await?
        .expect("alias count")
        .try_get::<i64>("", "alias_count")?)
}

#[tokio::test]
async fn historical_merge_aliases_backfill_in_bounded_replay_safe_pages() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = create_category(&db, tenant_id, admin.clone()).await?;

    let source_one = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "Historical source one",
        "historical-source-one",
    )
    .await?;
    let target_one = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "Retained target one",
        "retained-target-one",
    )
    .await?;
    let source_two = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "Historical source two",
        "historical-source-two",
    )
    .await?;
    let target_two = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "Retained target two",
        "retained-target-two",
    )
    .await?;

    merge_topic(
        &db,
        &event_bus,
        tenant_id,
        source_one,
        target_one,
        admin.clone(),
        "Historical merge one",
    )
    .await?;
    merge_topic(
        &db,
        &event_bus,
        tenant_id,
        source_two,
        target_two,
        admin.clone(),
        "Historical merge two",
    )
    .await?;
    remove_composed_aliases_for_historical_fixture(&db).await?;
    assert_eq!(alias_count(&db, tenant_id).await?, 0);

    let service = ForumTopicMergeRouteBackfillService::new(db.clone());
    let first_input = BackfillForumTopicMergeRouteAliasesInput {
        cursor: None,
        limit: 1,
    };
    let first = service
        .backfill_merge_route_aliases(tenant_id, admin.clone(), first_input.clone())
        .await?;
    assert_eq!(first.processed_operation_count, 1);
    assert_eq!(first.ensured_alias_count, 1);
    assert!(!first.exhausted);
    assert!(first.next_cursor.is_some());
    assert_eq!(alias_count(&db, tenant_id).await?, 1);

    let replay = service
        .backfill_merge_route_aliases(tenant_id, admin.clone(), first_input)
        .await?;
    assert_eq!(replay, first);
    assert_eq!(alias_count(&db, tenant_id).await?, 1);

    let second = service
        .backfill_merge_route_aliases(
            tenant_id,
            admin,
            BackfillForumTopicMergeRouteAliasesInput {
                cursor: first.next_cursor,
                limit: 1,
            },
        )
        .await?;
    assert_eq!(second.processed_operation_count, 1);
    assert_eq!(second.ensured_alias_count, 1);
    assert!(second.exhausted);
    assert!(second.next_cursor.is_none());
    assert_eq!(alias_count(&db, tenant_id).await?, 2);

    for (source_topic_id, target_topic_id, source_slug, target_slug) in [
        (
            source_one,
            target_one,
            "historical-source-one",
            "retained-target-one",
        ),
        (
            source_two,
            target_two,
            "historical-source-two",
            "retained-target-two",
        ),
    ] {
        let resolved = ForumTopicRouteService::new(db.clone())
            .resolve(
                tenant_id,
                "en",
                &ForumTopicRouteService::short_identity(source_topic_id),
                source_slug,
            )
            .await?;
        assert_eq!(resolved.disposition, ForumTopicRouteDisposition::Redirect);
        let canonical = resolved.canonical.expect("canonical redirect target");
        assert_eq!(canonical.topic_id, target_topic_id);
        assert_eq!(canonical.slug, target_slug);
    }

    Ok(())
}
