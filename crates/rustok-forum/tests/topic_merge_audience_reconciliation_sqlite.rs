use std::collections::BTreeSet;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints, ForumError,
    ForumModule, ForumTopicAudiencePolicyService, ForumTopicMergeAudienceOutcome,
    ForumTopicMergeAudienceReconciliationService, ForumTopicMergeService, MergeForumTopicInput,
    ReconcileForumTopicMergeAudienceInput, SetForumTopicAudiencePolicyInput, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, QueryResult,
    Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::Value as JsonValue;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn setup_before_forum_21g() -> TestResult<(
    DatabaseConnection,
    TransactionalEventBus,
    Box<dyn MigrationTrait>,
)> {
    let db_url = format!(
        "sqlite:file:forum_topic_merge_audience_reconciliation_{}?mode=memory&cache=shared",
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
    let mut forum_migrations = ForumModule.migrations();
    let forum_21g_migration = forum_migrations
        .pop()
        .ok_or("FORUM-21G migration missing from Forum registry")?;
    for migration in forum_migrations {
        migration.up(&schema).await?;
    }
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    Ok((db, event_bus, forum_21g_migration))
}

async fn apply_forum_21g(
    db: &DatabaseConnection,
    migration: Box<dyn MigrationTrait>,
) -> TestResult<()> {
    migration.up(&SchemaManager::new(db)).await?;
    Ok(())
}

async fn setup() -> TestResult<(DatabaseConnection, TransactionalEventBus)> {
    let (db, event_bus, migration) = setup_before_forum_21g().await?;
    apply_forum_21g(&db, migration).await?;
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
    suffix: &str,
) -> TestResult<Uuid> {
    Ok(CategoryService::new(db.clone())
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".to_string(),
                name: format!("Merge audience {suffix}"),
                slug: format!("merge-audience-{suffix}"),
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
                title: format!("Merge audience {key}"),
                slug: Some(format!("merge-audience-{key}")),
                body: rustok_api::RichTextDocument::single_paragraph(format!(
                    "Merge audience {key} body"
                )),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id)
}

fn source_constraints(
    allow_user_id: Uuid,
    deny_user_id: Uuid,
    group_id: Uuid,
) -> ForumAudienceConstraints {
    ForumAudienceConstraints {
        roles_any: vec![UserRole::Manager],
        minimum_trust_level: Some(30),
        channel_members_any: vec!["vip".to_string()],
        group_members_any: vec![group_id],
        allow_user_ids: vec![allow_user_id],
        deny_user_ids: vec![deny_user_id],
    }
}

#[tokio::test]
async fn historical_merge_audience_reconciliation_moves_source_only_layer_and_is_idempotent()
-> TestResult<()> {
    let (db, event_bus, forum_21g_migration) = setup_before_forum_21g().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let allow_user_id = Uuid::new_v4();
    let deny_user_id = Uuid::new_v4();
    for user_id in [actor_id, allow_user_id, deny_user_id] {
        insert_user(&db, tenant_id, user_id).await?;
    }
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = create_category(&db, tenant_id, admin.clone(), "history-move").await?;
    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "history-move-target",
    )
    .await?;
    let source_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "history-move-source",
    )
    .await?;

    let constraints =
        source_constraints(allow_user_id, deny_user_id, Uuid::new_v4()).normalize()?;
    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            source_topic_id,
            admin.clone(),
            SetForumTopicAudiencePolicyInput {
                constraints: constraints.clone(),
            },
        )
        .await?;
    let source_updated_at = policy_updated_at(&db, tenant_id, source_topic_id)
        .await?
        .ok_or("source audience policy timestamp missing")?;

    let merge_operation_id = Uuid::new_v4();
    ForumTopicMergeService::new(db.clone(), event_bus.clone())
        .merge_topic(
            tenant_id,
            target_topic_id,
            admin.clone(),
            MergeForumTopicInput {
                operation_id: merge_operation_id,
                source_topic_id,
                reason: "Create a pre-FORUM-21G historical merge receipt".to_string(),
            },
        )
        .await?;
    apply_forum_21g(&db, forum_21g_migration).await?;

    let stale_owner_write = ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            source_topic_id,
            admin.clone(),
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints::default(),
            },
        )
        .await;
    assert!(matches!(stale_owner_write, Err(ForumError::Validation(_))));
    assert_archived_audience_database_guard(&db, tenant_id, source_topic_id).await?;

    let baseline_projection_ids = projection_root_ids(&db, tenant_id).await?;
    let operation_id = Uuid::new_v4();
    let input = ReconcileForumTopicMergeAudienceInput {
        operation_id,
        reason: "Repair the historical source-only audience layer".to_string(),
    };
    let service = ForumTopicMergeAudienceReconciliationService::new(db.clone(), event_bus.clone());
    let reconciled = service
        .reconcile_merge_audience(tenant_id, merge_operation_id, admin.clone(), input.clone())
        .await?;

    assert_eq!(reconciled.operation_id, operation_id);
    assert_eq!(reconciled.event_id, operation_id);
    assert_eq!(reconciled.merge_operation_id, merge_operation_id);
    assert_eq!(reconciled.source_topic_id, source_topic_id);
    assert_eq!(reconciled.target_topic_id, target_topic_id);
    assert_eq!(reconciled.actor_id, actor_id);
    assert_eq!(
        reconciled.outcome,
        ForumTopicMergeAudienceOutcome::SourceOnlyMoved
    );

    assert_source_audience_empty(&db, tenant_id, source_topic_id).await?;
    let target_policy = ForumTopicAudiencePolicyService::new(db.clone())
        .get(tenant_id, target_topic_id, admin.clone())
        .await?;
    assert_eq!(
        target_policy.configured_constraints,
        Some(constraints.clone())
    );
    assert_eq!(
        policy_updated_at(&db, tenant_id, target_topic_id).await?,
        Some(source_updated_at)
    );
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
        .reconcile_merge_audience(tenant_id, merge_operation_id, admin.clone(), input.clone())
        .await?;
    assert_eq!(replay, reconciled);
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);
    assert_eq!(
        projection_root_ids(&db, tenant_id).await?,
        projection_ids_after
    );

    let drift = service
        .reconcile_merge_audience(
            tenant_id,
            merge_operation_id,
            admin.clone(),
            ReconcileForumTopicMergeAudienceInput {
                operation_id,
                reason: "Changed audience reconciliation command".to_string(),
            },
        )
        .await;
    assert!(matches!(
        drift,
        Err(ForumError::TopicMergeAudienceReconciliationConflict(id)) if id == operation_id
    ));

    let second_operation = service
        .reconcile_merge_audience(
            tenant_id,
            merge_operation_id,
            admin,
            ReconcileForumTopicMergeAudienceInput {
                operation_id: Uuid::new_v4(),
                reason: "A merge may reconcile audience only once".to_string(),
            },
        )
        .await;
    assert!(matches!(
        second_operation,
        Err(ForumError::TopicMergeAudienceReconciliationConflict(_))
    ));

    assert!(db
        .execute_unprepared(&format!(
            "UPDATE forum_topic_merge_audience_reconciliations SET reason = 'tampered' WHERE tenant_id = '{tenant_id}' AND operation_id = '{operation_id}'"
        ))
        .await
        .is_err());
    assert!(db
        .execute_unprepared(&format!(
            "DELETE FROM forum_topic_merge_audience_reconciliations WHERE tenant_id = '{tenant_id}' AND operation_id = '{operation_id}'"
        ))
        .await
        .is_err());
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 1);
    Ok(())
}

