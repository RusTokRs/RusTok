use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
    ForkForumReplyBranchInput, ForumError, ForumModule, ForumTopicForkService, ReplyService,
    TopicService,
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
        "sqlite:file:forum_topic_fork_{}?mode=memory&cache=shared",
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

async fn create_fixture(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    actor_id: Uuid,
) -> TestResult<(SecurityContext, Uuid, Uuid, Uuid, Uuid, Uuid)> {
    execute(
        db,
        "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
        vec![actor_id.into(), tenant_id.into()],
    )
    .await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Topic fork".to_string(),
                slug: "topic-fork".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?
        .id;
    let source_topic_id = TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id,
                title: "Source branch".to_string(),
                slug: Some("source-branch".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Source branch"),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id;
    let replies = ReplyService::new(db.clone(), event_bus.clone());
    let external_parent_id = replies
        .create(
            tenant_id,
            admin.clone(),
            source_topic_id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("External parent"),
                parent_reply_id: None,
            },
        )
        .await?
        .id;
    let branch_root_id = replies
        .create(
            tenant_id,
            admin.clone(),
            source_topic_id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Fork root"),
                parent_reply_id: Some(external_parent_id),
            },
        )
        .await?
        .id;
    let branch_child_id = replies
        .create(
            tenant_id,
            admin.clone(),
            source_topic_id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Fork child"),
                parent_reply_id: Some(branch_root_id),
            },
        )
        .await?
        .id;
    Ok((
        admin,
        category_id,
        source_topic_id,
        external_parent_id,
        branch_root_id,
        branch_child_id,
    ))
}

async fn insert_relation_revision(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    reply_id: Uuid,
    fingerprint: &str,
) -> TestResult<i64> {
    execute(
        db,
        r#"
        INSERT INTO forum_relation_revisions (
            tenant_id, target_kind, target_id, locale,
            projection_fingerprint, created_at
        ) VALUES (?, 'reply', ?, 'en', ?, CURRENT_TIMESTAMP)
        "#,
        vec![tenant_id.into(), reply_id.into(), fingerprint.into()],
    )
    .await?;
    Ok(row(
        db,
        "SELECT MAX(revision_id) AS revision_id FROM forum_relation_revisions WHERE tenant_id = ? AND target_kind = 'reply' AND target_id = ?",
        vec![tenant_id.into(), reply_id.into()],
    )
    .await?
    .try_get("", "revision_id")?)
}

