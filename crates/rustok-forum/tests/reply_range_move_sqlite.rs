use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumError,
    ForumModule, ForumReplyRangeMoveService, MoveForumReplyRangeInput, ReplyService, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, QueryResult,
    Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn setup() -> TestResult<(DatabaseConnection, TransactionalEventBus)> {
    let mut options = ConnectOptions::new(format!(
        "sqlite:file:forum_reply_range_move_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    ));
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
    Ok((
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db))),
    ))
}

async fn execute(
    db: &DatabaseConnection,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> TestResult<()> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        sql,
        values,
    ))
    .await?;
    Ok(())
}

async fn row(
    db: &DatabaseConnection,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> TestResult<QueryResult> {
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        sql,
        values,
    ))
    .await?
    .ok_or_else(|| std::io::Error::other("expected row").into())
}

async fn count(db: &DatabaseConnection, sql: &str, values: Vec<sea_orm::Value>) -> TestResult<i64> {
    Ok(row(db, sql, values).await?.try_get("", "value")?)
}

async fn create_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    admin: SecurityContext,
    name: &str,
    slug: &str,
) -> TestResult<Uuid> {
    Ok(CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin,
            CreateCategoryInput {
                locale: "en".to_string(),
                name: name.to_string(),
                slug: slug.to_string(),
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
    admin: SecurityContext,
    title: &str,
    slug: &str,
) -> TestResult<Uuid> {
    Ok(TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            admin,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id,
                title: title.to_string(),
                slug: Some(slug.to_string()),
                body: rustok_api::RichTextDocument::single_paragraph(title),
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
    admin: SecurityContext,
    body: &str,
    parent_reply_id: Option<Uuid>,
) -> TestResult<Uuid> {
    Ok(ReplyService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            admin,
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

async fn topic_reply_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<i64> {
    Ok(row(
        db,
        "SELECT reply_count FROM forum_topics WHERE tenant_id = ? AND id = ?",
        vec![tenant_id.into(), topic_id.into()],
    )
    .await?
    .try_get("", "reply_count")?)
}

async fn reply_location(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> TestResult<(Uuid, i64, Option<Uuid>)> {
    let row = row(
        db,
        "SELECT topic_id, position, parent_reply_id FROM forum_replies WHERE tenant_id = ? AND id = ?",
        vec![tenant_id.into(), reply_id.into()],
    )
    .await?;
    Ok((
        row.try_get("", "topic_id")?,
        row.try_get("", "position")?,
        row.try_get("", "parent_reply_id")?,
    ))
}

#[tokio::test]
async fn reply_range_move_is_atomic_idempotent_and_preserves_identity() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    execute(
        &db,
        "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
        vec![actor_id.into(), tenant_id.into()],
    )
    .await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let source_category_id = create_category(
        &db,
        tenant_id,
        admin.clone(),
        "Source category",
        "source-category",
    )
    .await?;
    let target_category_id = create_category(
        &db,
        tenant_id,
        admin.clone(),
        "Target category",
        "target-category",
    )
    .await?;
    let source_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        source_category_id,
        admin.clone(),
        "Source topic",
        "source-topic",
    )
    .await?;
    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        target_category_id,
        admin.clone(),
        "Target topic",
        "target-topic",
    )
    .await?;

    let external_parent_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "External parent",
        None,
    )
    .await?;
    let moved_root_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Moved root",
        Some(external_parent_id),
    )
    .await?;
    let moved_child_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Moved child",
        Some(moved_root_id),
    )
    .await?;
    let source_tail_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Source tail",
        None,
    )
    .await?;
    let target_existing_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        target_topic_id,
        admin.clone(),
        "Target existing",
        None,
    )
    .await?;

    execute(
        &db,
        "INSERT INTO forum_solutions (topic_id, tenant_id, reply_id, marked_by_user_id, marked_at) VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)",
        vec![
            source_topic_id.into(),
            tenant_id.into(),
            moved_child_id.into(),
            actor_id.into(),
        ],
    )
    .await?;
    execute(
        &db,
        r#"
        INSERT INTO forum_reply_revisions (
            tenant_id, reply_id, locale, body, revision_reason, created_at
        )
        SELECT tenant_id, reply_id, locale, body, 'edit', CURRENT_TIMESTAMP
        FROM forum_reply_bodies
        WHERE tenant_id = ? AND reply_id = ? AND locale = 'en'
        "#,
        vec![tenant_id.into(), moved_root_id.into()],
    )
    .await?;
    execute(
        &db,
        "INSERT INTO forum_reply_votes (reply_id, user_id, tenant_id, value, created_at, updated_at) VALUES (?, ?, ?, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![moved_root_id.into(), actor_id.into(), tenant_id.into()],
    )
    .await?;
    execute(
        &db,
        "INSERT INTO forum_relation_revisions (tenant_id, target_kind, target_id, locale, projection_fingerprint, created_at) VALUES (?, 'reply', ?, 'en', 'range-move', CURRENT_TIMESTAMP)",
        vec![tenant_id.into(), moved_root_id.into()],
    )
    .await?;

    let source_category_before = row(
        &db,
        "SELECT topic_count, reply_count FROM forum_categories WHERE tenant_id = ? AND id = ?",
        vec![tenant_id.into(), source_category_id.into()],
    )
    .await?;
    let target_category_before = row(
        &db,
        "SELECT topic_count, reply_count FROM forum_categories WHERE tenant_id = ? AND id = ?",
        vec![tenant_id.into(), target_category_id.into()],
    )
    .await?;
    let operation_id = Uuid::new_v4();
    let input = MoveForumReplyRangeInput {
        operation_id,
        target_topic_id,
        start_position: 2,
        end_position: 3,
        reason: "Move a bounded support exchange".to_string(),
    };
    let service = ForumReplyRangeMoveService::new(db.clone(), event_bus);
    let first = service
        .move_reply_range(tenant_id, source_topic_id, admin.clone(), input.clone())
        .await?;
    let replay = service
        .move_reply_range(tenant_id, source_topic_id, admin.clone(), input.clone())
        .await?;

    assert_eq!(first, replay);
    assert_eq!(first.operation_id, operation_id);
    assert_eq!(first.event_id, operation_id);
    assert_eq!(first.source_start_position, 2);
    assert_eq!(first.source_end_position, 3);
    assert_eq!(first.target_start_position, 2);
    assert_eq!(first.target_end_position, 3);
    assert_eq!(first.moved_reply_count, 2);
    assert_eq!(first.moved_published_reply_count, 2);
    assert_eq!(first.source_resulting_published_reply_count, 2);
    assert_eq!(first.target_resulting_published_reply_count, 3);
    assert_eq!(first.moved_solution_reply_id, Some(moved_child_id));
    assert_eq!(first.source_resulting_solution_reply_id, None);
    assert_eq!(
        first.target_resulting_solution_reply_id,
        Some(moved_child_id)
    );

    assert_eq!(
        reply_location(&db, tenant_id, external_parent_id).await?,
        (source_topic_id, 1, None)
    );
    assert_eq!(
        reply_location(&db, tenant_id, moved_root_id).await?,
        (target_topic_id, 2, None)
    );
    assert_eq!(
        reply_location(&db, tenant_id, moved_child_id).await?,
        (target_topic_id, 3, Some(moved_root_id))
    );
    assert_eq!(
        reply_location(&db, tenant_id, source_tail_id).await?,
        (source_topic_id, 4, None)
    );
    assert_eq!(
        reply_location(&db, tenant_id, target_existing_id).await?,
        (target_topic_id, 1, None)
    );
    assert_eq!(topic_reply_count(&db, tenant_id, source_topic_id).await?, 2);
    assert_eq!(topic_reply_count(&db, tenant_id, target_topic_id).await?, 3);

    let solution_topic_id: Uuid = row(
        &db,
        "SELECT topic_id FROM forum_solutions WHERE tenant_id = ? AND reply_id = ?",
        vec![tenant_id.into(), moved_child_id.into()],
    )
    .await?
    .try_get("", "topic_id")?;
    assert_eq!(solution_topic_id, target_topic_id);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_reply_bodies WHERE tenant_id = ? AND reply_id IN (?, ?)",
            vec![tenant_id.into(), moved_root_id.into(), moved_child_id.into()],
        )
        .await?,
        2
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_reply_revisions WHERE tenant_id = ? AND reply_id = ?",
            vec![tenant_id.into(), moved_root_id.into()],
        )
        .await?,
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_reply_votes WHERE tenant_id = ? AND reply_id = ? AND value = 1",
            vec![tenant_id.into(), moved_root_id.into()],
        )
        .await?,
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_relation_revisions WHERE tenant_id = ? AND target_kind = 'reply' AND target_id = ? AND projection_fingerprint = 'range-move'",
            vec![tenant_id.into(), moved_root_id.into()],
        )
        .await?,
        1
    );

    let source_category_after = row(
        &db,
        "SELECT topic_count, reply_count FROM forum_categories WHERE tenant_id = ? AND id = ?",
        vec![tenant_id.into(), source_category_id.into()],
    )
    .await?;
    let target_category_after = row(
        &db,
        "SELECT topic_count, reply_count FROM forum_categories WHERE tenant_id = ? AND id = ?",
        vec![tenant_id.into(), target_category_id.into()],
    )
    .await?;
    assert_eq!(
        source_category_after.try_get::<i64>("", "topic_count")?,
        source_category_before.try_get::<i64>("", "topic_count")?
    );
    assert_eq!(
        source_category_after.try_get::<i64>("", "reply_count")?,
        source_category_before.try_get::<i64>("", "reply_count")? - 2
    );
    assert_eq!(
        target_category_after.try_get::<i64>("", "topic_count")?,
        target_category_before.try_get::<i64>("", "topic_count")?
    );
    assert_eq!(
        target_category_after.try_get::<i64>("", "reply_count")?,
        target_category_before.try_get::<i64>("", "reply_count")? + 2
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_reply_range_move_operations WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        )
        .await?,
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_reply_range_move_items WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        )
        .await?,
        2
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_domain_events WHERE tenant_id = ? AND event_id = ? AND event_type = 'forum.topic.reply_range_moved'",
            vec![tenant_id.into(), operation_id.into()],
        )
        .await?,
        1
    );

    let mut conflict = input;
    conflict.reason = "Changed command".to_string();
    assert!(matches!(
        service
            .move_reply_range(tenant_id, source_topic_id, admin, conflict)
            .await,
        Err(ForumError::TopicReplyRangeMoveOperationConflict(id)) if id == operation_id
    ));
    assert!(
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE forum_reply_range_move_operations SET reason = ? WHERE tenant_id = ? AND operation_id = ?",
            vec!["tamper".into(), tenant_id.into(), operation_id.into()],
        ))
        .await
        .is_err()
    );
    assert!(
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "DELETE FROM forum_reply_range_move_items WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        ))
        .await
        .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn reply_range_move_rejects_outgoing_child_boundary_atomically() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    execute(
        &db,
        "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
        vec![actor_id.into(), tenant_id.into()],
    )
    .await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = create_category(&db, tenant_id, admin.clone(), "Range", "range").await?;
    let source_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "Source",
        "range-source",
    )
    .await?;
    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "Target",
        "range-target",
    )
    .await?;
    let keep_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Keep",
        None,
    )
    .await?;
    let selected_parent_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Selected parent",
        None,
    )
    .await?;
    let child_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Child left behind",
        Some(selected_parent_id),
    )
    .await?;

    let operation_id = Uuid::new_v4();
    let error = ForumReplyRangeMoveService::new(db.clone(), event_bus)
        .move_reply_range(
            tenant_id,
            source_topic_id,
            admin,
            MoveForumReplyRangeInput {
                operation_id,
                target_topic_id,
                start_position: 2,
                end_position: 2,
                reason: "Must reject the crossing child".to_string(),
            },
        )
        .await
        .expect_err("outgoing child edge must fail");
    assert!(error.to_string().contains("leave a child behind"));
    assert_eq!(
        reply_location(&db, tenant_id, keep_id).await?.0,
        source_topic_id
    );
    assert_eq!(
        reply_location(&db, tenant_id, selected_parent_id).await?.0,
        source_topic_id
    );
    assert_eq!(
        reply_location(&db, tenant_id, child_id).await?.0,
        source_topic_id
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_reply_range_move_operations WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        )
        .await?,
        0
    );
    Ok(())
}

