use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::services::{ForumTopicSplitService, SplitForumTopicRepliesInput};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumModule,
    ReplyService, TopicService,
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
        "sqlite:file:forum_topic_split_{}?mode=memory&cache=shared",
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
                name: "Topic split".to_string(),
                slug: "topic-split".to_string(),
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
) -> TestResult<Uuid> {
    Ok(TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id,
                title: "Source discussion".to_string(),
                slug: Some("source-discussion".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Source discussion"),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id)
}

async fn create_reply(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    topic_id: Uuid,
    security: SecurityContext,
    body: &str,
    parent_reply_id: Option<Uuid>,
) -> TestResult<Uuid> {
    Ok(ReplyService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            topic_id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph(body),
                parent_reply_id,
            },
        )
        .await?
        .id)
}

async fn scalar_i64(
    db: &DatabaseConnection,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> TestResult<i64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            sql,
            values,
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("expected scalar row"))?;
    Ok(row.try_get("", "value")?)
}

async fn topic_reply_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<i64> {
    scalar_i64(
        db,
        "SELECT reply_count AS value FROM forum_topics WHERE tenant_id = ? AND id = ?",
        vec![tenant_id.into(), topic_id.into()],
    )
    .await
}

async fn reply_topic_and_position(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> TestResult<(Uuid, i64, Option<Uuid>)> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT topic_id, position, parent_reply_id FROM forum_replies WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), reply_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("reply row missing"))?;
    Ok((
        row.try_get("", "topic_id")?,
        row.try_get("", "position")?,
        row.try_get("", "parent_reply_id")?,
    ))
}

