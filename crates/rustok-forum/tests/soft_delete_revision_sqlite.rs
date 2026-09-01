use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{ForumModule, RevisionService};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::entities::{taxonomy_term, taxonomy_term_alias, taxonomy_term_translation};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Schema,
    Statement,
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
             WHERE lower(hex(tenant_id)) = replace('{}', '-', '')
               AND lower(hex(id)) = replace('{}', '-', '')",
            reply_seed.tenant_id, reply_seed.reply_id
        ),
    )
    .await?;

    assert_reply_tombstone(&db, &reply_seed).await?;
    assert_rejected(
        &db,
        format!(
            "DELETE FROM forum_replies
             WHERE lower(hex(tenant_id)) = replace('{}', '-', '')
               AND lower(hex(id)) = replace('{}', '-', '')",
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
    assert_eq!(
        reply_revisions[0].body.document,
        rustok_api::RichTextDocument::single_paragraph("Edited reply")
    );
    assert_eq!(reply_revisions[1].revision_reason, "edit");
    assert_eq!(
        reply_revisions[1].body.document,
        rustok_api::RichTextDocument::single_paragraph("Original reply")
    );

    let topic_seed = seed_thread(&db, "topic-soft-delete").await?;
    edit_topic_and_reply(&db, &topic_seed).await?;
    refresh_counters(&db, &topic_seed).await?;

    execute(
        &db,
        format!(
            "DELETE FROM forum_topics
             WHERE lower(hex(tenant_id)) = replace('{}', '-', '')
               AND lower(hex(id)) = replace('{}', '-', '')",
            topic_seed.tenant_id, topic_seed.topic_id
        ),
    )
    .await?;

    assert_topic_tombstone(&db, &topic_seed).await?;
    assert_rejected(
        &db,
        format!(
            "DELETE FROM forum_topics
             WHERE lower(hex(tenant_id)) = replace('{}', '-', '')
               AND lower(hex(id)) = replace('{}', '-', '')",
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

    let protected_seed = seed_thread(&db, "non-empty-category-delete").await?;
    assert_rejected(
        &db,
        format!(
            "DELETE FROM forum_categories
             WHERE lower(hex(tenant_id)) = replace('{}', '-', '')
               AND lower(hex(id)) = replace('{}', '-', '')",
            protected_seed.tenant_id, protected_seed.category_id
        ),
        "non-empty category delete",
    )
    .await?;

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
    let builder = db.get_database_backend();
    let schema = Schema::new(builder);
    for create in [
        schema.create_table_from_entity(taxonomy_term::Entity),
        schema.create_table_from_entity(taxonomy_term_translation::Entity),
        schema.create_table_from_entity(taxonomy_term_alias::Entity),
    ] {
        let mut create = create;
        create.if_not_exists();
        db.execute_raw(builder.build(&create)).await?;
    }
    db.execute_unprepared(
        "CREATE TABLE users (
            id TEXT NOT NULL,
            tenant_id TEXT NOT NULL,
            PRIMARY KEY (id),
            UNIQUE (tenant_id, id)
        )",
    )
    .await?;
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
    let original_topic_body = stored_document("Original topic body");
    let original_reply_body = stored_document("Original reply");
    let topic_translation_id = Uuid::new_v4();
    let reply_body_id = Uuid::new_v4();

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_categories
    (id, tenant_id, position, moderated, topic_count, reply_count)
VALUES
    (X'{}', X'{}', 0, 0, 0, 0);

INSERT INTO forum_topics
    (id, tenant_id, category_id, author_id, status, metadata,
     is_pinned, is_locked, reply_count)
VALUES
    (X'{}', X'{}', X'{}', X'{}', 'open', '{{"seed":"{}"}}',
     0, 0, 0);

INSERT INTO forum_topic_translations
    (id, tenant_id, topic_id, locale, title, slug, body)
VALUES
    (X'{}', X'{}', X'{}', 'en', 'Original topic', '{}',
     '{original_topic_body}');

INSERT INTO forum_replies
    (id, tenant_id, topic_id, author_id, status, position)
VALUES
    (X'{}', X'{}', X'{}', X'{}', 'approved', 1);

INSERT INTO forum_reply_bodies
    (id, tenant_id, reply_id, locale, body)
VALUES
    (X'{}', X'{}', X'{}', 'en', '{original_reply_body}');

INSERT INTO forum_solutions
    (tenant_id, topic_id, reply_id, marked_by_user_id)
VALUES
    (X'{}', X'{}', X'{}', X'{}');

INSERT INTO forum_user_stats
    (tenant_id, user_id, topic_count, reply_count, solution_count)
VALUES
    (X'{}', X'{}', 0, 0, 0);
"#,
            seed.category_id.simple(),
            seed.tenant_id.simple(),
            seed.topic_id.simple(),
            seed.tenant_id.simple(),
            seed.category_id.simple(),
            seed.author_id.simple(),
            slug,
            topic_translation_id.simple(),
            seed.tenant_id.simple(),
            seed.topic_id.simple(),
            slug,
            seed.reply_id.simple(),
            seed.tenant_id.simple(),
            seed.topic_id.simple(),
            seed.author_id.simple(),
            reply_body_id.simple(),
            seed.tenant_id.simple(),
            seed.reply_id.simple(),
            seed.tenant_id.simple(),
            seed.topic_id.simple(),
            seed.reply_id.simple(),
            seed.author_id.simple(),
            seed.tenant_id.simple(),
            seed.author_id.simple(),
        ),
    )
    .await?;

    Ok(seed)
}

