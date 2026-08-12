use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumDigestMode, ForumError,
    ForumModule, ForumSubscriptionLevel, ForumTopicMergeService,
    ForumTopicMergeSubscriptionReconciliationService, MergeForumTopicInput,
    ReconcileForumTopicMergeSubscriptionsInput, SubscriptionService, TopicService,
    UpdateForumSubscriptionInput,
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
        "sqlite:file:forum_topic_merge_subscription_reconciliation_{}?mode=memory&cache=shared",
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
                name: "Merge subscriptions".to_string(),
                slug: "merge-subscriptions".to_string(),
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
                title: format!("Merge subscriptions {key}"),
                slug: Some(format!("merge-subscriptions-{key}")),
                body: rustok_api::RichTextDocument::single_paragraph(format!(
                    "Merge subscriptions {key} body"
                )),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id)
}

#[allow(clippy::too_many_arguments)]
async fn insert_subscription(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    user_id: Uuid,
    level: &str,
    notify_mentions: bool,
    notify_replies: bool,
    notify_new_topics: bool,
    digest_mode: &str,
    revision: i64,
) -> TestResult<()> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        r#"
        INSERT INTO forum_topic_subscriptions (
            topic_id, user_id, tenant_id, level,
            notify_mentions, notify_replies, notify_new_topics,
            digest_mode, last_notified_at, revision, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
        vec![
            topic_id.into(),
            user_id.into(),
            tenant_id.into(),
            level.into(),
            notify_mentions.into(),
            notify_replies.into(),
            notify_new_topics.into(),
            digest_mode.into(),
            revision.into(),
        ],
    ))
    .await?;
    Ok(())
}