#[tokio::test]
async fn reply_branch_fork_is_atomic_idempotent_and_preserves_provenance() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (admin, category_id, source_topic_id, external_parent_id, branch_root_id, branch_child_id) =
        create_fixture(&db, &event_bus, tenant_id, actor_id).await?;

    execute(
        &db,
        "INSERT INTO forum_solutions (topic_id, tenant_id, reply_id, marked_by_user_id, marked_at) VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)",
        vec![
            source_topic_id.into(),
            tenant_id.into(),
            branch_child_id.into(),
            actor_id.into(),
        ],
    )
    .await?;
    execute(
        &db,
        "UPDATE forum_user_stats SET solution_count = solution_count WHERE tenant_id = ? AND user_id = ?",
        vec![tenant_id.into(), actor_id.into()],
    )
    .await?;
    execute(
        &db,
        "INSERT INTO forum_topic_channel_access (tenant_id, topic_id, channel_slug) VALUES (?, ?, 'support')",
        vec![tenant_id.into(), source_topic_id.into()],
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
        vec![tenant_id.into(), branch_root_id.into()],
    )
    .await?;
    let source_reply_revision_id: i64 = row(
        &db,
        "SELECT MAX(id) AS id FROM forum_reply_revisions WHERE tenant_id = ? AND reply_id = ?",
        vec![tenant_id.into(), branch_root_id.into()],
    )
    .await?
    .try_get("", "id")?;

    let quoted_revision_id =
        insert_relation_revision(&db, tenant_id, external_parent_id, "quoted-external").await?;
    let source_relation_revision_id =
        insert_relation_revision(&db, tenant_id, branch_root_id, "fork-source").await?;
    execute(
        &db,
        "INSERT INTO forum_user_mentions (tenant_id, source_kind, source_id, source_locale, source_revision_id, mentioned_user_id, handle_snapshot, created_at) VALUES (?, 'reply', ?, 'en', ?, ?, 'actor', CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            branch_root_id.into(),
            source_relation_revision_id.into(),
            actor_id.into(),
        ],
    )
    .await?;
    execute(
        &db,
        "INSERT INTO forum_audience_mentions (tenant_id, source_kind, source_id, source_locale, source_revision_id, audience, created_at) VALUES (?, 'reply', ?, 'en', ?, 'moderators', CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            branch_root_id.into(),
            source_relation_revision_id.into(),
        ],
    )
    .await?;
    execute(
        &db,
        "INSERT INTO forum_quotes (tenant_id, source_kind, source_id, source_locale, source_revision_id, quoted_kind, quoted_id, quoted_revision_id, created_at) VALUES (?, 'reply', ?, 'en', ?, 'reply', ?, ?, CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            branch_root_id.into(),
            source_relation_revision_id.into(),
            external_parent_id.into(),
            quoted_revision_id.into(),
        ],
    )
    .await?;

    let category_before = row(
        &db,
        "SELECT topic_count, reply_count FROM forum_categories WHERE tenant_id = ? AND id = ?",
        vec![tenant_id.into(), category_id.into()],
    )
    .await?;
    let user_before = row(
        &db,
        "SELECT topic_count, reply_count, solution_count FROM forum_user_stats WHERE tenant_id = ? AND user_id = ?",
        vec![tenant_id.into(), actor_id.into()],
    )
    .await?;
    let source_before = row(
        &db,
        "SELECT status, reply_count, last_reply_at, updated_at FROM forum_topics WHERE tenant_id = ? AND id = ?",
        vec![tenant_id.into(), source_topic_id.into()],
    )
    .await?;
    let source_status_before: String = source_before.try_get("", "status")?;
    let source_reply_count_before: i64 = source_before.try_get("", "reply_count")?;
    let source_last_reply_before: Option<String> = source_before.try_get("", "last_reply_at")?;
    let source_updated_before: String = source_before.try_get("", "updated_at")?;

    let operation_id = Uuid::new_v4();
    let target_topic_id = Uuid::new_v4();
    let input = ForkForumReplyBranchInput {
        operation_id,
        target_topic_id,
        root_reply_id: branch_root_id,
        locale: "en".to_string(),
        title: "Forked branch".to_string(),
        slug: Some("Forked branch".to_string()),
        reason: "Preserve the original while continuing separately".to_string(),
    };
    let service = ForumTopicForkService::new(db.clone(), event_bus);
    let first = service
        .fork_reply_branch(tenant_id, source_topic_id, admin.clone(), input.clone())
        .await?;
    let replay = service
        .fork_reply_branch(tenant_id, source_topic_id, admin.clone(), input.clone())
        .await?;

    assert_eq!(first, replay);
    assert_eq!(first.copied_reply_count, 2);
    assert_eq!(first.copied_published_reply_count, 2);
    assert_eq!(first.copied_body_count, 2);
    assert_eq!(first.copied_reply_revision_count, 1);
    assert_eq!(first.copied_relation_revision_count, 3);
    assert_eq!(first.copied_mention_count, 2);
    assert_eq!(first.copied_quote_count, 1);

    let root_map = row(
        &db,
        "SELECT target_reply_id, source_parent_reply_id, target_parent_reply_id, target_position FROM forum_topic_fork_reply_items WHERE tenant_id = ? AND operation_id = ? AND source_reply_id = ?",
        vec![tenant_id.into(), operation_id.into(), branch_root_id.into()],
    )
    .await?;
    let target_root_id: Uuid = root_map.try_get("", "target_reply_id")?;
    assert_ne!(target_root_id, branch_root_id);
    assert_eq!(
        root_map.try_get::<Option<Uuid>>("", "source_parent_reply_id")?,
        Some(external_parent_id)
    );
    assert_eq!(
        root_map.try_get::<Option<Uuid>>("", "target_parent_reply_id")?,
        None
    );
    assert_eq!(root_map.try_get::<i64>("", "target_position")?, 1);

    let child_map = row(
        &db,
        "SELECT target_reply_id, target_parent_reply_id, target_position FROM forum_topic_fork_reply_items WHERE tenant_id = ? AND operation_id = ? AND source_reply_id = ?",
        vec![tenant_id.into(), operation_id.into(), branch_child_id.into()],
    )
    .await?;
    let target_child_id: Uuid = child_map.try_get("", "target_reply_id")?;
    assert_ne!(target_child_id, branch_child_id);
    assert_eq!(
        child_map.try_get::<Option<Uuid>>("", "target_parent_reply_id")?,
        Some(target_root_id)
    );
    assert_eq!(child_map.try_get::<i64>("", "target_position")?, 2);

    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_replies WHERE tenant_id = ? AND topic_id = ?",
            vec![tenant_id.into(), source_topic_id.into()],
        )
        .await?,
        3
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_replies WHERE tenant_id = ? AND topic_id = ?",
            vec![tenant_id.into(), target_topic_id.into()],
        )
        .await?,
        2
    );
    assert_eq!(
        row(
            &db,
            "SELECT parent_reply_id FROM forum_replies WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), branch_root_id.into()],
        )
        .await?
        .try_get::<Option<Uuid>>("", "parent_reply_id")?,
        Some(external_parent_id)
    );

    let source_after = row(
        &db,
        "SELECT status, reply_count, last_reply_at, updated_at FROM forum_topics WHERE tenant_id = ? AND id = ?",
        vec![tenant_id.into(), source_topic_id.into()],
    )
    .await?;
    assert_eq!(
        source_after.try_get::<String>("", "status")?,
        source_status_before
    );
    assert_eq!(
        source_after.try_get::<i64>("", "reply_count")?,
        source_reply_count_before
    );
    assert_eq!(
        source_after.try_get::<Option<String>>("", "last_reply_at")?,
        source_last_reply_before
    );
    assert_eq!(
        source_after.try_get::<String>("", "updated_at")?,
        source_updated_before
    );

    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_solutions WHERE tenant_id = ? AND topic_id = ? AND reply_id = ?",
            vec![
                tenant_id.into(),
                source_topic_id.into(),
                branch_child_id.into(),
            ],
        )
        .await?,
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_solutions WHERE tenant_id = ? AND topic_id = ?",
            vec![tenant_id.into(), target_topic_id.into()],
        )
        .await?,
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_topic_channel_access WHERE tenant_id = ? AND topic_id = ? AND channel_slug = 'support'",
            vec![tenant_id.into(), target_topic_id.into()],
        )
        .await?,
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_topic_fork_reply_items item JOIN forum_reply_bodies source_body ON source_body.tenant_id = item.tenant_id AND source_body.reply_id = item.source_reply_id JOIN forum_reply_bodies target_body ON target_body.tenant_id = item.tenant_id AND target_body.reply_id = item.target_reply_id AND target_body.locale = source_body.locale AND target_body.body = source_body.body WHERE item.tenant_id = ? AND item.operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        )
        .await?,
        2
    );

    let reply_revision_map = row(
        &db,
        "SELECT source_revision_id, target_revision_id FROM forum_topic_fork_revision_items WHERE tenant_id = ? AND operation_id = ? AND revision_kind = 'reply'",
        vec![tenant_id.into(), operation_id.into()],
    )
    .await?;
    assert_eq!(
        reply_revision_map.try_get::<i64>("", "source_revision_id")?,
        source_reply_revision_id
    );
    assert_ne!(
        reply_revision_map.try_get::<i64>("", "target_revision_id")?,
        source_reply_revision_id
    );

    let relation_map = row(
        &db,
        "SELECT source_revision_id, target_revision_id FROM forum_topic_fork_revision_items WHERE tenant_id = ? AND operation_id = ? AND revision_kind = 'relation' AND source_revision_id = ?",
        vec![
            tenant_id.into(),
            operation_id.into(),
            source_relation_revision_id.into(),
        ],
    )
    .await?;
    assert_eq!(
        relation_map.try_get::<i64>("", "source_revision_id")?,
        source_relation_revision_id
    );
    let target_relation_revision_id: i64 = relation_map.try_get("", "target_revision_id")?;
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_user_mentions WHERE tenant_id = ? AND source_revision_id = ?",
            vec![tenant_id.into(), target_relation_revision_id.into()],
        )
        .await?,
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_audience_mentions WHERE tenant_id = ? AND source_revision_id = ?",
            vec![tenant_id.into(), target_relation_revision_id.into()],
        )
        .await?,
        1
    );
    let quote = row(
        &db,
        "SELECT quoted_id, quoted_revision_id FROM forum_quotes WHERE tenant_id = ? AND source_revision_id = ?",
        vec![tenant_id.into(), target_relation_revision_id.into()],
    )
    .await?;
    assert_eq!(quote.try_get::<Uuid>("", "quoted_id")?, external_parent_id);
    assert_eq!(
        quote.try_get::<i64>("", "quoted_revision_id")?,
        quoted_revision_id
    );

    assert_eq!(
        row(
            &db,
            "SELECT topic_count, reply_count FROM forum_categories WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), category_id.into()],
        )
        .await?
        .try_get::<i64>("", "topic_count")?,
        category_before.try_get::<i64>("", "topic_count")? + 1
    );
    assert_eq!(
        row(
            &db,
            "SELECT topic_count, reply_count FROM forum_categories WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), category_id.into()],
        )
        .await?
        .try_get::<i64>("", "reply_count")?,
        category_before.try_get::<i64>("", "reply_count")? + 2
    );
    let user_after = row(
        &db,
        "SELECT topic_count, reply_count, solution_count FROM forum_user_stats WHERE tenant_id = ? AND user_id = ?",
        vec![tenant_id.into(), actor_id.into()],
    )
    .await?;
    assert_eq!(
        user_after.try_get::<i64>("", "topic_count")?,
        user_before.try_get::<i64>("", "topic_count")? + 1
    );
    assert_eq!(
        user_after.try_get::<i64>("", "reply_count")?,
        user_before.try_get::<i64>("", "reply_count")? + 2
    );
    assert_eq!(
        user_after.try_get::<i64>("", "solution_count")?,
        user_before.try_get::<i64>("", "solution_count")?
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_domain_events WHERE tenant_id = ? AND event_id = ? AND event_type = 'forum.topic.forked'",
            vec![tenant_id.into(), operation_id.into()],
        )
        .await?,
        1
    );

    let mut conflict = input;
    conflict.reason = "Changed fork command".to_string();
    assert!(matches!(
        service
            .fork_reply_branch(tenant_id, source_topic_id, admin, conflict)
            .await,
        Err(ForumError::TopicForkOperationConflict(id)) if id == operation_id
    ));
    assert!(
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE forum_topic_fork_operations SET reason = ? WHERE tenant_id = ? AND operation_id = ?",
            vec!["tamper".into(), tenant_id.into(), operation_id.into()],
        ))
        .await
        .is_err()
    );
    assert!(
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "DELETE FROM forum_topic_fork_reply_items WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        ))
        .await
        .is_err()
    );
    assert!(
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE forum_topic_fork_revision_items SET locale = ? WHERE tenant_id = ? AND operation_id = ?",
            vec!["ru".into(), tenant_id.into(), operation_id.into()],
        ))
        .await
        .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn reply_branch_fork_rejects_non_topological_source_positions_atomically() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (admin, _category_id, source_topic_id, _external, root_reply_id, _child) =
        create_fixture(&db, &event_bus, tenant_id, actor_id).await?;
    execute(
        &db,
        "UPDATE forum_replies SET position = 4 WHERE tenant_id = ? AND id = ?",
        vec![tenant_id.into(), root_reply_id.into()],
    )
    .await?;

    let operation_id = Uuid::new_v4();
    let target_topic_id = Uuid::new_v4();
    let error = ForumTopicForkService::new(db.clone(), event_bus)
        .fork_reply_branch(
            tenant_id,
            source_topic_id,
            admin,
            ForkForumReplyBranchInput {
                operation_id,
                target_topic_id,
                root_reply_id,
                locale: "en".to_string(),
                title: "Invalid fork".to_string(),
                slug: None,
                reason: "Must reject non-topological positions".to_string(),
            },
        )
        .await
        .expect_err("parent-after-child ordering must fail");
    assert!(error.to_string().contains("parent-before-child"));
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_topics WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), target_topic_id.into()],
        )
        .await?,
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM forum_topic_fork_operations WHERE tenant_id = ? AND operation_id = ?",
            vec![tenant_id.into(), operation_id.into()],
        )
        .await?,
        0
    );
    Ok(())
}
