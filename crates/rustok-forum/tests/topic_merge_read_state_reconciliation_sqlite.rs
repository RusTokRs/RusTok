use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumError,
    ForumModule, ForumTopicMergeReadStateReconciliationService, ForumTopicMergeService,
    ForumTopicReadStateService, MarkForumTopicReadInput, MarkForumTopicsReadBatchInput,
    MergeForumTopicInput, ReconcileForumTopicMergeReadStatesInput, ReplyService, TopicService,
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

type ReadStateSnapshot = (Uuid, i64, i64, String, String);

async fn setup() -> TestResult<(DatabaseConnection, TransactionalEventBus)> {
    let db_url = format!(
        "sqlite:file:forum_topic_merge_read_state_reconciliation_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    db.execute_unprepared(
        "CREATE TABLE users (id TEXT NOT NULL PRIMARY KEY, tenant_id TEXT NOT NULL, UNIQUE (tenant_id, id))",
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
                name: "Merge read state".to_string(),
                slug: "merge-read-state".to_string(),
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
    key: &str,
) -> TestResult<Uuid> {
    Ok(TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id,
                title: format!("Merge read state {key}"),
                slug: Some(format!("merge-read-state-{key}")),
                body: rustok_api::RichTextDocument::single_paragraph(format!(
                    "Merge read state {key} body"
                )),
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
    content: &str,
) -> TestResult<()> {
    ReplyService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            topic_id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph(content),
                parent_reply_id: None,
            },
        )
        .await?;
    Ok(())
}

async fn insert_read_state(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    user_id: Uuid,
    last_read_position: i64,
    last_read_revision: i64,
    timestamp: &str,
) -> TestResult<()> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        r#"
        INSERT INTO forum_topic_read_states (
            tenant_id, topic_id, user_id, last_read_position, last_read_revision,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
        vec![
            tenant_id.into(),
            topic_id.into(),
            user_id.into(),
            last_read_position.into(),
            last_read_revision.into(),
            timestamp.into(),
            timestamp.into(),
        ],
    ))
    .await?;
    Ok(())
}

