use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumError,
    ForumModule, ForumTopicMergeResult, ForumTopicMergeService, MergeForumTopicInput,
    ModerationService, ReplyService, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, QueryResult,
    Statement,
};
use sea_orm_migration::SchemaManager;
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct SolutionSnapshot {
    reply_id: Uuid,
    marked_by_user_id: Option<Uuid>,
    marked_at: String,
}

async fn setup() -> TestResult<(DatabaseConnection, TransactionalEventBus)> {
    let db_url = format!(
        "sqlite:file:forum_topic_merge_solution_resolution_{}?mode=memory&cache=shared",
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
                title: format!("Solution resolution {key}"),
                slug: Some(format!("solution-resolution-{key}")),
                body: rustok_api::RichTextDocument::single_paragraph(format!(
                    "Solution resolution {key} body"
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
) -> TestResult<Uuid> {
    Ok(ReplyService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            topic_id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph(text),
                parent_reply_id: None,
            },
        )
        .await?
        .id)
}

struct Fixture {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    tenant_id: Uuid,
    actor_id: Uuid,
    source_author_id: Uuid,
    target_author_id: Uuid,
    admin: SecurityContext,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    source_reply_id: Uuid,
    target_reply_id: Uuid,
    source_solution: SolutionSnapshot,
    target_solution: SolutionSnapshot,
}

async fn fixture(key: &str) -> TestResult<Fixture> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let source_author_id = Uuid::new_v4();
    let target_author_id = Uuid::new_v4();
    for user_id in [actor_id, source_author_id, target_author_id] {
        insert_user(&db, tenant_id, user_id).await?;
    }
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let source_author = SecurityContext::new(UserRole::Customer, Some(source_author_id));
    let target_author = SecurityContext::new(UserRole::Customer, Some(target_author_id));
    let category_id = create_category(
        &db,
        tenant_id,
        admin.clone(),
        &format!("solution-resolution-{key}"),
    )
    .await?;
    let source_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        &format!("{key}-source"),
    )
    .await?;
    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        &format!("{key}-target"),
    )
    .await?;
    let source_reply_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        source_topic_id,
        source_author,
        "Source competing solution",
    )
    .await?;
    let target_reply_id = create_reply(
        &db,
        &event_bus,
        tenant_id,
        target_topic_id,
        target_author,
        "Target competing solution",
    )
    .await?;
    let moderation = ModerationService::new(db.clone(), event_bus.clone());
    moderation
        .mark_solution(tenant_id, source_topic_id, source_reply_id, admin.clone())
        .await?;
    moderation
        .mark_solution(tenant_id, target_topic_id, target_reply_id, admin.clone())
        .await?;
    let source_solution = solution_snapshot(&db, tenant_id, source_topic_id)
        .await?
        .ok_or("source solution missing")?;
    let target_solution = solution_snapshot(&db, tenant_id, target_topic_id)
        .await?
        .ok_or("target solution missing")?;
    Ok(Fixture {
        db,
        event_bus,
        tenant_id,
        actor_id,
        source_author_id,
        target_author_id,
        admin,
        source_topic_id,
        target_topic_id,
        source_reply_id,
        target_reply_id,
        source_solution,
        target_solution,
    })
}

