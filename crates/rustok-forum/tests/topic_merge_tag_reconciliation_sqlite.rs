use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumError, ForumModule,
    ForumTopicMergeService, ForumTopicMergeTagReconciliationService, MergeForumTopicInput,
    ReconcileForumTopicMergeTagsInput, TopicService, UpdateTopicInput,
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
struct TagSnapshot {
    id: Uuid,
    topic_id: Uuid,
    term_id: Uuid,
    created_at: String,
}

async fn setup() -> TestResult<(DatabaseConnection, TransactionalEventBus)> {
    let db_url = format!(
        "sqlite:file:forum_topic_merge_tag_reconciliation_{}?mode=memory&cache=shared",
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
                name: "Merge tags".to_string(),
                slug: "merge-tags".to_string(),
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
    tags: &[&str],
) -> TestResult<Uuid> {
    Ok(TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id,
                title: format!("Merge tags {key}"),
                slug: Some(format!("merge-tags-{key}")),
                body: rustok_api::RichTextDocument::single_paragraph(format!(
                    "Merge tags {key} body"
                )),
                metadata: serde_json::json!({}),
                tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
                channel_slugs: None,
            },
        )
        .await?
        .id)
}

#[tokio::test]
async fn merge_tag_reconciliation_is_atomic_idempotent_and_preserves_relation_identity()
-> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = create_category(&db, tenant_id, admin.clone()).await?;
    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "target",
        &["shared", "target-only", "rust"],
    )
    .await?;
    let source_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "source",
        &["rust", "shared", "source-only"],
    )
    .await?;

    let source_before = tag_snapshots(&db, tenant_id, source_topic_id).await?;
    let target_before = tag_snapshots(&db, tenant_id, target_topic_id).await?;
    assert_eq!(source_before.len(), 3);
    assert_eq!(target_before.len(), 3);
    let moved_before = source_before
        .get("source-only")
        .cloned()
        .ok_or("source-only relation missing before merge")?;

    let merge_operation_id = Uuid::new_v4();
    ForumTopicMergeService::new(db.clone(), event_bus.clone())
        .merge_topic(
            tenant_id,
            target_topic_id,
            admin.clone(),
            MergeForumTopicInput {
                operation_id: merge_operation_id,
                source_topic_id,
                reason: "Merge duplicate topic before tag reconciliation".to_string(),
            },
        )
        .await?;

    let stale_service_update = TopicService::new(db.clone(), event_bus.clone())
        .update(
            tenant_id,
            source_topic_id,
            admin.clone(),
            UpdateTopicInput {
                locale: "en".to_string(),
                title: None,
                body: None,
                metadata: None,
                tags: Some(vec!["changed-after-merge".to_string()]),
                channel_slugs: None,
            },
        )
        .await;
    assert!(matches!(
        stale_service_update,
        Err(ForumError::Validation(_))
    ));
    assert_archived_tag_database_guards(
        &db,
        tenant_id,
        source_topic_id,
        &moved_before,
        target_before
            .get("target-only")
            .ok_or("target-only relation missing")?
            .term_id,
    )
    .await?;

    let baseline_projection_ids = projection_root_ids(&db, tenant_id).await?;
    let operation_id = Uuid::new_v4();
    let input = ReconcileForumTopicMergeTagsInput {
        operation_id,
        reason: "Union taxonomy tag membership after topic merge".to_string(),
    };
    let service = ForumTopicMergeTagReconciliationService::new(db.clone(), event_bus.clone());
    let reconciled = service
        .reconcile_merge_tags(tenant_id, merge_operation_id, admin.clone(), input.clone())
        .await?;

    assert_eq!(reconciled.operation_id, operation_id);
    assert_eq!(reconciled.event_id, operation_id);
    assert_eq!(reconciled.merge_operation_id, merge_operation_id);
    assert_eq!(reconciled.source_topic_id, source_topic_id);
    assert_eq!(reconciled.target_topic_id, target_topic_id);
    assert_eq!(reconciled.actor_id, actor_id);
    assert_eq!(reconciled.source_tag_count, 3);
    assert_eq!(reconciled.moved_source_only_count, 1);
    assert_eq!(reconciled.deduplicated_existing_count, 2);

    assert_eq!(tag_count(&db, tenant_id, source_topic_id).await?, 0);
    let target_after = tag_snapshots(&db, tenant_id, target_topic_id).await?;
    assert_eq!(target_after.len(), 4);
    assert_eq!(
        target_after.keys().cloned().collect::<BTreeSet<_>>(),
        ["rust", "shared", "source-only", "target-only"]
            .into_iter()
            .map(str::to_string)
            .collect()
    );

    let moved_after = target_after
        .get("source-only")
        .ok_or("source-only relation missing after reconciliation")?;
    assert_eq!(moved_after.id, moved_before.id);
    assert_eq!(moved_after.term_id, moved_before.term_id);
    assert_eq!(moved_after.created_at, moved_before.created_at);
    assert_eq!(moved_after.topic_id, target_topic_id);
    for retained_key in ["rust", "shared", "target-only"] {
        assert_eq!(
            target_after.get(retained_key),
            target_before.get(retained_key),
            "retained target relation changed for {retained_key}"
        );
    }

    assert_reconciliation_event(&db, tenant_id, &reconciled).await?;
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);
    let projection_ids_after = projection_root_ids(&db, tenant_id).await?;
    let new_projection_ids = projection_ids_after
        .difference(&baseline_projection_ids)
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(new_projection_ids.len(), 2);
    assert_eq!(
        projection_targets(&db, tenant_id, &new_projection_ids).await?,
        [
            ("forum_topic".to_string(), Some(source_topic_id)),
            ("forum_topic".to_string(), Some(target_topic_id)),
        ]
        .into_iter()
        .collect()
    );

    let replay = service
        .reconcile_merge_tags(tenant_id, merge_operation_id, admin.clone(), input.clone())
        .await?;
    assert_eq!(replay, reconciled);
    assert_eq!(
        tag_snapshots(&db, tenant_id, target_topic_id).await?,
        target_after
    );
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);
    assert_eq!(
        projection_root_ids(&db, tenant_id).await?,
        projection_ids_after
    );

    let drift = service
        .reconcile_merge_tags(
            tenant_id,
            merge_operation_id,
            admin.clone(),
            ReconcileForumTopicMergeTagsInput {
                operation_id,
                reason: "Changed tag reconciliation command".to_string(),
            },
        )
        .await;
    assert!(matches!(
        drift,
        Err(ForumError::TopicMergeTagReconciliationConflict(id)) if id == operation_id
    ));

    let second_operation = service
        .reconcile_merge_tags(
            tenant_id,
            merge_operation_id,
            admin,
            ReconcileForumTopicMergeTagsInput {
                operation_id: Uuid::new_v4(),
                reason: "A merge may reconcile tags only once".to_string(),
            },
        )
        .await;
    assert!(matches!(
        second_operation,
        Err(ForumError::TopicMergeTagReconciliationConflict(_))
    ));

    assert!(db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE forum_topic_merge_tag_reconciliations SET reason = 'tampered' WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        ))
        .await
        .is_err());
    assert!(db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "DELETE FROM forum_topic_merge_tag_reconciliations WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        ))
        .await
        .is_err());
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);
    Ok(())
}

