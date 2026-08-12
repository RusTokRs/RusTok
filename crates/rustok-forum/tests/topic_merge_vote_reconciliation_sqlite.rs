use std::collections::BTreeMap;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumError, ForumModule,
    ForumTopicMergeService, ForumTopicMergeVoteReconciliationService, MergeForumTopicInput,
    ReconcileForumTopicMergeVotesInput, TopicService, VoteService,
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct VoteSnapshot {
    topic_id: Uuid,
    user_id: Uuid,
    value: i32,
    created_at: String,
    updated_at: String,
}

async fn setup() -> TestResult<(DatabaseConnection, TransactionalEventBus)> {
    let db_url = format!(
        "sqlite:file:forum_topic_merge_vote_reconciliation_{}?mode=memory&cache=shared",
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
                name: "Merge votes".to_string(),
                slug: "merge-votes".to_string(),
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
                title: format!("Merge votes {key}"),
                slug: Some(format!("merge-votes-{key}")),
                body: rustok_api::RichTextDocument::single_paragraph(format!(
                    "Merge votes {key} body"
                )),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id)
}

#[tokio::test]
async fn merge_vote_reconciliation_is_atomic_idempotent_and_target_authoritative() -> TestResult<()>
{
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

    let votes = VoteService::new(db.clone());
    votes
        .set_topic_vote(
            tenant_id,
            source_topic_id,
            SecurityContext::new(UserRole::Admin, Some(source_only_user)),
            1,
        )
        .await?;
    votes
        .set_topic_vote(
            tenant_id,
            target_topic_id,
            SecurityContext::new(UserRole::Admin, Some(target_only_user)),
            -1,
        )
        .await?;
    votes
        .set_topic_vote(
            tenant_id,
            source_topic_id,
            SecurityContext::new(UserRole::Admin, Some(equal_user)),
            1,
        )
        .await?;
    votes
        .set_topic_vote(
            tenant_id,
            target_topic_id,
            SecurityContext::new(UserRole::Admin, Some(equal_user)),
            1,
        )
        .await?;
    votes
        .set_topic_vote(
            tenant_id,
            source_topic_id,
            SecurityContext::new(UserRole::Admin, Some(conflict_user)),
            1,
        )
        .await?;
    votes
        .set_topic_vote(
            tenant_id,
            target_topic_id,
            SecurityContext::new(UserRole::Admin, Some(conflict_user)),
            -1,
        )
        .await?;

    let source_before = vote_snapshots(&db, tenant_id, source_topic_id).await?;
    let target_before = vote_snapshots(&db, tenant_id, target_topic_id).await?;
    assert_eq!(source_before.len(), 3);
    assert_eq!(target_before.len(), 3);
    let moved_before = source_before
        .get(&source_only_user)
        .cloned()
        .ok_or("source-only vote missing before merge")?;

    let merge_operation_id = Uuid::new_v4();
    ForumTopicMergeService::new(db.clone(), event_bus)
        .merge_topic(
            tenant_id,
            target_topic_id,
            admin.clone(),
            MergeForumTopicInput {
                operation_id: merge_operation_id,
                source_topic_id,
                reason: "Merge duplicate topic before vote reconciliation".to_string(),
            },
        )
        .await?;

    let source_vote_write = VoteService::new(db.clone())
        .set_topic_vote(
            tenant_id,
            source_topic_id,
            SecurityContext::new(UserRole::Admin, Some(source_only_user)),
            -1,
        )
        .await;
    assert!(matches!(source_vote_write, Err(ForumError::Validation(_))));
    let source_vote_clear = VoteService::new(db.clone())
        .clear_topic_vote(
            tenant_id,
            source_topic_id,
            SecurityContext::new(UserRole::Admin, Some(source_only_user)),
        )
        .await;
    assert!(matches!(source_vote_clear, Err(ForumError::Validation(_))));
    assert_archived_vote_database_guards(
        &db,
        tenant_id,
        source_topic_id,
        source_only_user,
        raw_guard_user,
    )
    .await?;

    let operation_id = Uuid::new_v4();
    let input = ReconcileForumTopicMergeVotesInput {
        operation_id,
        reason: "Retain target voter authority after topic merge".to_string(),
    };
    let service = ForumTopicMergeVoteReconciliationService::new(db.clone());
    let reconciled = service
        .reconcile_merge_votes(tenant_id, merge_operation_id, admin.clone(), input.clone())
        .await?;

    assert_eq!(reconciled.operation_id, operation_id);
    assert_eq!(reconciled.event_id, operation_id);
    assert_eq!(reconciled.merge_operation_id, merge_operation_id);
    assert_eq!(reconciled.source_topic_id, source_topic_id);
    assert_eq!(reconciled.target_topic_id, target_topic_id);
    assert_eq!(reconciled.actor_id, actor_id);
    assert_eq!(reconciled.source_vote_count, 3);
    assert_eq!(reconciled.moved_source_only_count, 1);
    assert_eq!(reconciled.deduplicated_equal_count, 1);
    assert_eq!(reconciled.target_authority_conflict_count, 1);

    assert_eq!(vote_count(&db, tenant_id, source_topic_id).await?, 0);
    let target_after = vote_snapshots(&db, tenant_id, target_topic_id).await?;
    assert_eq!(target_after.len(), 4);
    let moved_after = target_after
        .get(&source_only_user)
        .ok_or("source-only vote missing after reconciliation")?;
    assert_eq!(moved_after.topic_id, target_topic_id);
    assert_eq!(moved_after.user_id, moved_before.user_id);
    assert_eq!(moved_after.value, moved_before.value);
    assert_eq!(moved_after.created_at, moved_before.created_at);
    assert_eq!(moved_after.updated_at, moved_before.updated_at);
    for retained_user in [target_only_user, equal_user, conflict_user] {
        assert_eq!(
            target_after.get(&retained_user),
            target_before.get(&retained_user),
            "retained target vote changed for {retained_user}"
        );
    }

    let summary = VoteService::new(db.clone())
        .topic_vote_summary(tenant_id, target_topic_id, Some(conflict_user))
        .await?;
    assert_eq!(summary.score, 0);
    assert_eq!(summary.current_user_vote, Some(-1));
    assert_reconciliation_event(&db, tenant_id, &reconciled).await?;
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);

    let replay = service
        .reconcile_merge_votes(tenant_id, merge_operation_id, admin.clone(), input.clone())
        .await?;
    assert_eq!(replay, reconciled);
    assert_eq!(
        vote_snapshots(&db, tenant_id, target_topic_id).await?,
        target_after
    );
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);

    let drift = service
        .reconcile_merge_votes(
            tenant_id,
            merge_operation_id,
            admin.clone(),
            ReconcileForumTopicMergeVotesInput {
                operation_id,
                reason: "Changed vote reconciliation command".to_string(),
            },
        )
        .await;
    assert!(matches!(
        drift,
        Err(ForumError::TopicMergeVoteReconciliationConflict(id)) if id == operation_id
    ));

    let second_operation = service
        .reconcile_merge_votes(
            tenant_id,
            merge_operation_id,
            admin,
            ReconcileForumTopicMergeVotesInput {
                operation_id: Uuid::new_v4(),
                reason: "A merge may reconcile votes only once".to_string(),
            },
        )
        .await;
    assert!(matches!(
        second_operation,
        Err(ForumError::TopicMergeVoteReconciliationConflict(_))
    ));

    assert!(db
        .execute_unprepared(&format!(
            "UPDATE forum_topic_merge_vote_reconciliations SET reason = 'tampered' WHERE tenant_id = '{tenant_id}' AND operation_id = '{operation_id}'"
        ))
        .await
        .is_err());
    assert!(db
        .execute_unprepared(&format!(
            "DELETE FROM forum_topic_merge_vote_reconciliations WHERE tenant_id = '{tenant_id}' AND operation_id = '{operation_id}'"
        ))
        .await
        .is_err());
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);
    Ok(())
}