#[tokio::test]
async fn merge_subscription_reconciliation_is_atomic_idempotent_and_target_authoritative()
-> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let source_only_user = Uuid::new_v4();
    let target_only_user = Uuid::new_v4();
    let equal_user = Uuid::new_v4();
    let conflict_user = Uuid::new_v4();
    let raw_guard_user = Uuid::new_v4();
    for user_id in [
        actor_id,
        source_only_user,
        target_only_user,
        equal_user,
        conflict_user,
        raw_guard_user,
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

    insert_subscription(
        &db,
        tenant_id,
        source_topic_id,
        source_only_user,
        "watching",
        true,
        true,
        true,
        "immediate",
        7,
    )
    .await?;
    insert_subscription(
        &db,
        tenant_id,
        target_topic_id,
        target_only_user,
        "tracking",
        true,
        false,
        false,
        "disabled",
        4,
    )
    .await?;
    insert_subscription(
        &db,
        tenant_id,
        source_topic_id,
        equal_user,
        "watching",
        true,
        true,
        false,
        "daily",
        2,
    )
    .await?;
    insert_subscription(
        &db,
        tenant_id,
        target_topic_id,
        equal_user,
        "watching",
        true,
        true,
        false,
        "daily",
        9,
    )
    .await?;
    insert_subscription(
        &db,
        tenant_id,
        source_topic_id,
        conflict_user,
        "watching",
        true,
        true,
        true,
        "immediate",
        3,
    )
    .await?;
    insert_subscription(
        &db,
        tenant_id,
        target_topic_id,
        conflict_user,
        "muted",
        false,
        false,
        false,
        "disabled",
        5,
    )
    .await?;

    let merge_operation_id = Uuid::new_v4();
    ForumTopicMergeService::new(db.clone(), event_bus)
        .merge_topic(
            tenant_id,
            target_topic_id,
            admin.clone(),
            MergeForumTopicInput {
                operation_id: merge_operation_id,
                source_topic_id,
                reason: "Merge duplicate topic before subscription reconciliation".to_string(),
            },
        )
        .await?;

    let source_write = SubscriptionService::new(db.clone())
        .update_topic_subscription(
            tenant_id,
            source_topic_id,
            SecurityContext::new(UserRole::Admin, Some(source_only_user)),
            UpdateForumSubscriptionInput {
                level: ForumSubscriptionLevel::Tracking,
                notify_mentions: None,
                notify_replies: None,
                notify_new_topics: None,
                digest_mode: Some(ForumDigestMode::Disabled),
                expected_revision: Some(7),
            },
        )
        .await;
    assert!(matches!(source_write, Err(ForumError::Validation(_))));
    assert_archived_subscription_database_guards(
        &db,
        tenant_id,
        source_topic_id,
        source_only_user,
        raw_guard_user,
    )
    .await?;

    let operation_id = Uuid::new_v4();
    let input = ReconcileForumTopicMergeSubscriptionsInput {
        operation_id,
        reason: "Retain target subscription authority after topic merge".to_string(),
    };
    let service = ForumTopicMergeSubscriptionReconciliationService::new(db.clone());
    let reconciled = service
        .reconcile_merge_subscriptions(tenant_id, merge_operation_id, admin.clone(), input.clone())
        .await?;

    assert_eq!(reconciled.operation_id, operation_id);
    assert_eq!(reconciled.event_id, operation_id);
    assert_eq!(reconciled.merge_operation_id, merge_operation_id);
    assert_eq!(reconciled.source_topic_id, source_topic_id);
    assert_eq!(reconciled.target_topic_id, target_topic_id);
    assert_eq!(reconciled.actor_id, actor_id);
    assert_eq!(reconciled.source_subscription_count, 4);
    assert_eq!(reconciled.moved_source_only_count, 1);
    assert_eq!(reconciled.deduplicated_equal_count, 2);
    assert_eq!(reconciled.target_authority_conflict_count, 1);

    assert_eq!(
        subscription_count(&db, tenant_id, source_topic_id).await?,
        0
    );
    assert_eq!(
        subscription_count(&db, tenant_id, target_topic_id).await?,
        5
    );
    assert_subscription(
        &db,
        tenant_id,
        target_topic_id,
        actor_id,
        "watching",
        true,
        true,
        true,
        "immediate",
        1,
    )
    .await?;
    assert_subscription(
        &db,
        tenant_id,
        target_topic_id,
        source_only_user,
        "watching",
        true,
        true,
        true,
        "immediate",
        8,
    )
    .await?;
    assert_subscription(
        &db,
        tenant_id,
        target_topic_id,
        target_only_user,
        "tracking",
        true,
        false,
        false,
        "disabled",
        4,
    )
    .await?;
    assert_subscription(
        &db,
        tenant_id,
        target_topic_id,
        equal_user,
        "watching",
        true,
        true,
        false,
        "daily",
        9,
    )
    .await?;
    assert_subscription(
        &db,
        tenant_id,
        target_topic_id,
        conflict_user,
        "muted",
        false,
        false,
        false,
        "disabled",
        5,
    )
    .await?;
    assert_reconciliation_event(&db, tenant_id, &reconciled).await?;
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);

    let replay = service
        .reconcile_merge_subscriptions(tenant_id, merge_operation_id, admin.clone(), input.clone())
        .await?;
    assert_eq!(replay, reconciled);
    assert_eq!(
        subscription_count(&db, tenant_id, source_topic_id).await?,
        0
    );
    assert_eq!(
        subscription_count(&db, tenant_id, target_topic_id).await?,
        5
    );
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);

    let drift = service
        .reconcile_merge_subscriptions(
            tenant_id,
            merge_operation_id,
            admin.clone(),
            ReconcileForumTopicMergeSubscriptionsInput {
                operation_id,
                reason: "Changed reconciliation command".to_string(),
            },
        )
        .await;
    assert!(matches!(
        drift,
        Err(ForumError::TopicMergeSubscriptionReconciliationConflict(id)) if id == operation_id
    ));

    let second_operation = service
        .reconcile_merge_subscriptions(
            tenant_id,
            merge_operation_id,
            admin,
            ReconcileForumTopicMergeSubscriptionsInput {
                operation_id: Uuid::new_v4(),
                reason: "A merge may be reconciled only once".to_string(),
            },
        )
        .await;
    assert!(matches!(
        second_operation,
        Err(ForumError::TopicMergeSubscriptionReconciliationConflict(_))
    ));

    assert!(db
        .execute_unprepared(&format!(
            "UPDATE forum_topic_merge_subscription_reconciliations SET reason = 'tampered' WHERE tenant_id = '{tenant_id}' AND operation_id = '{operation_id}'"
        ))
        .await
        .is_err());
    assert!(db
        .execute_unprepared(&format!(
            "DELETE FROM forum_topic_merge_subscription_reconciliations WHERE tenant_id = '{tenant_id}' AND operation_id = '{operation_id}'"
        ))
        .await
        .is_err());
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);
    assert_reconciliation_event(&db, tenant_id, &reconciled).await?;
    Ok(())
}