#[tokio::test]
async fn merge_tag_reconciliation_requires_a_real_merge_receipt() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let result = ForumTopicMergeTagReconciliationService::new(db, event_bus)
        .reconcile_merge_tags(
            tenant_id,
            Uuid::new_v4(),
            SecurityContext::new(UserRole::Admin, Some(actor_id)),
            ReconcileForumTopicMergeTagsInput {
                operation_id: Uuid::new_v4(),
                reason: "Missing merge receipts must fail closed".to_string(),
            },
        )
        .await;
    assert!(matches!(result, Err(ForumError::Validation(_))));
    Ok(())
}

async fn assert_archived_tag_database_guards(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    existing: &TagSnapshot,
    new_term_id: Uuid,
) -> TestResult<()> {
    let update = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE forum_topic_tags SET created_at = created_at WHERE id = ? AND tenant_id = ?",
            vec![existing.id.into(), tenant_id.into()],
        ))
        .await;
    assert!(update.is_err());

    let insert = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO forum_topic_tags (id, topic_id, term_id, tenant_id, created_at) VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)",
            vec![
                Uuid::new_v4().into(),
                source_topic_id.into(),
                new_term_id.into(),
                tenant_id.into(),
            ],
        ))
        .await;
    assert!(insert.is_err());
    Ok(())
}

