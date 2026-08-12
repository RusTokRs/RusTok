use std::collections::BTreeSet;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumError,
    ForumModule, ForumTopicMoveService, MoveForumTopicInput, ReplyService, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, QueryResult,
    Statement,
};
use sea_orm_migration::SchemaManager;
use serde_json::Value as JsonValue;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn setup() -> TestResult<(DatabaseConnection, TransactionalEventBus)> {
    let db_url = format!(
        "sqlite:file:forum_topic_move_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    db.execute_unprepared(
        r#"
        CREATE TABLE users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        )
        "#,
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
    slug: &str,
) -> TestResult<Uuid> {
    Ok(CategoryService::new(db.clone())
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".to_string(),
                name: slug.replace('-', " "),
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
    security: SecurityContext,
) -> TestResult<Uuid> {
    Ok(TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id,
                title: "Move owner topic".to_string(),
                slug: Some("move-owner-topic".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Move owner topic body"),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id)
}

async fn create_approved_reply(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    topic_id: Uuid,
    security: SecurityContext,
) -> TestResult<Uuid> {
    let reply = ReplyService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            topic_id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph(
                    "Approved reply moved with its topic",
                ),
                parent_reply_id: None,
            },
        )
        .await?;
    assert_eq!(reply.status, "approved");
    Ok(reply.id)
}

#[tokio::test]
async fn topic_move_is_atomic_idempotent_and_append_only() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));

    let source_category_id = create_category(&db, tenant_id, admin.clone(), "move-source").await?;
    let target_category_id = create_category(&db, tenant_id, admin.clone(), "move-target").await?;
    let topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        source_category_id,
        admin.clone(),
    )
    .await?;
    let _reply_id =
        create_approved_reply(&db, &event_bus, tenant_id, topic_id, admin.clone()).await?;

    assert_category_counters(&db, tenant_id, source_category_id, 1, 1).await?;
    assert_category_counters(&db, tenant_id, target_category_id, 0, 0).await?;
    let baseline_projection_ids = projection_root_ids(&db, tenant_id).await?;
    let operation_id = Uuid::new_v4();
    let input = MoveForumTopicInput {
        operation_id,
        target_category_id,
        reason: "Consolidate the discussion under the canonical category".to_string(),
    };

    let service = ForumTopicMoveService::new(db.clone(), event_bus.clone());
    let moved = service
        .move_topic(tenant_id, topic_id, admin.clone(), input.clone())
        .await?;
    assert_eq!(moved.operation_id, operation_id);
    assert_eq!(moved.event_id, operation_id);
    assert_eq!(moved.topic_id, topic_id);
    assert_eq!(moved.source_category_id, source_category_id);
    assert_eq!(moved.target_category_id, target_category_id);
    assert_eq!(moved.actor_id, actor_id);
    assert_eq!(moved.published_reply_count, 1);
    assert_eq!(moved.reason, input.reason);

    assert_eq!(
        topic_category_id(&db, tenant_id, topic_id).await?,
        target_category_id
    );
    assert_category_counters(&db, tenant_id, source_category_id, 0, 0).await?;
    assert_category_counters(&db, tenant_id, target_category_id, 1, 1).await?;
    assert_eq!(move_operation_count(&db, tenant_id).await?, 1);
    assert_semantic_event(&db, tenant_id, &moved).await?;

    let projection_ids_after_move = projection_root_ids(&db, tenant_id).await?;
    let new_projection_ids = projection_ids_after_move
        .difference(&baseline_projection_ids)
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(new_projection_ids.len(), 3);
    let targets = projection_targets(&db, tenant_id, &new_projection_ids).await?;
    let expected_targets = [
        ("forum_topic".to_string(), Some(topic_id)),
        ("forum_category".to_string(), Some(source_category_id)),
        ("forum_category".to_string(), Some(target_category_id)),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    assert_eq!(targets, expected_targets);

    let replay = service
        .move_topic(tenant_id, topic_id, admin.clone(), input.clone())
        .await?;
    assert_eq!(replay, moved);
    assert_eq!(move_operation_count(&db, tenant_id).await?, 1);
    assert_eq!(
        projection_root_ids(&db, tenant_id).await?,
        projection_ids_after_move
    );
    assert_category_counters(&db, tenant_id, source_category_id, 0, 0).await?;
    assert_category_counters(&db, tenant_id, target_category_id, 1, 1).await?;

    let conflict = service
        .move_topic(
            tenant_id,
            topic_id,
            admin.clone(),
            MoveForumTopicInput {
                operation_id,
                target_category_id,
                reason: "A different command payload".to_string(),
            },
        )
        .await;
    assert!(matches!(
        conflict,
        Err(ForumError::TopicMoveOperationConflict(id)) if id == operation_id
    ));
    assert_eq!(move_operation_count(&db, tenant_id).await?, 1);

    let same_category = service
        .move_topic(
            tenant_id,
            topic_id,
            admin.clone(),
            MoveForumTopicInput {
                operation_id: Uuid::new_v4(),
                target_category_id,
                reason: "No-op moves are not commands".to_string(),
            },
        )
        .await;
    assert!(matches!(same_category, Err(ForumError::Validation(_))));

    assert!(db
        .execute_unprepared(&format!(
            "UPDATE forum_topic_move_operations SET reason = 'tampered' WHERE tenant_id = '{tenant_id}' AND operation_id = '{operation_id}'"
        ))
        .await
        .is_err());
    assert!(db
        .execute_unprepared(&format!(
            "DELETE FROM forum_topic_move_operations WHERE tenant_id = '{tenant_id}' AND operation_id = '{operation_id}'"
        ))
        .await
        .is_err());

    assert_eq!(move_operation_count(&db, tenant_id).await?, 1);
    assert_semantic_event(&db, tenant_id, &moved).await?;
    Ok(())
}

