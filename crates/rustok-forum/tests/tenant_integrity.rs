use rustok_core::MigrationSource;
use rustok_forum::ForumModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn postgres_rejects_cross_tenant_forum_core_relations() -> TestResult<()> {
    let Some((db, schema_name)) = setup_postgres().await? else {
        return Ok(());
    };

    let result = exercise_tenant_constraints(&db).await;

    db.execute(Statement::from_string(
        DatabaseBackend::Postgres,
        format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#),
    ))
    .await?;

    result
}

async fn setup_postgres() -> TestResult<Option<(DatabaseConnection, String)>> {
    let database_url = match std::env::var("RUSTOK_FORUM_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
    {
        Ok(url)
            if url.starts_with("postgres://") || url.starts_with("postgresql://") =>
        {
            url
        }
        _ => return Ok(None),
    };

    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);

    let db = Database::connect(options).await?;
    let schema_name = format!("rustok_forum_tenant_{}", Uuid::new_v4().simple());

    db.execute(Statement::from_string(
        DatabaseBackend::Postgres,
        format!(r#"CREATE SCHEMA "{schema_name}""#),
    ))
    .await?;
    db.execute(Statement::from_string(
        DatabaseBackend::Postgres,
        format!(r#"SET search_path TO "{schema_name}""#),
    ))
    .await?;

    let manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }

    Ok(Some((db, schema_name)))
}

async fn exercise_tenant_constraints(db: &DatabaseConnection) -> TestResult<()> {
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let category_a = Uuid::new_v4();
    let category_b = Uuid::new_v4();
    let topic_a = Uuid::new_v4();
    let topic_b = Uuid::new_v4();
    let reply_a = Uuid::new_v4();

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_categories
    (id, tenant_id, position, moderated, topic_count, reply_count)
VALUES
    ('{category_a}', '{tenant_a}', 0, FALSE, 0, 0),
    ('{category_b}', '{tenant_b}', 0, FALSE, 0, 0)
"#
        ),
    )
    .await?;

    assert_rejected(
        db,
        format!(
            r#"
INSERT INTO forum_categories
    (id, tenant_id, parent_id, position, moderated, topic_count, reply_count)
VALUES
    ('{}', '{tenant_b}', '{category_a}', 0, FALSE, 0, 0)
"#,
            Uuid::new_v4()
        ),
        "cross-tenant category parent",
    )
    .await?;

    assert_rejected(
        db,
        format!(
            r#"
INSERT INTO forum_category_translations
    (id, category_id, tenant_id, locale, name, slug)
VALUES
    ('{}', '{category_a}', '{tenant_b}', 'en-US', 'Wrong tenant', 'wrong-tenant')
"#,
            Uuid::new_v4()
        ),
        "cross-tenant category translation",
    )
    .await?;

    assert_rejected(
        db,
        format!(
            r#"
INSERT INTO forum_topics
    (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
VALUES
    ('{topic_b}', '{tenant_b}', '{category_a}', 'open', '{{}}', FALSE, FALSE, 0)
"#
        ),
        "cross-tenant topic category",
    )
    .await?;

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_topics
    (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
VALUES
    ('{topic_a}', '{tenant_a}', '{category_a}', 'open', '{{}}', FALSE, FALSE, 0),
    ('{topic_b}', '{tenant_b}', '{category_b}', 'open', '{{}}', FALSE, FALSE, 0)
"#
        ),
    )
    .await?;

    assert_rejected(
        db,
        format!(
            r#"
INSERT INTO forum_replies
    (id, tenant_id, topic_id, status, position)
VALUES
    ('{}', '{tenant_b}', '{topic_a}', 'approved', 1)
"#,
            Uuid::new_v4()
        ),
        "cross-tenant reply topic",
    )
    .await?;

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_replies
    (id, tenant_id, topic_id, status, position)
VALUES
    ('{reply_a}', '{tenant_a}', '{topic_a}', 'approved', 1)
"#
        ),
    )
    .await?;

    assert_rejected(
        db,
        format!(
            r#"
INSERT INTO forum_replies
    (id, tenant_id, topic_id, parent_reply_id, status, position)
VALUES
    ('{}', '{tenant_b}', '{topic_b}', '{reply_a}', 'approved', 1)
"#,
            Uuid::new_v4()
        ),
        "cross-tenant parent reply",
    )
    .await?;

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_category_translations
    (id, category_id, tenant_id, locale, name, slug)
VALUES
    ('{}', '{category_a}', '{tenant_a}',
     'zh-Hant-HK', 'Long locale', 'long-locale')
"#,
            Uuid::new_v4()
        ),
    )
    .await?;

    Ok(())
}

async fn execute(db: &DatabaseConnection, sql: String) -> TestResult<()> {
    db.execute(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await?;
    Ok(())
}

async fn assert_rejected(
    db: &DatabaseConnection,
    sql: String,
    relation: &str,
) -> TestResult<()> {
    let result = db
        .execute(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await;
    assert!(result.is_err(), "{relation} must be rejected by PostgreSQL");
    Ok(())
}