async fn tag_snapshots(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<BTreeMap<String, TagSnapshot>> {
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT term.canonical_key, tag.id, tag.topic_id, tag.term_id, tag.created_at
            FROM forum_topic_tags tag
            JOIN taxonomy_terms term
              ON term.tenant_id = tag.tenant_id
             AND term.id = tag.term_id
            WHERE tag.tenant_id = ? AND tag.topic_id = ?
            ORDER BY term.canonical_key
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ))
        .await?;
    let mut snapshots = BTreeMap::new();
    for row in rows {
        snapshots.insert(
            row.try_get("", "canonical_key")?,
            TagSnapshot {
                id: row.try_get("", "id")?,
                topic_id: row.try_get("", "topic_id")?,
                term_id: row.try_get("", "term_id")?,
                created_at: row.try_get("", "created_at")?,
            },
        );
    }
    Ok(snapshots)
}

async fn tag_count(db: &DatabaseConnection, tenant_id: Uuid, topic_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS value FROM forum_topic_tags WHERE tenant_id = ? AND topic_id = ?",
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
            "SELECT COUNT(*) AS value FROM forum_topic_merge_tag_reconciliations WHERE tenant_id = ?",
            vec![tenant_id.into()],
        ),
    )
    .await
}

async fn assert_reconciliation_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    reconciled: &rustok_forum::ForumTopicMergeTagReconciliationResult,
) -> TestResult<()> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
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
        .ok_or("tag reconciliation semantic event missing")?;
    assert_eq!(row.try_get::<String>("", "aggregate_type")?, "forum_topic");
    assert_eq!(
        row.try_get::<Uuid>("", "aggregate_id")?,
        reconciled.target_topic_id
    );
    assert_eq!(
        row.try_get::<String>("", "event_type")?,
        "forum.topic.merge.tags_reconciled"
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
    assert_eq!(payload["source_tag_count"], reconciled.source_tag_count);
    assert_eq!(
        payload["moved_source_only_count"],
        reconciled.moved_source_only_count
    );
    assert_eq!(
        payload["deduplicated_existing_count"],
        reconciled.deduplicated_existing_count
    );
    assert_eq!(payload["reason"], reconciled.reason);
    Ok(())
}

async fn projection_root_ids(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<BTreeSet<Uuid>> {
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT payload FROM sys_events WHERE event_type = 'index.reindex_requested'",
            Vec::new(),
        ))
        .await?;
    let mut ids = BTreeSet::new();
    for row in rows {
        let envelope: EventEnvelope = serde_json::from_value(row.try_get("", "payload")?)?;
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
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT payload FROM sys_events WHERE event_type = 'index.reindex_requested'",
            Vec::new(),
        ))
        .await?;
    let mut targets = BTreeSet::new();
    for row in rows {
        let envelope: EventEnvelope = serde_json::from_value(row.try_get("", "payload")?)?;
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
    let row: QueryResult = db.query_one_raw(statement).await?.ok_or("scalar row missing")?;
    Ok(row.try_get("", "value")?)
}
