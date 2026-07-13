use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{ForumModule, RevisionService};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
    TryGetable,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn sqlite_preserves_forum_tombstones_and_revision_history() -> TestResult<()> {
    let db = setup_sqlite().await?;

    let reply_seed = seed_thread(&db, "reply-soft-delete").await?;
    edit_topic_and_reply(&db, &reply_seed).await?;
    refresh_counters(&db, &reply_seed).await?;

    execute(
        &db,
        format!(
            "DELETE FROM forum_replies
             WHERE tenant_id = '{}' AND id = '{}'",
            reply_seed.tenant_id, reply_seed.reply_id
        ),
    )
    .await?;

    assert_reply_tombstone(&db, &reply_seed).await?;
    assert_rejected(
        &db,
        format!(
            "DELETE FROM forum_replies
             WHERE tenant_id = '{}' AND id = '{}'",
            reply_seed.tenant_id, reply_seed.reply_id
        ),
        "repeated reply soft delete",
    )
    .await?;

    let revision_service = RevisionService::new(db.clone());
    let reply_revisions = revision_service
        .list_reply_revisions(
            reply_seed.tenant_id,
            reply_seed.reply_id,
            Some("en"),
            20,
            admin_security(),
        )
        .await?;
    assert_eq!(reply_revisions.len(), 2);
    assert_eq!(reply_revisions[0].revision_reason, "delete");
    assert_eq!(reply_revisions[0].body, "Edited reply");
    assert_eq!(reply_revisions[1].revision_reason, "edit");
    assert_eq!(reply_revisions[1].body, "Original reply");

    let topic_seed = seed_thread(&db, "topic-soft-delete").await?;
    edit_topic_and_reply(&db, &topic_seed).await?;
    refresh_counters(&db, &topic_seed).await?;

    execute(
        &db,
        format!(
            "DELETE FROM forum_topics
             WHERE tenant_id = '{}' AND id = '{}'",
            topic_seed.tenant_id, topic_seed.topic_id
        ),
    )
    .await?;

    assert_topic_tombstone(&db, &topic_seed).await?;
    assert_rejected(
        &db,
        format!(
            "DELETE FROM forum_topics
             WHERE tenant_id = '{}' AND id = '{}'",
            topic_seed.tenant_id, topic_seed.topic_id
        ),
        "repeated topic soft delete",
    )
    .await?;

    let topic_revisions = revision_service
        .list_topic_revisions(
            topic_seed.tenant_id,
            topic_seed.topic_id,
            Some("en"),
            20,
            admin_security(),
        )
        .await?;
    assert_eq!(topic_revisions.len(), 2);
    assert_eq!(topic_revisions[0].revision_reason, "delete");
    assert_eq!(topic_revisions[0].title, "Edited topic");
    assert_eq!(topic_revisions[1].revision_reason, "edit");
    assert_eq!(topic_revisions[1].title, "Original topic");

    let cascade_seed = seed_thread(&db, "category-hard-delete").await?;
    execute(
        &db,
        format!(
            "DELETE FROM forum_categories
             WHERE tenant_id = '{}' AND id = '{}'",
            cascade_seed.tenant_id, cascade_seed.category_id
        ),
    )
    .await?;
    assert_absent(&db, "forum_topics", cascade_seed.tenant_id, cascade_seed.topic_id).await?;
    assert_absent(&db, "forum_replies", cascade_seed.tenant_id, cascade_seed.reply_id).await?;

    Ok(())
}

#[derive(Clone, Copy)]
struct ThreadSeed {
    tenant_id: Uuid,
    category_id: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
    author_id: Uuid,
}

async fn setup_sqlite() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_soft_delete_revision_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    let manager = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}