async fn edit_topic_and_reply(db: &DatabaseConnection, seed: &ThreadSeed) -> TestResult<()> {
    let edited_topic_body = stored_document("Edited topic body");
    let edited_reply_body = stored_document("Edited reply");
    execute(
        db,
        format!(
            r#"
UPDATE forum_topic_translations
SET title = 'Edited topic',
    body = '{edited_topic_body}',
    updated_at = CURRENT_TIMESTAMP
WHERE lower(hex(tenant_id)) = replace('{}', '-', '')
  AND lower(hex(topic_id)) = replace('{}', '-', '') AND locale = 'en';

UPDATE forum_reply_bodies
SET body = '{edited_reply_body}',
    updated_at = CURRENT_TIMESTAMP
WHERE lower(hex(tenant_id)) = replace('{}', '-', '')
  AND lower(hex(reply_id)) = replace('{}', '-', '') AND locale = 'en';
"#,
            seed.tenant_id, seed.topic_id, seed.tenant_id, seed.reply_id
        ),
    )
    .await
}

fn stored_document(text: &str) -> String {
    serde_json::to_string(&rustok_api::RichTextDocument::single_paragraph(text))
        .expect("richtext fixture should serialize")
        .replace('\'', "''")
}

async fn refresh_counters(db: &DatabaseConnection, seed: &ThreadSeed) -> TestResult<()> {
    execute(
        db,
        format!(
            r#"
UPDATE forum_topics
SET reply_count = reply_count
WHERE lower(hex(tenant_id)) = replace('{}', '-', '')
  AND lower(hex(id)) = replace('{}', '-', '');

UPDATE forum_categories
SET topic_count = topic_count,
    reply_count = reply_count
WHERE lower(hex(tenant_id)) = replace('{}', '-', '')
  AND lower(hex(id)) = replace('{}', '-', '');

UPDATE forum_user_stats
SET topic_count = topic_count,
    reply_count = reply_count,
    solution_count = solution_count
WHERE lower(hex(tenant_id)) = replace('{}', '-', '')
  AND lower(hex(user_id)) = replace('{}', '-', '');
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
        .query_one_raw(Statement::from_string(
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
WHERE lower(hex(reply.tenant_id)) = replace('{}', '-', '')
  AND lower(hex(reply.id)) = replace('{}', '-', '')
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
        || body != stored_document("Edited reply")
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
        .query_one_raw(Statement::from_string(
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
WHERE lower(hex(topic.tenant_id)) = replace('{}', '-', '')
  AND lower(hex(topic.id)) = replace('{}', '-', '')
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
        || title != "Edited topic"
        || body != stored_document("Edited topic body")
        || reply_status != "deleted"
        || reply_is_deleted != 1
        || reply_body != stored_document("Edited reply")
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

async fn execute(db: &DatabaseConnection, sql: String) -> TestResult<()> {
    for statement in sql.split(';').map(str::trim).filter(|sql| !sql.is_empty()) {
        db.execute_unprepared(statement).await.map_err(|error| {
            test_error(format!(
                "SQLite statement failed: {error}; SQL: {statement}"
            ))
        })?;
    }
    Ok(())
}

async fn assert_rejected(db: &DatabaseConnection, sql: String, label: &str) -> TestResult<()> {
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