#[tokio::test]
async fn merge_vote_reconciliation_requires_a_real_merge_receipt() -> TestResult<()> {
    let (db, _event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let result = ForumTopicMergeVoteReconciliationService::new(db)
        .reconcile_merge_votes(
            tenant_id,
            Uuid::new_v4(),
            SecurityContext::new(UserRole::Admin, Some(actor_id)),
            ReconcileForumTopicMergeVotesInput {
                operation_id: Uuid::new_v4(),
                reason: "Missing merge receipts must fail closed".to_string(),
            },
        )
        .await;
    assert!(matches!(result, Err(ForumError::Validation(_))));
    Ok(())
}

async fn assert_archived_vote_database_guards(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    existing_user_id: Uuid,
    new_user_id: Uuid,
) -> TestResult<()> {
    let update = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE forum_topic_votes SET value = value WHERE tenant_id = ? AND topic_id = ? AND user_id = ?",
            vec![tenant_id.into(), source_topic_id.into(), existing_user_id.into()],
        ))
        .await;
    assert!(update.is_err());

    let insert = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO forum_topic_votes (topic_id, user_id, tenant_id, value, created_at, updated_at) VALUES (?, ?, ?, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![source_topic_id.into(), new_user_id.into(), tenant_id.into()],
        ))
        .await;
    assert!(insert.is_err());
    Ok(())
}

async fn vote_snapshots(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<BTreeMap<Uuid, VoteSnapshot>> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT topic_id, user_id, value, created_at, updated_at
            FROM forum_topic_votes
            WHERE tenant_id = ? AND topic_id = ?
            ORDER BY user_id
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ))
        .await?;
    let mut snapshots = BTreeMap::new();
    for row in rows {
        let user_id = row.try_get("", "user_id")?;
        snapshots.insert(
            user_id,
            VoteSnapshot {
                topic_id: row.try_get("", "topic_id")?,
                user_id,
                value: row.try_get("", "value")?,
                created_at: row.try_get("", "created_at")?,
                updated_at: row.try_get("", "updated_at")?,
            },
        );
    }
    Ok(snapshots)
}

async fn vote_count(db: &DatabaseConnection, tenant_id: Uuid, topic_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS value FROM forum_topic_votes WHERE tenant_id = ? AND topic_id = ?",
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
            "SELECT COUNT(*) AS value FROM forum_topic_merge_vote_reconciliations WHERE tenant_id = ?",
            vec![tenant_id.into()],
        ),
    )
    .await
}

async fn assert_reconciliation_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    reconciled: &rustok_forum::ForumTopicMergeVoteReconciliationResult,
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
        .ok_or("vote reconciliation semantic event missing")?;
    assert_eq!(row.try_get::<String>("", "aggregate_type")?, "forum_topic");
    assert_eq!(
        row.try_get::<Uuid>("", "aggregate_id")?,
        reconciled.target_topic_id
    );
    assert_eq!(
        row.try_get::<String>("", "event_type")?,
        "forum.topic.merge_votes_reconciled"
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
    assert_eq!(payload["source_vote_count"], reconciled.source_vote_count);
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