#[tokio::test]
async fn reply_range_move_rejects_competing_solutions_atomically() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    execute(
        &db,
        "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
        vec![actor_id.into(), tenant_id.into()],
    )
    .await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id =
        create_category(&db, tenant_id, admin.clone(), "Solutions", "solutions").await?;
    let source_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "Solved source",
        "solved-source",
    )
    .await?;
    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "Solved target",
        "solved-target",
    )
    .await?;
    create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Keep source",
        None,
    )
    .await?;
    let source_solution_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Source solution",
        None,
    )
    .await?;
    let target_solution_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        target_topic_id,
        admin.clone(),
        "Target solution",
        None,
    )
    .await?;
    execute(
        &db,
        "INSERT INTO forum_solutions (topic_id, tenant_id, reply_id, marked_by_user_id, marked_at) VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)",
        vec![
            source_topic_id.into(),
            tenant_id.into(),
            source_solution_id.into(),
            actor_id.into(),
        ],
    )
    .await?;
    execute(
        &db,
        "INSERT INTO forum_solutions (topic_id, tenant_id, reply_id, marked_by_user_id, marked_at) VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)",
        vec![
            target_topic_id.into(),
            tenant_id.into(),
            target_solution_id.into(),
            actor_id.into(),
        ],
    )
    .await?;

    let operation_id = Uuid::new_v4();
    assert!(matches!(
        ForumReplyRangeMoveService::new(db.clone(), event_bus)
            .move_reply_range(
                tenant_id,
                source_topic_id,
                admin,
                MoveForumReplyRangeInput {
                    operation_id,
                    target_topic_id,
                    start_position: 2,
                    end_position: 2,
                    reason: "Competing solutions require explicit policy".to_string(),
                },
            )
            .await,
        Err(ForumError::TopicReplyRangeMoveSolutionConflict(id)) if id == operation_id
    ));
    assert_eq!(
        reply_location(&db, tenant_id, source_solution_id).await?.0,
        source_topic_id
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_solutions WHERE tenant_id = ? AND topic_id IN (?, ?)",
            vec![
                tenant_id.into(),
                source_topic_id.into(),
                target_topic_id.into(),
            ],
        )
        .await?,
        2
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_reply_range_move_operations WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        )
        .await?,
        0
    );
    Ok(())
}