#[tokio::test]
async fn manager_can_select_source_solution_and_replay_exact_audit() -> TestResult<()> {
    let fixture = fixture("source-wins").await?;
    let operation_id = Uuid::new_v4();
    let input = MergeForumTopicInput {
        operation_id,
        source_topic_id: fixture.source_topic_id,
        reason: "Select the source answer after moderator review".to_string(),
    };
    let service = ForumTopicMergeService::new(fixture.db.clone(), fixture.event_bus.clone());

    let unresolved = service
        .merge_topic(
            fixture.tenant_id,
            fixture.target_topic_id,
            fixture.admin.clone(),
            input.clone(),
        )
        .await
        .expect_err("ordinary merge must keep competing solutions fail-closed");
    assert!(matches!(
        unresolved,
        ForumError::TopicMergeSolutionConflict(id) if id == operation_id
    ));

    let merged = service
        .merge_topic_resolving_solution(
            fixture.tenant_id,
            fixture.target_topic_id,
            fixture.admin.clone(),
            fixture.source_reply_id,
            input.clone(),
        )
        .await?;
    assert_eq!(merged.operation_id, operation_id);
    assert_eq!(merged.event_id, operation_id);
    assert_eq!(
        solution_snapshot(&fixture.db, fixture.tenant_id, fixture.source_topic_id).await?,
        None
    );
    assert_eq!(
        solution_snapshot(&fixture.db, fixture.tenant_id, fixture.target_topic_id).await?,
        Some(fixture.source_solution.clone())
    );
    assert_eq!(
        user_solution_count(&fixture.db, fixture.tenant_id, fixture.source_author_id).await?,
        1
    );
    assert_eq!(
        user_solution_count(&fixture.db, fixture.tenant_id, fixture.target_author_id).await?,
        0
    );
    assert_eq!(
        reply_topic_id(&fixture.db, fixture.tenant_id, fixture.source_reply_id).await?,
        fixture.target_topic_id
    );
    assert_merge_event_and_resolution_audit(
        &fixture.db,
        fixture.tenant_id,
        fixture.actor_id,
        &merged,
        fixture.source_reply_id,
        fixture.target_reply_id,
        fixture.source_reply_id,
        fixture.target_reply_id,
        Some(fixture.target_author_id),
    )
    .await?;

    let replay = service
        .merge_topic_resolving_solution(
            fixture.tenant_id,
            fixture.target_topic_id,
            fixture.admin.clone(),
            fixture.source_reply_id,
            input.clone(),
        )
        .await?;
    assert_eq!(replay, merged);
    assert_eq!(
        merge_operation_count(&fixture.db, fixture.tenant_id).await?,
        1
    );
    assert_eq!(merge_event_count(&fixture.db, fixture.tenant_id).await?, 1);
    assert_eq!(
        resolution_audit_count(&fixture.db, fixture.tenant_id).await?,
        1
    );
    assert_eq!(
        user_solution_count(&fixture.db, fixture.tenant_id, fixture.target_author_id).await?,
        0
    );

    let selection_drift = service
        .merge_topic_resolving_solution(
            fixture.tenant_id,
            fixture.target_topic_id,
            fixture.admin.clone(),
            fixture.target_reply_id,
            input.clone(),
        )
        .await;
    assert!(matches!(
        selection_drift,
        Err(ForumError::TopicMergeOperationConflict(id)) if id == operation_id
    ));
    let command_shape_drift = service
        .merge_topic(
            fixture.tenant_id,
            fixture.target_topic_id,
            fixture.admin,
            input,
        )
        .await;
    assert!(matches!(
        command_shape_drift,
        Err(ForumError::TopicMergeOperationConflict(id)) if id == operation_id
    ));

    let update_result = fixture
        .db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE forum_topic_merge_solution_resolutions SET selected_solution_reply_id = ? WHERE tenant_id = ? AND operation_id = ?",
            vec![
                fixture.target_reply_id.into(),
                fixture.tenant_id.into(),
                operation_id.into(),
            ],
        ))
        .await;
    assert!(update_result.is_err());
    let delete_result = fixture
        .db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "DELETE FROM forum_topic_merge_solution_resolutions WHERE tenant_id = ? AND operation_id = ?",
            vec![fixture.tenant_id.into(), operation_id.into()],
        ))
        .await;
    assert!(delete_result.is_err());
    Ok(())
}

#[tokio::test]
async fn manager_can_select_target_solution_and_invalid_selection_is_atomic() -> TestResult<()> {
    let fixture = fixture("target-wins").await?;
    let service = ForumTopicMergeService::new(fixture.db.clone(), fixture.event_bus.clone());
    let invalid_operation_id = Uuid::new_v4();
    let invalid = service
        .merge_topic_resolving_solution(
            fixture.tenant_id,
            fixture.target_topic_id,
            fixture.admin.clone(),
            Uuid::new_v4(),
            MergeForumTopicInput {
                operation_id: invalid_operation_id,
                source_topic_id: fixture.source_topic_id,
                reason: "Reject an unrelated solution identity".to_string(),
            },
        )
        .await
        .expect_err("unrelated solution identity must fail before mutation");
    assert_eq!(invalid.stable_code(), "FORUM_VALIDATION_FAILED");
    assert_eq!(
        merge_operation_count(&fixture.db, fixture.tenant_id).await?,
        0
    );
    assert_eq!(merge_event_count(&fixture.db, fixture.tenant_id).await?, 0);
    assert_eq!(
        resolution_audit_count(&fixture.db, fixture.tenant_id).await?,
        0
    );
    assert_eq!(
        solution_snapshot(&fixture.db, fixture.tenant_id, fixture.source_topic_id).await?,
        Some(fixture.source_solution.clone())
    );
    assert_eq!(
        solution_snapshot(&fixture.db, fixture.tenant_id, fixture.target_topic_id).await?,
        Some(fixture.target_solution.clone())
    );

    let operation_id = Uuid::new_v4();
    let merged = service
        .merge_topic_resolving_solution(
            fixture.tenant_id,
            fixture.target_topic_id,
            fixture.admin,
            fixture.target_reply_id,
            MergeForumTopicInput {
                operation_id,
                source_topic_id: fixture.source_topic_id,
                reason: "Retain the target answer after moderator review".to_string(),
            },
        )
        .await?;
    assert_eq!(merged.operation_id, operation_id);
    assert_eq!(
        solution_snapshot(&fixture.db, fixture.tenant_id, fixture.target_topic_id).await?,
        Some(fixture.target_solution)
    );
    assert_eq!(
        user_solution_count(&fixture.db, fixture.tenant_id, fixture.source_author_id).await?,
        0
    );
    assert_eq!(
        user_solution_count(&fixture.db, fixture.tenant_id, fixture.target_author_id).await?,
        1
    );
    assert_merge_event_and_resolution_audit(
        &fixture.db,
        fixture.tenant_id,
        fixture.actor_id,
        &merged,
        fixture.source_reply_id,
        fixture.target_reply_id,
        fixture.target_reply_id,
        fixture.source_reply_id,
        Some(fixture.source_author_id),
    )
    .await?;
    Ok(())
}