#[tokio::test]
async fn topic_move_rejects_foreign_and_archived_targets_without_partial_state() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let foreign_actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    insert_user(&db, foreign_tenant_id, foreign_actor_id).await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let foreign_admin = SecurityContext::new(UserRole::Admin, Some(foreign_actor_id));

    let source_category_id = create_category(&db, tenant_id, admin.clone(), "guard-source").await?;
    let archived_target_id =
        create_category(&db, tenant_id, admin.clone(), "guard-archived").await?;
    let foreign_target_id =
        create_category(&db, foreign_tenant_id, foreign_admin, "guard-foreign").await?;
    let topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        source_category_id,
        admin.clone(),
    )
    .await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        r#"
        INSERT INTO forum_category_lifecycle (
            category_id, tenant_id, archived_at, updated_at
        ) VALUES (?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
        vec![archived_target_id.into(), tenant_id.into()],
    ))
    .await?;

    let baseline_projection_ids = projection_root_ids(&db, tenant_id).await?;
    let service = ForumTopicMoveService::new(db.clone(), event_bus);
    let archived = service
        .move_topic(
            tenant_id,
            topic_id,
            admin.clone(),
            MoveForumTopicInput {
                operation_id: Uuid::new_v4(),
                target_category_id: archived_target_id,
                reason: "Archived targets cannot receive active topics".to_string(),
            },
        )
        .await;
    assert!(matches!(archived, Err(ForumError::Validation(_))));

    let foreign = service
        .move_topic(
            tenant_id,
            topic_id,
            admin,
            MoveForumTopicInput {
                operation_id: Uuid::new_v4(),
                target_category_id: foreign_target_id,
                reason: "Foreign targets must remain absent".to_string(),
            },
        )
        .await;
    assert!(matches!(
        foreign,
        Err(ForumError::CategoryNotFound(id)) if id == foreign_target_id
    ));

    assert_eq!(
        topic_category_id(&db, tenant_id, topic_id).await?,
        source_category_id
    );
    assert_category_counters(&db, tenant_id, source_category_id, 1, 0).await?;
    assert_category_counters(&db, tenant_id, archived_target_id, 0, 0).await?;
    assert_eq!(move_operation_count(&db, tenant_id).await?, 0);
    assert_eq!(
        projection_root_ids(&db, tenant_id).await?,
        baseline_projection_ids
    );
    Ok(())
}

