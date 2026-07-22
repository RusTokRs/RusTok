use rustok_core::MigrationSource;
use rustok_forum::ForumModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn sqlite_rejects_cross_tenant_forum_relation_rows() -> TestResult<()> {
    let db = setup_sqlite().await?;
    exercise_relation_constraints(&db).await
}

async fn setup_sqlite() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_relation_tenant_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    execute(
        &db,
        r#"
CREATE TABLE taxonomy_terms (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    scope_type TEXT NOT NULL,
    scope_value TEXT NOT NULL DEFAULT '',
    canonical_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
)
"#
        .to_string(),
    )
    .await?;

    let manager = SchemaManager::new(&db);
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}

async fn exercise_relation_constraints(db: &DatabaseConnection) -> TestResult<()> {
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let category_a = Uuid::new_v4();
    let category_b = Uuid::new_v4();
    let topic_a = Uuid::new_v4();
    let topic_a2 = Uuid::new_v4();
    let topic_b = Uuid::new_v4();
    let reply_a = Uuid::new_v4();
    let reply_a2 = Uuid::new_v4();
    let reply_b = Uuid::new_v4();
    let term_a = Uuid::new_v4();
    let term_b = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_categories
    (id, tenant_id, position, moderated, topic_count, reply_count)
VALUES
    ('{category_a}', '{tenant_a}', 0, 0, 0, 0),
    ('{category_b}', '{tenant_b}', 0, 0, 0, 0);

INSERT INTO forum_topics
    (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
VALUES
    ('{topic_a}', '{tenant_a}', '{category_a}', 'open', '{{}}', 0, 0, 0),
    ('{topic_a2}', '{tenant_a}', '{category_a}', 'open', '{{}}', 0, 0, 0),
    ('{topic_b}', '{tenant_b}', '{category_b}', 'open', '{{}}', 0, 0, 0);

INSERT INTO forum_replies
    (id, tenant_id, topic_id, status, position)
VALUES
    ('{reply_a}', '{tenant_a}', '{topic_a}', 'approved', 1),
    ('{reply_a2}', '{tenant_a}', '{topic_a2}', 'approved', 1),
    ('{reply_b}', '{tenant_b}', '{topic_b}', 'approved', 1);

INSERT INTO taxonomy_terms
    (id, tenant_id, kind, scope_type, scope_value, canonical_key, status)
VALUES
    ('{term_a}', '{tenant_a}', 'tag', 'module', 'forum', 'tenant-a-tag', 'active'),
    ('{term_b}', '{tenant_b}', 'tag', 'module', 'forum', 'tenant-b-tag', 'active');
"#
        ),
    )
    .await?;

    for (sql, label) in [
        (
            format!(
                "INSERT INTO forum_topic_votes (topic_id, user_id, tenant_id, value) VALUES ('{topic_a}', '{user_id}', '{tenant_b}', 1)"
            ),
            "cross-tenant topic vote",
        ),
        (
            format!(
                "INSERT INTO forum_reply_votes (reply_id, user_id, tenant_id, value) VALUES ('{reply_a}', '{user_id}', '{tenant_b}', 1)"
            ),
            "cross-tenant reply vote",
        ),
        (
            format!(
                "INSERT INTO forum_category_subscriptions (category_id, user_id, tenant_id) VALUES ('{category_a}', '{user_id}', '{tenant_b}')"
            ),
            "cross-tenant category subscription",
        ),
        (
            format!(
                "INSERT INTO forum_topic_subscriptions (topic_id, user_id, tenant_id) VALUES ('{topic_a}', '{user_id}', '{tenant_b}')"
            ),
            "cross-tenant topic subscription",
        ),
        (
            format!(
                "INSERT INTO forum_solutions (topic_id, tenant_id, reply_id) VALUES ('{topic_a}', '{tenant_b}', '{reply_b}')"
            ),
            "cross-tenant solution",
        ),
        (
            format!(
                "INSERT INTO forum_solutions (topic_id, tenant_id, reply_id) VALUES ('{topic_a}', '{tenant_a}', '{reply_a2}')"
            ),
            "solution reply from another topic",
        ),
        (
            format!(
                "INSERT INTO forum_topic_tags (id, topic_id, term_id, tenant_id) VALUES ('{}', '{topic_a}', '{term_a}', '{tenant_b}')",
                Uuid::new_v4()
            ),
            "cross-tenant topic tag",
        ),
        (
            format!(
                "INSERT INTO forum_topic_tags (id, topic_id, term_id, tenant_id) VALUES ('{}', '{topic_a}', '{term_b}', '{tenant_a}')",
                Uuid::new_v4()
            ),
            "cross-tenant taxonomy term",
        ),
    ] {
        assert_rejected(db, sql, label).await?;
    }

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_topic_votes (topic_id, user_id, tenant_id, value)
VALUES ('{topic_a}', '{user_id}', '{tenant_a}', 1);
INSERT INTO forum_reply_votes (reply_id, user_id, tenant_id, value)
VALUES ('{reply_a}', '{user_id}', '{tenant_a}', 1);
INSERT INTO forum_category_subscriptions (category_id, user_id, tenant_id)
VALUES ('{category_a}', '{user_id}', '{tenant_a}');
INSERT INTO forum_topic_subscriptions (topic_id, user_id, tenant_id)
VALUES ('{topic_a}', '{user_id}', '{tenant_a}');
INSERT INTO forum_solutions (topic_id, tenant_id, reply_id)
VALUES ('{topic_a}', '{tenant_a}', '{reply_a}');
INSERT INTO forum_topic_tags (id, topic_id, term_id, tenant_id)
VALUES ('{}', '{topic_a}', '{term_a}', '{tenant_a}');
"#,
            Uuid::new_v4()
        ),
    )
    .await?;

    Ok(())
}

async fn execute(db: &DatabaseConnection, sql: String) -> TestResult<()> {
    db.execute_unprepared(&sql).await?;
    Ok(())
}

async fn assert_rejected(db: &DatabaseConnection, sql: String, relation: &str) -> TestResult<()> {
    let result = db.execute_unprepared(&sql).await;
    assert!(result.is_err(), "{relation} must be rejected");
    Ok(())
}