async fn solution_snapshot(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<Option<SolutionSnapshot>> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT reply_id, marked_by_user_id, marked_at FROM forum_solutions WHERE tenant_id = ? AND topic_id = ?",
            vec![tenant_id.into(), topic_id.into()],
        ))
        .await?;
    row.map(|row| {
        Ok(SolutionSnapshot {
            reply_id: row.try_get("", "reply_id")?,
            marked_by_user_id: row.try_get("", "marked_by_user_id")?,
            marked_at: row.try_get("", "marked_at")?,
        })
    })
    .transpose()
}

async fn user_solution_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
) -> TestResult<i32> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT solution_count FROM forum_user_stats WHERE tenant_id = ? AND user_id = ?",
            vec![tenant_id.into(), user_id.into()],
        ))
        .await?
        .ok_or("forum user stat row missing")?;
    Ok(row.try_get("", "solution_count")?)
}

async fn reply_topic_id(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> TestResult<Uuid> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT topic_id FROM forum_replies WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), reply_id.into()],
        ))
        .await?
        .ok_or("reply row missing")?;
    Ok(row.try_get("", "topic_id")?)
}

async fn assert_merge_event_and_resolution_audit(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    actor_id: Uuid,
    merge: &ForumTopicMergeResult,
    source_solution_reply_id: Uuid,
    target_solution_reply_id: Uuid,
    selected_solution_reply_id: Uuid,
    rejected_solution_reply_id: Uuid,
    rejected_solution_author_id: Option<Uuid>,
) -> TestResult<()> {
    let event = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT schema_version, actor_id, payload FROM forum_domain_events WHERE tenant_id = ? AND event_id = ?",
            vec![tenant_id.into(), merge.operation_id.into()],
        ))
        .await?
        .ok_or("merge semantic event missing")?;
    assert_eq!(event.try_get::<i16>("", "schema_version")?, 1);
    assert_eq!(
        event.try_get::<Option<Uuid>>("", "actor_id")?,
        Some(actor_id)
    );
    let payload: JsonValue = event.try_get("", "payload")?;
    assert_eq!(
        payload,
        json!({
            "operation_id": merge.operation_id,
            "source_topic_id": merge.source_topic_id,
            "target_topic_id": merge.target_topic_id,
            "category_id": merge.category_id,
            "moved_reply_count": merge.moved_reply_count,
            "moved_published_reply_count": merge.moved_published_reply_count,
            "resulting_published_reply_count": merge.resulting_published_reply_count,
            "position_offset": merge.position_offset,
            "reason": merge.reason,
        })
    );
    assert!(payload.get("solution_resolution").is_none());

    let audit = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT source_solution_reply_id, target_solution_reply_id, selected_solution_reply_id, rejected_solution_reply_id, rejected_solution_author_id, resolved_at FROM forum_topic_merge_solution_resolutions WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), merge.operation_id.into()],
        ))
        .await?
        .ok_or("merge solution resolution audit missing")?;
    assert_eq!(
        audit.try_get::<Uuid>("", "source_solution_reply_id")?,
        source_solution_reply_id
    );
    assert_eq!(
        audit.try_get::<Uuid>("", "target_solution_reply_id")?,
        target_solution_reply_id
    );
    assert_eq!(
        audit.try_get::<Uuid>("", "selected_solution_reply_id")?,
        selected_solution_reply_id
    );
    assert_eq!(
        audit.try_get::<Uuid>("", "rejected_solution_reply_id")?,
        rejected_solution_reply_id
    );
    assert_eq!(
        audit.try_get::<Option<Uuid>>("", "rejected_solution_author_id")?,
        rejected_solution_author_id
    );
    assert!(!audit.try_get::<String>("", "resolved_at")?.is_empty());
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

async fn resolution_audit_count(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS value FROM forum_topic_merge_solution_resolutions WHERE tenant_id = ?",
            vec![tenant_id.into()],
        ),
    )
    .await
}

async fn scalar_i64(db: &DatabaseConnection, statement: Statement) -> TestResult<i64> {
    let row: QueryResult = db.query_one(statement).await?.ok_or("scalar row missing")?;
    Ok(row.try_get("", "value")?)
}