#[tokio::test]
async fn historical_merge_audience_reconciliation_rejects_different_dual_layers_atomically()
-> TestResult<()> {
    let (db, event_bus, forum_21g_migration) = setup_before_forum_21g().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let source_user_id = Uuid::new_v4();
    let target_user_id = Uuid::new_v4();
    for user_id in [actor_id, source_user_id, target_user_id] {
        insert_user(&db, tenant_id, user_id).await?;
    }
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = create_category(&db, tenant_id, admin.clone(), "history-conflict").await?;
    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "history-conflict-target",
    )
    .await?;
    let source_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "history-conflict-source",
    )
    .await?;

    let source_constraints = ForumAudienceConstraints {
        allow_user_ids: vec![source_user_id],
        ..ForumAudienceConstraints::default()
    }
    .normalize()?;
    let target_constraints = ForumAudienceConstraints {
        allow_user_ids: vec![target_user_id],
        ..ForumAudienceConstraints::default()
    }
    .normalize()?;
    let audience = ForumTopicAudiencePolicyService::new(db.clone());
    audience
        .set(
            tenant_id,
            source_topic_id,
            admin.clone(),
            SetForumTopicAudiencePolicyInput {
                constraints: source_constraints.clone(),
            },
        )
        .await?;
    audience
        .set(
            tenant_id,
            target_topic_id,
            admin.clone(),
            SetForumTopicAudiencePolicyInput {
                constraints: target_constraints.clone(),
            },
        )
        .await?;

    let merge_operation_id = Uuid::new_v4();
    ForumTopicMergeService::new(db.clone(), event_bus.clone())
        .merge_topic(
            tenant_id,
            target_topic_id,
            admin.clone(),
            MergeForumTopicInput {
                operation_id: merge_operation_id,
                source_topic_id,
                reason: "Create a conflicting pre-FORUM-21G historical merge".to_string(),
            },
        )
        .await?;
    apply_forum_21g(&db, forum_21g_migration).await?;

    let baseline_projection_ids = projection_root_ids(&db, tenant_id).await?;
    let event_count_before = reconciliation_event_count(&db, tenant_id).await?;
    let result = ForumTopicMergeAudienceReconciliationService::new(db.clone(), event_bus)
        .reconcile_merge_audience(
            tenant_id,
            merge_operation_id,
            admin.clone(),
            ReconcileForumTopicMergeAudienceInput {
                operation_id: Uuid::new_v4(),
                reason: "Do not broaden two different historical local layers".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(ForumError::TopicMergeAudiencePolicyConflict(id)) if id == merge_operation_id
    ));
    assert_eq!(reconciliation_count(&db, tenant_id).await?, 0);
    assert_eq!(
        reconciliation_event_count(&db, tenant_id).await?,
        event_count_before
    );
    assert_eq!(
        projection_root_ids(&db, tenant_id).await?,
        baseline_projection_ids
    );

    let source_after = ForumTopicAudiencePolicyService::new(db.clone())
        .get(tenant_id, source_topic_id, admin.clone())
        .await?;
    let target_after = ForumTopicAudiencePolicyService::new(db.clone())
        .get(tenant_id, target_topic_id, admin)
        .await?;
    assert_eq!(
        source_after.configured_constraints,
        Some(source_constraints)
    );
    assert_eq!(
        target_after.configured_constraints,
        Some(target_constraints)
    );
    Ok(())
}