#[tokio::test]
async fn merge_subscription_reconciliation_requires_a_real_merge_receipt() -> TestResult<()> {
    let (db, _event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let result = ForumTopicMergeSubscriptionReconciliationService::new(db)
        .reconcile_merge_subscriptions(
            tenant_id,
            Uuid::new_v4(),
            SecurityContext::new(UserRole::Admin, Some(actor_id)),
            ReconcileForumTopicMergeSubscriptionsInput {
                operation_id: Uuid::new_v4(),
                reason: "Missing merge receipts must fail closed".to_string(),
            },
        )
        .await;
    assert!(matches!(result, Err(ForumError::Validation(_))));
    Ok(())
}

async fn assert_archived_subscription_database_guards(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    existing_user_id: Uuid,
    new_user_id: Uuid,
) -> TestResult<()> {
    let update = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            UPDATE forum_topic_subscriptions
            SET revision = revision + 1, updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = ? AND topic_id = ? AND user_id = ?
            "#,
            vec![
                tenant_id.into(),
                source_topic_id.into(),
                existing_user_id.into(),
            ],
        ))
        .await;
    assert!(update.is_err());

    let insert = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            INSERT INTO forum_topic_subscriptions (
                topic_id, user_id, tenant_id, level,
                notify_mentions, notify_replies, notify_new_topics,
                digest_mode, last_notified_at, revision, created_at, updated_at
            ) VALUES (?, ?, ?, 'tracking', 1, 0, 0, 'disabled', NULL, 1,
                      CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
            vec![source_topic_id.into(), new_user_id.into(), tenant_id.into()],
        ))
        .await;
    assert!(insert.is_err());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn assert_subscription(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    user_id: Uuid,
    expected_level: &str,
    expected_notify_mentions: bool,
    expected_notify_replies: bool,
    expected_notify_new_topics: bool,
    expected_digest_mode: &str,
    expected_revision: i64,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT level, notify_mentions, notify_replies, notify_new_topics,
                   digest_mode, revision
            FROM forum_topic_subscriptions
            WHERE tenant_id = ? AND topic_id = ? AND user_id = ?
            "#,
            vec![tenant_id.into(), topic_id.into(), user_id.into()],
        ))
        .await?
        .ok_or("topic subscription row missing")?;
    assert_eq!(row.try_get::<String>("", "level")?, expected_level);
    assert_eq!(
        row.try_get::<bool>("", "notify_mentions")?,
        expected_notify_mentions
    );
    assert_eq!(
        row.try_get::<bool>("", "notify_replies")?,
        expected_notify_replies
    );
    assert_eq!(
        row.try_get::<bool>("", "notify_new_topics")?,
        expected_notify_new_topics
    );
    assert_eq!(
        row.try_get::<String>("", "digest_mode")?,
        expected_digest_mode
    );
    assert_eq!(row.try_get::<i64>("", "revision")?, expected_revision);
    Ok(())
}

async fn subscription_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS value FROM forum_topic_subscriptions WHERE tenant_id = ? AND topic_id = ?",
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
            "SELECT COUNT(*) AS value FROM forum_topic_merge_subscription_reconciliations WHERE tenant_id = ?",
            vec![tenant_id.into()],
        ),
    )
    .await
}

async fn assert_reconciliation_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    reconciled: &rustok_forum::ForumTopicMergeSubscriptionReconciliationResult,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT aggregate_type, aggregate_id, event_type, schema_version,
                   actor_id, payload
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
        "forum.topic.merge_subscriptions_reconciled"
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
        payload["source_subscription_count"],
        reconciled.source_subscription_count
    );
    assert_eq!(
        payload["moved_source_only_count"],
        reconciled.moved_source_only_count
    );
    assert_eq!(
        payload["deduplicated_equal_count"],
        reconciled.deduplicated_equal_count
    );
    assert_eq!(
        payload["target_authority_conflict_count"],
        reconciled.target_authority_conflict_count
    );
    assert_eq!(payload["reason"], reconciled.reason);
    Ok(())
}

async fn scalar_i64(db: &DatabaseConnection, statement: Statement) -> TestResult<i64> {
    let row: QueryResult = db.query_one(statement).await?.ok_or("scalar row missing")?;
    Ok(row.try_get("", "value")?)
}