async fn seed_thread(db: &DatabaseConnection, slug: &str) -> TestResult<ThreadSeed> {
    let seed = ThreadSeed {
        tenant_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        topic_id: Uuid::new_v4(),
        reply_id: Uuid::new_v4(),
        author_id: Uuid::new_v4(),
    };

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_categories
    (id, tenant_id, position, moderated, topic_count, reply_count)
VALUES
    ('{}', '{}', 0, 0, 0, 0);

INSERT INTO forum_topics
    (id, tenant_id, category_id, author_id, status, metadata,
     is_pinned, is_locked, reply_count)
VALUES
    ('{}', '{}', '{}', '{}', 'open', '{{"seed":"{}"}}',
     0, 0, 0);

INSERT INTO forum_topic_translations
    (id, tenant_id, topic_id, locale, title, slug, body, body_format)
VALUES
    ('{}', '{}', '{}', 'en', 'Original topic', '{}',
     'Original topic body', 'markdown');

INSERT INTO forum_replies
    (id, tenant_id, topic_id, author_id, status, position)
VALUES
    ('{}', '{}', '{}', '{}', 'approved', 1);

INSERT INTO forum_reply_bodies
    (id, tenant_id, reply_id, locale, body, body_format)
VALUES
    ('{}', '{}', '{}', 'en', 'Original reply', 'markdown');

INSERT INTO forum_solutions
    (tenant_id, topic_id, reply_id, marked_by_user_id)
VALUES
    ('{}', '{}', '{}', '{}');

INSERT INTO forum_user_stats
    (tenant_id, user_id, topic_count, reply_count, solution_count)
VALUES
    ('{}', '{}', 0, 0, 0);
"#,
            seed.category_id,
            seed.tenant_id,
            seed.topic_id,
            seed.tenant_id,
            seed.category_id,
            seed.author_id,
            slug,
            Uuid::new_v4(),
            seed.tenant_id,
            seed.topic_id,
            slug,
            seed.reply_id,
            seed.tenant_id,
            seed.topic_id,
            seed.author_id,
            Uuid::new_v4(),
            seed.tenant_id,
            seed.reply_id,
            seed.tenant_id,
            seed.topic_id,
            seed.reply_id,
            seed.author_id,
            seed.tenant_id,
            seed.author_id,
        ),
    )
    .await?;

    Ok(seed)
}

async fn edit_topic_and_reply(db: &DatabaseConnection, seed: &ThreadSeed) -> TestResult<()> {
    execute(
        db,
        format!(
            r#"
UPDATE forum_topic_translations
SET title = 'Edited topic',
    body = 'Edited topic body',
    updated_at = CURRENT_TIMESTAMP
WHERE tenant_id = '{}' AND topic_id = '{}' AND locale = 'en';

UPDATE forum_reply_bodies
SET body = 'Edited reply',
    updated_at = CURRENT_TIMESTAMP
WHERE tenant_id = '{}' AND reply_id = '{}' AND locale = 'en';
"#,
            seed.tenant_id, seed.topic_id, seed.tenant_id, seed.reply_id
        ),
    )
    .await
}

async fn refresh_counters(db: &DatabaseConnection, seed: &ThreadSeed) -> TestResult<()> {
    execute(
        db,
        format!(
            r#"
UPDATE forum_topics
SET reply_count = reply_count
WHERE tenant_id = '{}' AND id = '{}';

UPDATE forum_categories
SET topic_count = topic_count,
    reply_count = reply_count
WHERE tenant_id = '{}' AND id = '{}';

UPDATE forum_user_stats
SET topic_count = topic_count,
    reply_count = reply_count,
    solution_count = solution_count
WHERE tenant_id = '{}' AND user_id = '{}';
"#,
            seed.tenant_id,
            seed.topic_id,
            seed.tenant_id,
            seed.category_id,
            seed.tenant_id,
            seed.author_id,
        ),
    )
    .await
}

async fn assert_reply_tombstone(db: &DatabaseConnection, seed: &ThreadSeed) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                r#"
SELECT
    reply.status,
    CASE WHEN reply.deleted_at IS NOT NULL THEN 1 ELSE 0 END AS is_deleted,
    body.body,
    (SELECT COUNT(*) FROM forum_solutions solution
      WHERE solution.tenant_id = reply.tenant_id
        AND solution.reply_id = reply.id) AS solution_count,
    topic.reply_count AS topic_reply_count,
    category.reply_count AS category_reply_count,
    stats.reply_count AS user_reply_count,
    stats.solution_count AS user_solution_count
FROM forum_replies reply
JOIN forum_reply_bodies body
  ON body.tenant_id = reply.tenant_id
 AND body.reply_id = reply.id
JOIN forum_topics topic
  ON topic.tenant_id = reply.tenant_id
 AND topic.id = reply.topic_id
JOIN forum_categories category
  ON category.tenant_id = topic.tenant_id
 AND category.id = topic.category_id
JOIN forum_user_stats stats
  ON stats.tenant_id = reply.tenant_id
 AND stats.user_id = reply.author_id
WHERE reply.tenant_id = '{}' AND reply.id = '{}'
"#,
                seed.tenant_id, seed.reply_id
            ),
        ))
        .await?
        .ok_or_else(|| test_error("soft-deleted reply row disappeared"))?;

    let status: String = row.try_get("", "status")?;
    let is_deleted: i64 = row.try_get("", "is_deleted")?;
    let body: String = row.try_get("", "body")?;
    let counts = [
        row.try_get::<i64>("", "solution_count")?,
        row.try_get::<i64>("", "topic_reply_count")?,
        row.try_get::<i64>("", "category_reply_count")?,
        row.try_get::<i64>("", "user_reply_count")?,
        row.try_get::<i64>("", "user_solution_count")?,
    ];
    if status != "deleted"
        || is_deleted != 1
        || body != "[deleted]"
        || counts.iter().any(|count| *count != 0)
    {
        return Err(test_error(format!(
            "invalid reply tombstone: status={status}, deleted={is_deleted}, \
             body={body}, counts={counts:?}"
        )));
    }
    Ok(())
}