async fn topic_category_id(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<Uuid> {
    Ok(db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT category_id FROM forum_topics WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), topic_id.into()],
        ))
        .await?
        .ok_or("topic row missing")?
        .try_get("", "category_id")?)
}

async fn assert_category_counters(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    expected_topics: i32,
    expected_replies: i32,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT topic_count, reply_count FROM forum_categories WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), category_id.into()],
        ))
        .await?
        .ok_or("category row missing")?;
    assert_eq!(row.try_get::<i32>("", "topic_count")?, expected_topics);
    assert_eq!(row.try_get::<i32>("", "reply_count")?, expected_replies);
    Ok(())
}

async fn move_operation_count(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS value FROM forum_topic_move_operations WHERE tenant_id = ?",
            vec![tenant_id.into()],
        ),
    )
    .await
}

async fn assert_semantic_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    moved: &rustok_forum::ForumTopicMoveResult,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT event_id, aggregate_type, aggregate_id, event_type,
                   schema_version, actor_id, payload
            FROM forum_domain_events
            WHERE tenant_id = ? AND event_id = ?
            "#,
            vec![tenant_id.into(), moved.event_id.into()],
        ))
        .await?
        .ok_or("semantic event row missing")?;
    assert_eq!(row.try_get::<Uuid>("", "event_id")?, moved.event_id);
    assert_eq!(row.try_get::<String>("", "aggregate_type")?, "forum_topic");
    assert_eq!(row.try_get::<Uuid>("", "aggregate_id")?, moved.topic_id);
    assert_eq!(
        row.try_get::<String>("", "event_type")?,
        "forum.topic.moved"
    );
    assert_eq!(row.try_get::<i16>("", "schema_version")?, 1);
    assert_eq!(
        row.try_get::<Option<Uuid>>("", "actor_id")?,
        Some(moved.actor_id)
    );
    let payload: JsonValue = row.try_get("", "payload")?;
    assert_eq!(payload["operation_id"], moved.operation_id.to_string());
    assert_eq!(payload["topic_id"], moved.topic_id.to_string());
    assert_eq!(
        payload["source_category_id"],
        moved.source_category_id.to_string()
    );
    assert_eq!(
        payload["target_category_id"],
        moved.target_category_id.to_string()
    );
    assert_eq!(
        payload["published_reply_count"],
        moved.published_reply_count
    );
    assert_eq!(payload["reason"], moved.reason);
    Ok(())
}

async fn projection_root_ids(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<BTreeSet<Uuid>> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT payload FROM sys_events WHERE event_type = 'index.reindex_requested'",
            Vec::new(),
        ))
        .await?;
    let mut ids = BTreeSet::new();
    for row in rows {
        let payload: JsonValue = row.try_get("", "payload")?;
        let envelope: EventEnvelope = serde_json::from_value(payload)?;
        if envelope.tenant_id == tenant_id {
            ids.insert(envelope.id);
        }
    }
    Ok(ids)
}

async fn projection_targets(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    event_ids: &BTreeSet<Uuid>,
) -> TestResult<BTreeSet<(String, Option<Uuid>)>> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT payload FROM sys_events WHERE event_type = 'index.reindex_requested'",
            Vec::new(),
        ))
        .await?;
    let mut targets = BTreeSet::new();
    for row in rows {
        let payload: JsonValue = row.try_get("", "payload")?;
        let envelope: EventEnvelope = serde_json::from_value(payload)?;
        if envelope.tenant_id != tenant_id || !event_ids.contains(&envelope.id) {
            continue;
        }
        match envelope.event {
            DomainEvent::ReindexRequested {
                target_type,
                target_id,
            } => {
                targets.insert((target_type, target_id));
            }
            event => panic!("unexpected projection root event: {event:?}"),
        }
    }
    Ok(targets)
}

async fn scalar_i64(db: &DatabaseConnection, statement: Statement) -> TestResult<i64> {
    let row: QueryResult = db.query_one(statement).await?.ok_or("scalar row missing")?;
    Ok(row.try_get("", "value")?)
}