#[tokio::test]
async fn merge_read_state_reconciliation_is_conservative_atomic_and_idempotent() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let source_only_user = Uuid::new_v4();
    let overlap_user_one = Uuid::new_v4();
    let overlap_user_two = Uuid::new_v4();
    let target_only_user = Uuid::new_v4();
    let raw_guard_user = Uuid::new_v4();
    let bulk_user = Uuid::new_v4();
    for user_id in [
        actor_id,
        source_only_user,
        overlap_user_one,
        overlap_user_two,
        target_only_user,
        raw_guard_user,
        bulk_user,
    ] {
        insert_user(&db, tenant_id, user_id).await?;
    }

    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = create_category(&db, tenant_id, admin.clone()).await?;
    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "target",
    )
    .await?;
    let source_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "source",
    )
    .await?;
    create_reply(
        &db,
        &event_bus,
        tenant_id,
        target_topic_id,
        admin.clone(),
        "Target reply",
    )
    .await?;
    create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Source reply one",
    )
    .await?;
    create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Source reply two",
    )
    .await?;

    insert_read_state(
        &db,
        tenant_id,
        source_topic_id,
        source_only_user,
        2,
        0,
        "2026-08-01T00:00:01Z",
    )
    .await?;
    insert_read_state(
        &db,
        tenant_id,
        source_topic_id,
        overlap_user_one,
        1,
        0,
        "2026-08-01T00:00:02Z",
    )
    .await?;
    insert_read_state(
        &db,
        tenant_id,
        source_topic_id,
        overlap_user_two,
        2,
        0,
        "2026-08-01T00:00:03Z",
    )
    .await?;
    insert_read_state(
        &db,
        tenant_id,
        target_topic_id,
        overlap_user_one,
        1,
        0,
        "2026-08-01T00:00:11Z",
    )
    .await?;
    insert_read_state(
        &db,
        tenant_id,
        target_topic_id,
        overlap_user_two,
        0,
        0,
        "2026-08-01T00:00:12Z",
    )
    .await?;
    insert_read_state(
        &db,
        tenant_id,
        target_topic_id,
        target_only_user,
        1,
        0,
        "2026-08-01T00:00:13Z",
    )
    .await?;
    let target_before = read_state_snapshots(&db, tenant_id, target_topic_id).await?;

    let merge_operation_id = Uuid::new_v4();
    ForumTopicMergeService::new(db.clone(), event_bus)
        .merge_topic(
            tenant_id,
            target_topic_id,
            admin.clone(),
            MergeForumTopicInput {
                operation_id: merge_operation_id,
                source_topic_id,
                reason: "Merge duplicate topic before read-state reconciliation".to_string(),
            },
        )
        .await?;

    let service_write = ForumTopicReadStateService::new(db.clone())
        .mark_topic_read(
            tenant_id,
            source_topic_id,
            SecurityContext::new(UserRole::Customer, Some(source_only_user)),
            MarkForumTopicReadInput {
                last_read_position: 2,
                last_read_revision: 0,
            },
        )
        .await;
    assert!(matches!(service_write, Err(ForumError::Validation(_))));

    assert!(
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            INSERT INTO forum_topic_read_states (
                tenant_id, topic_id, user_id, last_read_position, last_read_revision,
                created_at, updated_at
            ) VALUES (?, ?, ?, 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
            vec![
                tenant_id.into(),
                source_topic_id.into(),
                raw_guard_user.into()
            ],
        ))
        .await
        .is_err()
    );
    assert!(
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            UPDATE forum_topic_read_states
               SET last_read_position = last_read_position + 1,
                   updated_at = CURRENT_TIMESTAMP
             WHERE tenant_id = ? AND topic_id = ? AND user_id = ?
            "#,
            vec![
                tenant_id.into(),
                source_topic_id.into(),
                source_only_user.into(),
            ],
        ))
        .await
        .is_err()
    );

    let operation_id = Uuid::new_v4();
    let input = ReconcileForumTopicMergeReadStatesInput {
        operation_id,
        reason: "Discard source high-water without marking target content read".to_string(),
    };
    let service = ForumTopicMergeReadStateReconciliationService::new(db.clone());
    let reconciled = service
        .reconcile_merge_read_states(tenant_id, merge_operation_id, admin.clone(), input.clone())
        .await?;

    assert_eq!(reconciled.operation_id, operation_id);
    assert_eq!(reconciled.event_id, operation_id);
    assert_eq!(reconciled.merge_operation_id, merge_operation_id);
    assert_eq!(reconciled.source_topic_id, source_topic_id);
    assert_eq!(reconciled.target_topic_id, target_topic_id);
    assert_eq!(reconciled.actor_id, actor_id);
    assert_eq!(reconciled.source_read_state_count, 3);
    assert_eq!(reconciled.discarded_source_only_count, 1);
    assert_eq!(reconciled.discarded_target_overlap_count, 2);
    assert_eq!(read_state_count(&db, tenant_id, source_topic_id).await?, 0);
    assert_eq!(
        read_state_snapshots(&db, tenant_id, target_topic_id).await?,
        target_before
    );
    assert_reconciliation_event(&db, tenant_id, &reconciled).await?;
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);

    let replay = service
        .reconcile_merge_read_states(tenant_id, merge_operation_id, admin.clone(), input.clone())
        .await?;
    assert_eq!(replay, reconciled);
    assert_eq!(read_state_count(&db, tenant_id, source_topic_id).await?, 0);
    assert_eq!(
        read_state_snapshots(&db, tenant_id, target_topic_id).await?,
        target_before
    );
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);

    let bulk = ForumTopicReadStateService::new(db.clone())
        .mark_all_read(
            tenant_id,
            SecurityContext::new(UserRole::Customer, Some(bulk_user)),
            MarkForumTopicsReadBatchInput {
                cursor: None,
                limit: Some(10),
            },
        )
        .await?;
    assert_eq!(bulk.processed, 1);
    assert!(!bulk.has_more);
    assert_eq!(read_state_count(&db, tenant_id, source_topic_id).await?, 0);
    assert_eq!(read_state_count(&db, tenant_id, target_topic_id).await?, 4);

    let drift = service
        .reconcile_merge_read_states(
            tenant_id,
            merge_operation_id,
            admin.clone(),
            ReconcileForumTopicMergeReadStatesInput {
                operation_id,
                reason: "Changed read-state reconciliation command".to_string(),
            },
        )
        .await;
    assert!(matches!(
        drift,
        Err(ForumError::TopicMergeReadStateReconciliationConflict(id)) if id == operation_id
    ));

    let second_operation = service
        .reconcile_merge_read_states(
            tenant_id,
            merge_operation_id,
            admin,
            ReconcileForumTopicMergeReadStatesInput {
                operation_id: Uuid::new_v4(),
                reason: "A merge may reconcile read state only once".to_string(),
            },
        )
        .await;
    assert!(matches!(
        second_operation,
        Err(ForumError::TopicMergeReadStateReconciliationConflict(_))
    ));

    assert!(db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE forum_topic_merge_read_state_reconciliations SET reason = 'tampered' WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        ))
        .await
        .is_err());
    assert!(db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "DELETE FROM forum_topic_merge_read_state_reconciliations WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        ))
        .await
        .is_err());
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);
    assert_reconciliation_event(&db, tenant_id, &reconciled).await?;
    Ok(())
}