async fn assert_topic_tombstone(db: &DatabaseConnection, seed: &ThreadSeed) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                r#"
SELECT
    topic.status,
    CASE WHEN topic.deleted_at IS NOT NULL THEN 1 ELSE 0 END AS is_deleted,
    topic.is_locked,
    translation.title,
    translation.body,
    reply.status AS reply_status,
    CASE WHEN reply.deleted_at IS NOT NULL THEN 1 ELSE 0 END AS reply_is_deleted,
    reply_body.body AS reply_body,
    category.topic_count AS category_topic_count,
    category.reply_count AS category_reply_count,
    stats.topic_count AS user_topic_count,
    stats.reply_count AS user_reply_count,
    stats.solution_count AS user_solution_count
FROM forum_topics topic
JOIN forum_topic_translations translation
  ON translation.tenant_id = topic.tenant_id
 AND translation.topic_id = topic.id
JOIN forum_replies reply
  ON reply.tenant_id = topic.tenant_id
 AND reply.topic_id = topic.id
JOIN forum_reply_bodies reply_body
  ON reply_body.tenant_id = reply.tenant_id
 AND reply_body.reply_id = reply.id
JOIN forum_categories category
  ON category.tenant_id = topic.tenant_id
 AND category.id = topic.category_id
JOIN forum_user_stats stats
  ON stats.tenant_id = topic.tenant_id
 AND stats.user_id = topic.author_id
WHERE topic.tenant_id = '{}' AND topic.id = '{}'
"#,
                seed.tenant_id, seed.topic_id
            ),
        ))
        .await?
        .ok_or_else(|| test_error("soft-deleted topic row disappeared"))?;

    let status: String = row.try_get("", "status")?;
    let is_deleted: i64 = row.try_get("", "is_deleted")?;
    let is_locked: i64 = row.try_get("", "is_locked")?;
    let title: String = row.try_get("", "title")?;
    let body: String = row.try_get("", "body")?;
    let reply_status: String = row.try_get("", "reply_status")?;
    let reply_is_deleted: i64 = row.try_get("", "reply_is_deleted")?;
    let reply_body: String = row.try_get("", "reply_body")?;
    let counts = [
        row.try_get::<i64>("", "category_topic_count")?,
        row.try_get::<i64>("", "category_reply_count")?,
        row.try_get::<i64>("", "user_topic_count")?,
        row.try_get::<i64>("", "user_reply_count")?,
        row.try_get::<i64>("", "user_solution_count")?,
    ];

    if status != "archived"
        || is_deleted != 1
        || is_locked != 1
        || title != "[deleted]"
        || body != "[deleted]"
        || reply_status != "deleted"
        || reply_is_deleted != 1
        || reply_body != "[deleted]"
        || counts.iter().any(|count| *count != 0)
    {
        return Err(test_error(format!(
            "invalid topic tombstone: status={status}, deleted={is_deleted}, locked={is_locked}, \
             title={title}, body={body}, reply_status={reply_status}, \
             reply_deleted={reply_is_deleted}, reply_body={reply_body}, counts={counts:?}"
        )));
    }
    Ok(())
}

async fn assert_absent(
    db: &DatabaseConnection,
    table: &str,
    tenant_id: Uuid,
    id: Uuid,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT COUNT(*) AS value
                 FROM {table}
                 WHERE tenant_id = '{tenant_id}' AND id = '{id}'"
            ),
        ))
        .await?
        .ok_or_else(|| test_error(format!("{table} count query returned no row")))?;
    let count: i64 = row.try_get("", "value")?;
    if count != 0 {
        return Err(test_error(format!(
            "{table} row must be physically removed by category cascade"
        )));
    }
    Ok(())
}

async fn execute(db: &DatabaseConnection, sql: String) -> TestResult<()> {
    db.execute_unprepared(&sql).await?;
    Ok(())
}

async fn assert_rejected(
    db: &DatabaseConnection,
    sql: String,
    label: &str,
) -> TestResult<()> {
    if db.execute_unprepared(&sql).await.is_ok() {
        return Err(test_error(format!("{label} must be rejected")));
    }
    Ok(())
}

fn admin_security() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn test_error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other(message.into()))
}
