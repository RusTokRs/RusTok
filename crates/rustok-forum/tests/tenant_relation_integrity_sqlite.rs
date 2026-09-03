use rustok_core::MigrationSource;
use rustok_forum::ForumModule;
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn sql_uuid(id: Uuid) -> String {
    format!("X'{}'", id.simple().to_string().to_uppercase())
}

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

    let manager = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
    }
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        );",
    )
    .await
    .expect("users table fixture should apply");
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
    ({}, {}, 0, 0, 0, 0),
    ({}, {}, 0, 0, 0, 0);

INSERT INTO forum_topics
    (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
VALUES
    ({}, {}, {}, 'open', '{{}}', 0, 0, 0),
    ({}, {}, {}, 'open', '{{}}', 0, 0, 0),
    ({}, {}, {}, 'open', '{{}}', 0, 0, 0);

INSERT INTO forum_replies
    (id, tenant_id, topic_id, status, position)
VALUES
    ({}, {}, {}, 'approved', 1),
    ({}, {}, {}, 'approved', 1),
    ({}, {}, {}, 'approved', 1);

INSERT INTO taxonomy_terms
    (id, tenant_id, kind, scope_type, scope_value, canonical_key, revision)
VALUES
    ({}, {}, 'tag', 'module', 'forum', 'tenant-a-tag', 1),
    ({}, {}, 'tag', 'module', 'forum', 'tenant-b-tag', 1);
"#,
            sql_uuid(category_a),
            sql_uuid(tenant_a),
            sql_uuid(category_b),
            sql_uuid(tenant_b),
            sql_uuid(topic_a),
            sql_uuid(tenant_a),
            sql_uuid(category_a),
            sql_uuid(topic_a2),
            sql_uuid(tenant_a),
            sql_uuid(category_a),
            sql_uuid(topic_b),
            sql_uuid(tenant_b),
            sql_uuid(category_b),
            sql_uuid(reply_a),
            sql_uuid(tenant_a),
            sql_uuid(topic_a),
            sql_uuid(reply_a2),
            sql_uuid(tenant_a),
            sql_uuid(topic_a2),
            sql_uuid(reply_b),
            sql_uuid(tenant_b),
            sql_uuid(topic_b),
            sql_uuid(term_a),
            sql_uuid(tenant_a),
            sql_uuid(term_b),
            sql_uuid(tenant_b),
        ),
    )
    .await?;

    for (sql, label) in [
        (
            format!(
                "INSERT INTO forum_topic_votes (topic_id, user_id, tenant_id, value) VALUES ({}, {}, {}, 1)",
                sql_uuid(topic_a),
                sql_uuid(user_id),
                sql_uuid(tenant_b)
            ),
            "cross-tenant topic vote",
        ),
        (
            format!(
                "INSERT INTO forum_reply_votes (reply_id, user_id, tenant_id, value) VALUES ({}, {}, {}, 1)",
                sql_uuid(reply_a),
                sql_uuid(user_id),
                sql_uuid(tenant_b)
            ),
            "cross-tenant reply vote",
        ),
        (
            format!(
                "INSERT INTO forum_category_subscriptions (category_id, user_id, tenant_id, updated_at) VALUES ({}, {}, {}, CURRENT_TIMESTAMP)",
                sql_uuid(category_a),
                sql_uuid(user_id),
                sql_uuid(tenant_b)
            ),
            "cross-tenant category subscription",
        ),
        (
            format!(
                "INSERT INTO forum_topic_subscriptions (topic_id, user_id, tenant_id, updated_at) VALUES ({}, {}, {}, CURRENT_TIMESTAMP)",
                sql_uuid(topic_a),
                sql_uuid(user_id),
                sql_uuid(tenant_b)
            ),
            "cross-tenant topic subscription",
        ),
        (
            format!(
                "INSERT INTO forum_solutions (topic_id, tenant_id, reply_id) VALUES ({}, {}, {})",
                sql_uuid(topic_a),
                sql_uuid(tenant_b),
                sql_uuid(reply_b)
            ),
            "cross-tenant solution",
        ),
        (
            format!(
                "INSERT INTO forum_solutions (topic_id, tenant_id, reply_id) VALUES ({}, {}, {})",
                sql_uuid(topic_a),
                sql_uuid(tenant_a),
                sql_uuid(reply_a2)
            ),
            "solution reply from another topic",
        ),
        (
            format!(
                "INSERT INTO forum_topic_tags (id, topic_id, term_id, tenant_id) VALUES ({}, {}, {}, {})",
                sql_uuid(Uuid::new_v4()),
                sql_uuid(topic_a),
                sql_uuid(term_a),
                sql_uuid(tenant_b)
            ),
            "cross-tenant topic tag",
        ),
        (
            format!(
                "INSERT INTO forum_topic_tags (id, topic_id, term_id, tenant_id) VALUES ({}, {}, {}, {})",
                sql_uuid(Uuid::new_v4()),
                sql_uuid(topic_a),
                sql_uuid(term_b),
                sql_uuid(tenant_a)
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
VALUES ({}, {}, {}, 1);
INSERT INTO forum_reply_votes (reply_id, user_id, tenant_id, value)
VALUES ({}, {}, {}, 1);
INSERT INTO forum_category_subscriptions (category_id, user_id, tenant_id, updated_at)
VALUES ({}, {}, {}, CURRENT_TIMESTAMP);
INSERT INTO forum_topic_subscriptions (topic_id, user_id, tenant_id, updated_at)
VALUES ({}, {}, {}, CURRENT_TIMESTAMP);
INSERT INTO forum_solutions (topic_id, tenant_id, reply_id)
VALUES ({}, {}, {});
INSERT INTO forum_topic_tags (id, topic_id, term_id, tenant_id)
VALUES ({}, {}, {}, {});
"#,
            sql_uuid(topic_a),
            sql_uuid(user_id),
            sql_uuid(tenant_a),
            sql_uuid(reply_a),
            sql_uuid(user_id),
            sql_uuid(tenant_a),
            sql_uuid(category_a),
            sql_uuid(user_id),
            sql_uuid(tenant_a),
            sql_uuid(topic_a),
            sql_uuid(user_id),
            sql_uuid(tenant_a),
            sql_uuid(topic_a),
            sql_uuid(tenant_a),
            sql_uuid(reply_a),
            sql_uuid(Uuid::new_v4()),
            sql_uuid(topic_a),
            sql_uuid(term_a),
            sql_uuid(tenant_a),
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
