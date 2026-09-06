use std::collections::BTreeSet;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumError,
    ForumModule, ForumTopicMergeResult, ForumTopicMergeService, MergeForumTopicInput, ReplyService,
    TopicService,
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
        "sqlite:file:forum_topic_merge_cross_category_{}?mode=memory&cache=shared",
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
    key: &str,
) -> TestResult<Uuid> {
    Ok(TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id,
                title: format!("Cross-category merge {key}"),
                slug: Some(format!("cross-category-merge-{key}")),
                body: rustok_api::RichTextDocument::single_paragraph(format!(
                    "Cross-category merge {key} body"
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
    text: &str,
    parent_reply_id: Option<Uuid>,
) -> TestResult<Uuid> {
    Ok(ReplyService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            topic_id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph(text),
                parent_reply_id,
            },
        )
        .await?
        .id)
}

#[tokio::test]
async fn cross_category_topic_merge_transfers_published_reply_counters_once() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));

    let source_category_id = create_category(&db, tenant_id, admin.clone(), "cross-source").await?;
    let target_category_id = create_category(&db, tenant_id, admin.clone(), "cross-target").await?;
    let source_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        source_category_id,
        admin.clone(),
        "source",
    )
    .await?;
    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        target_category_id,
        admin.clone(),
        "target",
    )
    .await?;
    let target_reply_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        target_topic_id,
        admin.clone(),
        "Target reply",
        None,
    )
    .await?;
    let source_root_reply_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Source root reply",
        None,
    )
    .await?;
    let source_child_reply_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Source child reply",
        Some(source_root_reply_id),
    )
    .await?;

    assert_category_counters(&db, tenant_id, source_category_id, 1, 2).await?;
    assert_category_counters(&db, tenant_id, target_category_id, 1, 1).await?;
    let baseline_projection_ids = projection_root_ids(&db, tenant_id).await?;
    let operation_id = Uuid::new_v4();
    let input = MergeForumTopicInput {
        operation_id,
        source_topic_id,
        reason: "Consolidate discussions under the retained target category".to_string(),
    };
    let service = ForumTopicMergeService::new(db.clone(), event_bus);
    let merged = service
        .merge_topic(tenant_id, target_topic_id, admin.clone(), input.clone())
        .await?;

    assert_eq!(merged.operation_id, operation_id);
    assert_eq!(merged.event_id, operation_id);
    assert_eq!(merged.source_topic_id, source_topic_id);
    assert_eq!(merged.target_topic_id, target_topic_id);
    assert_eq!(merged.category_id, target_category_id);
    assert_eq!(merged.moved_reply_count, 2);
    assert_eq!(merged.moved_published_reply_count, 2);
    assert_eq!(merged.resulting_published_reply_count, 3);
    assert_eq!(merged.position_offset, 1);

    assert_topic_state(
        &db,
        tenant_id,
        source_topic_id,
        source_category_id,
        "archived",
        true,
        0,
    )
    .await?;
    assert_topic_state(
        &db,
        tenant_id,
        target_topic_id,
        target_category_id,
        "open",
        false,
        3,
    )
    .await?;
    assert_category_counters(&db, tenant_id, source_category_id, 1, 0).await?;
    assert_category_counters(&db, tenant_id, target_category_id, 1, 3).await?;
    assert_reply_location(&db, tenant_id, target_reply_id, target_topic_id, 1, None).await?;
    assert_reply_location(
        &db,
        tenant_id,
        source_root_reply_id,
        target_topic_id,
        2,
        None,
    )
    .await?;
    assert_reply_location(
        &db,
        tenant_id,
        source_child_reply_id,
        target_topic_id,
        3,
        Some(source_root_reply_id),
    )
    .await?;
    assert_merge_event(&db, tenant_id, &merged).await?;
    assert_eq!(merge_operation_count(&db, tenant_id).await?, 1);

    let projection_ids_after_merge = projection_root_ids(&db, tenant_id).await?;
    let new_projection_ids = projection_ids_after_merge
        .difference(&baseline_projection_ids)
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(new_projection_ids.len(), 4);
    assert_eq!(
        projection_targets(&db, tenant_id, &new_projection_ids).await?,
        [
            ("forum_topic".to_string(), Some(source_topic_id)),
            ("forum_topic".to_string(), Some(target_topic_id)),
            ("forum_category".to_string(), Some(source_category_id)),
            ("forum_category".to_string(), Some(target_category_id)),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>()
    );

    let replay = service
        .merge_topic(tenant_id, target_topic_id, admin, input)
        .await?;
    assert_eq!(replay, merged);
    assert_category_counters(&db, tenant_id, source_category_id, 1, 0).await?;
    assert_category_counters(&db, tenant_id, target_category_id, 1, 3).await?;
    assert_eq!(merge_operation_count(&db, tenant_id).await?, 1);
    assert_eq!(
        projection_root_ids(&db, tenant_id).await?,
        projection_ids_after_merge
    );
    Ok(())
}