#[tokio::test]
async fn topic_merge_rejects_incompatible_source_audience_before_commit() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let source_user_id = Uuid::new_v4();
    for user_id in [actor_id, source_user_id] {
        insert_user(&db, tenant_id, user_id).await?;
    }
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = create_category(&db, tenant_id, admin.clone(), "guard").await?;
    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "guard-target",
    )
    .await?;
    let source_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "guard-source",
    )
    .await?;
    let source_constraints = ForumAudienceConstraints {
        allow_user_ids: vec![source_user_id],
        ..ForumAudienceConstraints::default()
    }
    .normalize()?;
    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            source_topic_id,
            admin.clone(),
            SetForumTopicAudiencePolicyInput {
                constraints: source_constraints.clone(),
            },
        )
        .await?;

    let baseline_projection_ids = projection_root_ids(&db, tenant_id).await?;
    let merge_event_count_before = merge_event_count(&db, tenant_id).await?;
    let merge_operation_id = Uuid::new_v4();
    let result = ForumTopicMergeService::new(db.clone(), event_bus)
        .merge_topic(
            tenant_id,
            target_topic_id,
            admin.clone(),
            MergeForumTopicInput {
                operation_id: merge_operation_id,
                source_topic_id,
                reason: "This merge must roll back before commit".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(ForumError::TopicMergeAudiencePolicyConflict(_))
    ));

    assert_eq!(
        merge_receipt_count(&db, tenant_id, merge_operation_id).await?,
        0
    );
    assert_eq!(
        merge_event_count(&db, tenant_id).await?,
        merge_event_count_before
    );
    assert_eq!(
        projection_root_ids(&db, tenant_id).await?,
        baseline_projection_ids
    );
    assert_eq!(topic_status(&db, tenant_id, source_topic_id).await?, "open");
    assert_eq!(topic_status(&db, tenant_id, target_topic_id).await?, "open");

    let source_after = ForumTopicAudiencePolicyService::new(db.clone())
        .get(tenant_id, source_topic_id, admin.clone())
        .await?;
    let target_after = ForumTopicAudiencePolicyService::new(db)
        .get(tenant_id, target_topic_id, admin)
        .await?;
    assert_eq!(
        source_after.configured_constraints,
        Some(source_constraints)
    );
    assert_eq!(target_after.configured_constraints, None);
    Ok(())
}