#[tokio::test]
async fn merge_read_state_reconciliation_requires_a_real_merge_receipt() -> TestResult<()> {
    let (db, _event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let result = ForumTopicMergeReadStateReconciliationService::new(db)
        .reconcile_merge_read_states(
            tenant_id,
            Uuid::new_v4(),
            SecurityContext::new(UserRole::Admin, Some(actor_id)),
            ReconcileForumTopicMergeReadStatesInput {
                operation_id: Uuid::new_v4(),
                reason: "Missing merge receipts must fail closed".to_string(),
            },
        )
        .await;
    assert!(matches!(result, Err(ForumError::Validation(_))));
    Ok(())
}

async fn read_state_snapshots(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<Vec<ReadStateSnapshot>> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT user_id, last_read_position, last_read_revision, created_at, updated_at
              FROM forum_topic_read_states
             WHERE tenant_id = ? AND topic_id = ?
             ORDER BY user_id
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ))
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok((
                row.try_get("", "user_id")?,
                row.try_get("", "last_read_position")?,
                row.try_get("", "last_read_revision")?,
                row.try_get("", "created_at")?,
                row.try_get("", "updated_at")?,
            ))
        })
        .collect()
}

async fn read_state_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS value FROM forum_topic_read_states WHERE tenant_id = ? AND topic_id = ?",
            vec![tenant_id.into(), topic_id.into()],
        ),
    )
    .await
}

async fn reconciliation_count(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS value FROM forum_topic_merge_read_state_reconciliations WHERE tenant_id = ?",
            vec![tenant_id.into()],
        ),
    )
    .await
}

async fn assert_reconciliation_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    reconciled: &rustok_forum::ForumTopicMergeReadStateReconciliationResult,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT aggregate_type, aggregate_id, event_type, schema_version, actor_id, payload
              FROM forum_domain_events
             WHERE tenant_id = ? AND event_id = ?
            "#,
            vec![tenant_id.into(), reconciled.event_id.into()],
        ))
        .await?
        .ok_or("reconciliation semantic event missing")?;
    assert_eq!(row.try_get::<String>("", "aggregate_type")?, "forum_topic");
    assert_eq!(
        row.try_get::<Uuid>("", "aggregate_id")?,
        reconciled.target_topic_id
    );
    assert_eq!(
        row.try_get::<String>("", "event_type")?,
        "forum.topic.merge.read_state_reconciled"
    );
    assert_eq!(row.try_get::<i16>("", "schema_version")?, 1);
    assert_eq!(
        row.try_get::<Option<Uuid>>("", "actor_id")?,
        Some(reconciled.actor_id)
    );
    let payload: JsonValue = row.try_get("", "payload")?;
    assert_eq!(payload["operation_id"], reconciled.operation_id.to_string());
    assert_eq!(
        payload["merge_operation_id"],
        reconciled.merge_operation_id.to_string()
    );
    assert_eq!(
        payload["source_topic_id"],
        reconciled.source_topic_id.to_string()
    );
    assert_eq!(
        payload["target_topic_id"],
        reconciled.target_topic_id.to_string()
    );
    assert_eq!(
        payload["source_read_state_count"],
        reconciled.source_read_state_count
    );
    assert_eq!(
        payload["discarded_source_only_count"],
        reconciled.discarded_source_only_count
    );
    assert_eq!(
        payload["discarded_target_overlap_count"],
        reconciled.discarded_target_overlap_count
    );
    assert_eq!(payload["reason"], reconciled.reason);
    Ok(())
}

async fn scalar_i64(db: &DatabaseConnection, statement: Statement) -> TestResult<i64> {
    let row: QueryResult = db.query_one(statement).await?.ok_or("scalar row missing")?;
    Ok(row.try_get("", "value")?)
}