#[tokio::test]
async fn cross_category_topic_merge_rolls_back_on_source_counter_drift() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));

    let source_category_id = create_category(&db, tenant_id, admin.clone(), "drift-source").await?;
    let target_category_id = create_category(&db, tenant_id, admin.clone(), "drift-target").await?;
    let source_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        source_category_id,
        admin.clone(),
        "drift-source",
    )
    .await?;
    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        target_category_id,
        admin.clone(),
        "drift-target",
    )
    .await?;
    let source_reply_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        admin.clone(),
        "Reply with an intentionally corrupted category aggregate",
        None,
    )
    .await?;
    db.execute_unprepared("DROP TRIGGER IF EXISTS forum_categories_public_reply_count_update")
        .await?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "UPDATE forum_categories SET reply_count = 0 WHERE tenant_id = ? AND id = ?",
        vec![tenant_id.into(), source_category_id.into()],
    ))
    .await?;

    let baseline_projection_ids = projection_root_ids(&db, tenant_id).await?;
    let baseline_events = merge_event_count(&db, tenant_id).await?;
    let service = ForumTopicMergeService::new(db.clone(), event_bus);
    let result = service
        .merge_topic(
            tenant_id,
            target_topic_id,
            admin,
            MergeForumTopicInput {
                operation_id: Uuid::new_v4(),
                source_topic_id,
                reason: "Fail closed instead of underflowing the source category".to_string(),
            },
        )
        .await;
    let error = result.expect_err("category counter drift must abort the merge");
    assert_eq!(error.stable_code(), "FORUM_VALIDATION_FAILED");
    assert!(matches!(error, ForumError::Validation(_)));

    assert_topic_state(
        &db,
        tenant_id,
        source_topic_id,
        source_category_id,
        "open",
        false,
        1,
    )
    .await?;
    assert_topic_state(
        &db,
        tenant_id,
        target_topic_id,
        target_category_id,
        "open",
        false,
        0,
    )
    .await?;
    assert_reply_location(&db, tenant_id, source_reply_id, source_topic_id, 1, None).await?;
    assert_category_counters(&db, tenant_id, source_category_id, 1, 0).await?;
    assert_category_counters(&db, tenant_id, target_category_id, 1, 0).await?;
    assert_eq!(merge_operation_count(&db, tenant_id).await?, 0);
    assert_eq!(merge_event_count(&db, tenant_id).await?, baseline_events);
    assert_eq!(
        projection_root_ids(&db, tenant_id).await?,
        baseline_projection_ids
    );
    Ok(())
}

async fn assert_topic_state(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    expected_category_id: Uuid,
    expected_status: &str,
    expected_locked: bool,
    expected_reply_count: i32,
) -> TestResult<()> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT category_id, status, is_locked, reply_count FROM forum_topics WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), topic_id.into()],
        ))
        .await?
        .ok_or("topic row missing")?;
    assert_eq!(
        row.try_get::<Uuid>("", "category_id")?,
        expected_category_id
    );
    assert_eq!(row.try_get::<String>("", "status")?, expected_status);
    assert_eq!(row.try_get::<bool>("", "is_locked")?, expected_locked);
    assert_eq!(row.try_get::<i32>("", "reply_count")?, expected_reply_count);
    Ok(())
}

async fn assert_category_counters(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    expected_topics: i32,
    expected_replies: i32,
) -> TestResult<()> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
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

async fn assert_reply_location(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    reply_id: Uuid,
    expected_topic_id: Uuid,
    expected_position: i64,
    expected_parent_reply_id: Option<Uuid>,
) -> TestResult<()> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT topic_id, position, parent_reply_id FROM forum_replies WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), reply_id.into()],
        ))
        .await?
        .ok_or("reply row missing")?;
    assert_eq!(row.try_get::<Uuid>("", "topic_id")?, expected_topic_id);
    assert_eq!(row.try_get::<i64>("", "position")?, expected_position);
    assert_eq!(
        row.try_get::<Option<Uuid>>("", "parent_reply_id")?,
        expected_parent_reply_id
    );
    Ok(())
}

async fn assert_merge_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    merged: &ForumTopicMergeResult,
) -> TestResult<()> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT aggregate_type, aggregate_id, event_type, schema_version, actor_id, payload FROM forum_domain_events WHERE tenant_id = ? AND event_id = ?",
            vec![tenant_id.into(), merged.event_id.into()],
        ))
        .await?
        .ok_or("semantic event row missing")?;
    assert_eq!(row.try_get::<String>("", "aggregate_type")?, "forum_topic");
    assert_eq!(
        row.try_get::<Uuid>("", "aggregate_id")?,
        merged.target_topic_id
    );
    assert_eq!(
        row.try_get::<String>("", "event_type")?,
        "forum.topic.merged"
    );
    assert_eq!(row.try_get::<i16>("", "schema_version")?, 1);
    assert_eq!(
        row.try_get::<Option<Uuid>>("", "actor_id")?,
        Some(merged.actor_id)
    );
    let payload: JsonValue = row.try_get("", "payload")?;
    let payload_object = payload
        .as_object()
        .ok_or("merge payload must be an object")?;
    assert_eq!(payload_object.len(), 9);
    assert!(!payload_object.contains_key("source_category_id"));
    assert_eq!(payload["operation_id"], merged.operation_id.to_string());
    assert_eq!(
        payload["source_topic_id"],
        merged.source_topic_id.to_string()
    );
    assert_eq!(
        payload["target_topic_id"],
        merged.target_topic_id.to_string()
    );
    assert_eq!(payload["category_id"], merged.category_id.to_string());
    assert_eq!(payload["moved_reply_count"], merged.moved_reply_count);
    assert_eq!(
        payload["moved_published_reply_count"],
        merged.moved_published_reply_count
    );
    assert_eq!(
        payload["resulting_published_reply_count"],
        merged.resulting_published_reply_count
    );
    assert_eq!(payload["position_offset"], merged.position_offset);
    assert_eq!(payload["reason"], merged.reason);
    Ok(())
}

async fn merge_operation_count(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS value FROM forum_topic_merge_operations WHERE tenant_id = ?",
            vec![tenant_id.into()],
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
    let row: QueryResult = db
        .query_one_raw(statement)
        .await?
        .ok_or("scalar row missing")?;
    Ok(row.try_get("", "value")?)
}