#[tokio::test]
async fn merge_audience_reconciliation_requires_a_real_merge_receipt() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let result = ForumTopicMergeAudienceReconciliationService::new(db, event_bus)
        .reconcile_merge_audience(
            tenant_id,
            Uuid::new_v4(),
            SecurityContext::new(UserRole::Admin, Some(actor_id)),
            ReconcileForumTopicMergeAudienceInput {
                operation_id: Uuid::new_v4(),
                reason: "Missing merge receipts must fail closed".to_string(),
            },
        )
        .await;
    assert!(matches!(result, Err(ForumError::Validation(_))));
    Ok(())
}

async fn assert_archived_audience_database_guard(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    source_topic_id: Uuid,
) -> TestResult<()> {
    let result = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO forum_topic_audience_roles (tenant_id, topic_id, role) VALUES (?, ?, 'admin')",
            vec![tenant_id.into(), source_topic_id.into()],
        ))
        .await;
    assert!(result.is_err());
    Ok(())
}

async fn assert_source_audience_empty(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    source_topic_id: Uuid,
) -> TestResult<()> {
    for table in [
        "forum_topic_audience_policies",
        "forum_topic_audience_roles",
        "forum_topic_audience_channels",
        "forum_topic_audience_groups",
        "forum_topic_audience_users",
    ] {
        let count = scalar_i64(
            db,
            Statement::from_sql_and_values(
                DbBackend::Sqlite,
                format!(
                    "SELECT COUNT(*) AS value FROM {table} WHERE tenant_id = ? AND topic_id = ?"
                ),
                vec![tenant_id.into(), source_topic_id.into()],
            ),
        )
        .await?;
        assert_eq!(count, 0, "source audience table is not empty: {table}");
    }
    Ok(())
}

async fn policy_updated_at(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<Option<String>> {
    Ok(db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT updated_at FROM forum_topic_audience_policies WHERE tenant_id = ? AND topic_id = ?",
            vec![tenant_id.into(), topic_id.into()],
        ))
        .await?
        .map(|row| row.try_get("", "updated_at"))
        .transpose()?)
}

async fn topic_status(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<String> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT status FROM forum_topics WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), topic_id.into()],
        ))
        .await?
        .ok_or("topic row missing")?;
    Ok(row.try_get("", "status")?)
}

async fn merge_receipt_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS value FROM forum_topic_merge_operations WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        ),
    )
    .await
}

async fn merge_event_count(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS value FROM forum_domain_events WHERE tenant_id = ? AND event_type = 'forum.topic.merged'",
            vec![tenant_id.into()],
        ),
    )
    .await
}

async fn reconciliation_count(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS value FROM forum_topic_merge_audience_reconciliations WHERE tenant_id = ?",
            vec![tenant_id.into()],
        ),
    )
    .await
}

async fn reconciliation_event_count(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS value FROM forum_domain_events WHERE tenant_id = ? AND event_type = 'forum.topic.merge_audience_reconciled'",
            vec![tenant_id.into()],
        ),
    )
    .await
}

async fn assert_reconciliation_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    reconciled: &rustok_forum::ForumTopicMergeAudienceReconciliationResult,
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
        .ok_or("audience reconciliation semantic event missing")?;
    assert_eq!(row.try_get::<String>("", "aggregate_type")?, "forum_topic");
    assert_eq!(
        row.try_get::<Uuid>("", "aggregate_id")?,
        reconciled.target_topic_id
    );
    assert_eq!(
        row.try_get::<String>("", "event_type")?,
        "forum.topic.merge_audience_reconciled"
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
    assert_eq!(payload["outcome"], "source_only_moved");
    assert_eq!(payload["reason"], reconciled.reason);
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
        .query_all(Statement::from_sql_and_values(
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
    let row: QueryResult = db.query_one(statement).await?.ok_or("scalar row missing")?;
    Ok(row.try_get("", "value")?)
}