#[tokio::test]
async fn selected_reply_split_is_atomic_idempotent_and_append_only() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = create_category(&db, tenant_id, admin.clone()).await?;
    let source_topic_id =
        create_topic(&db, &event_bus, tenant_id, category_id, admin.clone()).await?;
    let remaining_reply_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Keep in source",
        None,
    )
    .await?;
    let selected_root_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Move root",
        None,
    )
    .await?;
    let selected_child_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Move child",
        Some(selected_root_id),
    )
    .await?;

    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO forum_solutions (topic_id, tenant_id, reply_id, marked_by_user_id, marked_at) VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)",
        vec![
            source_topic_id.into(),
            tenant_id.into(),
            selected_child_id.into(),
            actor_id.into(),
        ],
    ))
    .await?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO forum_topic_channel_access (tenant_id, topic_id, channel_slug) VALUES (?, ?, ?)",
        vec![tenant_id.into(), source_topic_id.into(), "support".into()],
    ))
    .await?;

    let category_before = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT topic_count, reply_count FROM forum_categories WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), category_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("category missing"))?;
    let topic_count_before: i64 = category_before.try_get("", "topic_count")?;
    let reply_count_before: i64 = category_before.try_get("", "reply_count")?;
    let body_rows_before = scalar_i64(
        &db,
        "SELECT COUNT(*) AS value FROM forum_reply_bodies WHERE tenant_id = ? AND reply_id IN (?, ?)",
        vec![
            tenant_id.into(),
            selected_root_id.into(),
            selected_child_id.into(),
        ],
    )
    .await?;

    let target_topic_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let input = SplitForumTopicRepliesInput {
        operation_id,
        target_topic_id,
        reply_ids: vec![selected_child_id, selected_root_id],
        locale: "en".to_string(),
        title: "Extracted branch".to_string(),
        slug: Some("Extracted branch".to_string()),
        reason: "Separate an independent support branch".to_string(),
    };
    let service = ForumTopicSplitService::new(db.clone(), event_bus.clone());
    let first = service
        .split_selected_replies(tenant_id, source_topic_id, admin.clone(), input.clone())
        .await?;
    let replay = service
        .split_selected_replies(tenant_id, source_topic_id, admin.clone(), input.clone())
        .await?;

    assert_eq!(first, replay);
    assert_eq!(first.operation_id, operation_id);
    assert_eq!(first.event_id, operation_id);
    assert_eq!(first.source_topic_id, source_topic_id);
    assert_eq!(first.target_topic_id, target_topic_id);
    assert_eq!(first.category_id, category_id);
    assert_eq!(first.actor_id, actor_id);
    assert_eq!(first.moved_reply_count, 2);
    assert_eq!(first.moved_published_reply_count, 2);
    assert_eq!(first.source_resulting_published_reply_count, 1);
    assert_eq!(first.target_resulting_published_reply_count, 2);
    assert_eq!(first.solution_reply_id, Some(selected_child_id));

    assert_eq!(topic_reply_count(&db, tenant_id, source_topic_id).await?, 1);
    assert_eq!(topic_reply_count(&db, tenant_id, target_topic_id).await?, 2);
    assert_eq!(
        reply_topic_and_position(&db, tenant_id, remaining_reply_id).await?,
        (source_topic_id, 1, None)
    );
    assert_eq!(
        reply_topic_and_position(&db, tenant_id, selected_root_id).await?,
        (target_topic_id, 1, None)
    );
    assert_eq!(
        reply_topic_and_position(&db, tenant_id, selected_child_id).await?,
        (target_topic_id, 2, Some(selected_root_id))
    );

    let solution_topic: Uuid = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT topic_id FROM forum_solutions WHERE tenant_id = ? AND reply_id = ?",
            vec![tenant_id.into(), selected_child_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("solution missing"))?
        .try_get("", "topic_id")?;
    assert_eq!(solution_topic, target_topic_id);
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*) AS value FROM forum_topic_channel_access WHERE tenant_id = ? AND topic_id = ? AND channel_slug = ?",
            vec![tenant_id.into(), target_topic_id.into(), "support".into()],
        )
        .await?,
        1
    );
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*) AS value FROM forum_reply_bodies WHERE tenant_id = ? AND reply_id IN (?, ?)",
            vec![
                tenant_id.into(),
                selected_root_id.into(),
                selected_child_id.into(),
            ],
        )
        .await?,
        body_rows_before
    );

    let category_after = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT topic_count, reply_count FROM forum_categories WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), category_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("category missing"))?;
    let topic_count_after: i64 = category_after.try_get("", "topic_count")?;
    let reply_count_after: i64 = category_after.try_get("", "reply_count")?;
    assert_eq!(topic_count_after, topic_count_before + 1);
    assert_eq!(reply_count_after, reply_count_before);
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*) AS value FROM forum_topic_split_operations WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        )
        .await?,
        1
    );
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*) AS value FROM forum_topic_split_reply_items WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        )
        .await?,
        2
    );
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*) AS value FROM forum_domain_events WHERE tenant_id = ? AND event_id = ? AND event_type = ?",
            vec![tenant_id.into(), operation_id.into(), "forum.topic.split".into()],
        )
        .await?,
        1
    );

    let mut conflict = input;
    conflict.reason = "Changed command".to_string();
    assert!(
        service
            .split_selected_replies(tenant_id, source_topic_id, admin, conflict)
            .await
            .is_err()
    );
    assert!(
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE forum_topic_split_operations SET reason = ? WHERE tenant_id = ? AND operation_id = ?",
            vec!["tamper".into(), tenant_id.into(), operation_id.into()],
        ))
        .await
        .is_err()
    );
    assert!(
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "DELETE FROM forum_topic_split_reply_items WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        ))
        .await
        .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn selected_reply_split_rejects_cross_boundary_parent_edges() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = create_category(&db, tenant_id, admin.clone()).await?;
    let source_topic_id =
        create_topic(&db, &event_bus, tenant_id, category_id, admin.clone()).await?;
    let selected_parent = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Selected parent",
        None,
    )
    .await?;
    create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Child left behind",
        Some(selected_parent),
    )
    .await?;
    create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Other root",
        None,
    )
    .await?;

    let target_topic_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let error = ForumTopicSplitService::new(db.clone(), event_bus)
        .split_selected_replies(
            tenant_id,
            source_topic_id,
            admin,
            SplitForumTopicRepliesInput {
                operation_id,
                target_topic_id,
                reply_ids: vec![selected_parent],
                locale: "en".to_string(),
                title: "Invalid split".to_string(),
                slug: None,
                reason: "Must fail atomically".to_string(),
            },
        )
        .await
        .expect_err("cross-boundary parent edge must fail");
    assert!(error.to_string().contains("child reply"));
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*) AS value FROM forum_topics WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), target_topic_id.into()],
        )
        .await?,
        0
    );
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*) AS value FROM forum_topic_split_operations WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        )
        .await?,
        0
    );
    Ok(())
}
