use std::{env, error::Error};

use rustok_core::MigrationSource;
use rustok_forum::ForumModule;
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_FORUM_TEST_DATABASE_URL";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone, Copy)]
struct ForumSeed {
    tenant_id: Uuid,
    category_id: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
}

struct PostgresHarness {
    control: DatabaseConnection,
    database_url: String,
}

impl PostgresHarness {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum moderation-revision PostgreSQL migration contract"
            );
            return Ok(None);
        };
        Ok(Some(Self {
            control: connect_postgres(&database_url).await?,
            database_url,
        }))
    }

    async fn create_schema(&self, label: &str) -> TestResult<(String, DatabaseConnection)> {
        let schema = format!(
            "rustok_forum_moderation_revision_{}_{}",
            label,
            Uuid::new_v4().simple()
        );
        self.control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema}""#))
            .await?;
        let db = connect_postgres(&self.database_url).await?;
        db.execute_unprepared(&format!(r#"SET search_path TO "{schema}""#))
            .await?;
        Ok((schema, db))
    }

    async fn drop_schema(&self, schema: &str) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema}" CASCADE"#))
            .await?;
        Ok(())
    }
}

#[tokio::test]
async fn sqlite_moderation_revision_migration_backfills_and_tracks_owner_changes() -> TestResult<()>
{
    let db = sqlite_database().await?;
    install_prerequisites(&db).await?;
    let revision_migration = install_forum_before_revision_migration(&db).await?;
    let seed = seed_existing_subjects(&db).await?;

    revision_migration.up(&SchemaManager::new(&db)).await?;
    assert_backfilled_revisions(&db, seed).await?;
    exercise_revision_triggers(&db, seed).await?;
    assert_new_subject_initialization(&db, seed).await?;

    db.close().await?;
    Ok(())
}

#[tokio::test]
async fn sqlite_clean_install_initializes_moderation_revision_clocks() -> TestResult<()> {
    let db = sqlite_database().await?;
    install_prerequisites(&db).await?;
    install_all_forum_migrations(&db).await?;
    let seed = seed_existing_subjects(&db).await?;
    assert_new_subject_revisions(&db, seed.topic_id, seed.reply_id).await?;
    db.close().await?;
    Ok(())
}

#[tokio::test]
async fn postgres_moderation_revision_migration_clean_upgrade_and_trigger_contract()
-> TestResult<()> {
    let Some(harness) = PostgresHarness::setup().await? else {
        return Ok(());
    };

    let (upgrade_schema, upgrade_db) = harness.create_schema("upgrade").await?;
    let upgrade_result = async {
        install_prerequisites(&upgrade_db).await?;
        let revision_migration = install_forum_before_revision_migration(&upgrade_db).await?;
        let seed = seed_existing_subjects(&upgrade_db).await?;
        revision_migration
            .up(&SchemaManager::new(&upgrade_db))
            .await?;
        assert_backfilled_revisions(&upgrade_db, seed).await?;
        exercise_revision_triggers(&upgrade_db, seed).await?;
        assert_new_subject_initialization(&upgrade_db, seed).await
    }
    .await;
    upgrade_db.close().await?;
    harness.drop_schema(&upgrade_schema).await?;
    upgrade_result?;

    let (clean_schema, clean_db) = harness.create_schema("clean").await?;
    let clean_result = async {
        install_prerequisites(&clean_db).await?;
        install_all_forum_migrations(&clean_db).await?;
        let seed = seed_existing_subjects(&clean_db).await?;
        assert_new_subject_revisions(&clean_db, seed.topic_id, seed.reply_id).await
    }
    .await;
    clean_db.close().await?;
    harness.drop_schema(&clean_schema).await?;
    clean_result?;

    harness.control.close().await?;
    Ok(())
}

fn sql_uuid(backend: sea_orm::DatabaseBackend, id: Uuid) -> String {
    match backend {
        sea_orm::DatabaseBackend::Sqlite => format!("X'{}'", hex::encode(id.as_bytes())),
        _ => format!("'{id}'"),
    }
}

async fn install_prerequisites(db: &DatabaseConnection) -> TestResult<()> {
    let id_type = match db.get_database_backend() {
        sea_orm::DatabaseBackend::Sqlite => "TEXT",
        _ => "UUID",
    };
    db.execute_unprepared(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id {id_type} NOT NULL PRIMARY KEY,
            tenant_id {id_type} NOT NULL
        )
        "#
    ))
    .await?;
    let manager = SchemaManager::new(db);
    for migration in OutboxModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    flex::cache_generation::create_field_definition_cache_generation_table(&manager).await?;
    Ok(())
}

async fn install_forum_before_revision_migration(
    db: &DatabaseConnection,
) -> TestResult<Box<dyn sea_orm_migration::MigrationTrait>> {
    let manager = SchemaManager::new(db);
    let mut migrations = ForumModule.migrations();
    let index = migrations
        .iter()
        .position(|m| m.name().contains("add_forum_moderation_subject_revisions"))
        .ok_or_else(|| test_error("Moderation revision migration not found"))?;
    let before: Vec<_> = migrations.drain(..index).collect();
    let revision_migration = migrations.remove(0);
    for migration in before {
        migration.up(&manager).await?;
    }
    Ok(revision_migration)
}

async fn install_all_forum_migrations(db: &DatabaseConnection) -> TestResult<()> {
    let manager = SchemaManager::new(db);
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(())
}

async fn seed_existing_subjects(db: &DatabaseConnection) -> TestResult<ForumSeed> {
    let seed = ForumSeed {
        tenant_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        topic_id: Uuid::new_v4(),
        reply_id: Uuid::new_v4(),
    };
    let backend = db.get_database_backend();
    let cat_id = sql_uuid(backend, seed.category_id);
    let tenant_id = sql_uuid(backend, seed.tenant_id);
    let top_id = sql_uuid(backend, seed.topic_id);
    let rep_id = sql_uuid(backend, seed.reply_id);
    db.execute_unprepared(&format!(
        r#"
        INSERT INTO forum_categories
            (id, tenant_id, position, moderated, topic_count, reply_count)
        VALUES
            ({cat_id}, {tenant_id}, 0, FALSE, 1, 1);
        INSERT INTO forum_topics
            (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
        VALUES
            ({top_id}, {tenant_id}, {cat_id}, 'open', '{{}}', FALSE, FALSE, 1);
        INSERT INTO forum_replies
            (id, tenant_id, topic_id, status, position)
        VALUES
            ({rep_id}, {tenant_id}, {top_id}, 'approved', 1);
        "#
    ))
    .await?;
    Ok(seed)
}

async fn assert_backfilled_revisions(db: &DatabaseConnection, seed: ForumSeed) -> TestResult<()> {
    assert_eq!(topic_revision(db, seed.tenant_id, seed.topic_id).await?, 1);
    assert_eq!(reply_revision(db, seed.tenant_id, seed.reply_id).await?, 1);
    assert_eq!(
        scalar_i64(
            db,
            "SELECT COUNT(*) AS value FROM forum_topic_moderation_subject_revisions",
        )
        .await?,
        1
    );
    assert_eq!(
        scalar_i64(
            db,
            "SELECT COUNT(*) AS value FROM forum_reply_moderation_subject_revisions",
        )
        .await?,
        1
    );
    Ok(())
}

async fn exercise_revision_triggers(db: &DatabaseConnection, seed: ForumSeed) -> TestResult<()> {
    let backend = db.get_database_backend();
    let tid = sql_uuid(backend, seed.tenant_id);
    let topid = sql_uuid(backend, seed.topic_id);
    let repid = sql_uuid(backend, seed.reply_id);

    db.execute_unprepared(&format!(
        "UPDATE forum_topics SET metadata = '{{\"clock\":true}}' WHERE tenant_id = {tid} AND id = {topid}"
    ))
    .await?;
    assert_eq!(topic_revision(db, seed.tenant_id, seed.topic_id).await?, 2);

    db.execute_unprepared(&format!(
        "UPDATE forum_topics SET is_locked = TRUE WHERE tenant_id = {tid} AND id = {topid}"
    ))
    .await?;
    assert_eq!(topic_revision(db, seed.tenant_id, seed.topic_id).await?, 3);

    let topic_translation_id = Uuid::new_v4();
    let ttid = sql_uuid(backend, topic_translation_id);
    db.execute_unprepared(&format!(
        r#"
        INSERT INTO forum_topic_translations
            (id, tenant_id, topic_id, locale, title, slug, body)
        VALUES
            ({ttid}, {tid}, {topid}, 'en', 'Clock title', 'clock-title', 'Clock body')
        "#
    ))
    .await?;
    assert_eq!(topic_revision(db, seed.tenant_id, seed.topic_id).await?, 4);

    db.execute_unprepared(&format!(
        "UPDATE forum_topic_translations SET title = 'Clock title edited' WHERE tenant_id = {tid} AND id = {ttid}"
    ))
    .await?;
    assert_eq!(topic_revision(db, seed.tenant_id, seed.topic_id).await?, 5);

    db.execute_unprepared(&format!(
        "DELETE FROM forum_topic_translations WHERE tenant_id = {tid} AND id = {ttid}"
    ))
    .await?;
    assert_eq!(topic_revision(db, seed.tenant_id, seed.topic_id).await?, 6);

    db.execute_unprepared(&format!(
        "UPDATE forum_topics SET reply_count = reply_count WHERE tenant_id = {tid} AND id = {topid}"
    ))
    .await?;
    assert_eq!(topic_revision(db, seed.tenant_id, seed.topic_id).await?, 6);

    db.execute_unprepared(&format!(
        "UPDATE forum_replies SET status = 'hidden' WHERE tenant_id = {tid} AND id = {repid}"
    ))
    .await?;
    assert_eq!(reply_revision(db, seed.tenant_id, seed.reply_id).await?, 2);

    let body_id = Uuid::new_v4();
    let bid = sql_uuid(backend, body_id);
    db.execute_unprepared(&format!(
        r#"
        INSERT INTO forum_reply_bodies
            (id, tenant_id, reply_id, locale, body)
        VALUES
            ({bid}, {tid}, {repid}, 'en', 'Clock reply body')
        "#
    ))
    .await?;
    assert_eq!(reply_revision(db, seed.tenant_id, seed.reply_id).await?, 3);

    db.execute_unprepared(&format!(
        "UPDATE forum_reply_bodies SET body = 'Clock reply body edited' WHERE tenant_id = {tid} AND id = {bid}"
    ))
    .await?;
    assert_eq!(reply_revision(db, seed.tenant_id, seed.reply_id).await?, 4);

    db.execute_unprepared(&format!(
        "DELETE FROM forum_reply_bodies WHERE tenant_id = {tid} AND id = {bid}"
    ))
    .await?;
    assert_eq!(reply_revision(db, seed.tenant_id, seed.reply_id).await?, 5);

    db.execute_unprepared(&format!(
        "UPDATE forum_replies SET updated_at = updated_at WHERE tenant_id = {tid} AND id = {repid}"
    ))
    .await?;
    assert_eq!(reply_revision(db, seed.tenant_id, seed.reply_id).await?, 5);
    Ok(())
}

async fn assert_new_subject_initialization(
    db: &DatabaseConnection,
    seed: ForumSeed,
) -> TestResult<()> {
    let new_topic = Uuid::new_v4();
    let new_reply = Uuid::new_v4();
    let backend = db.get_database_backend();
    let ntopid = sql_uuid(backend, new_topic);
    let nrepid = sql_uuid(backend, new_reply);
    let tid = sql_uuid(backend, seed.tenant_id);
    let cid = sql_uuid(backend, seed.category_id);
    db.execute_unprepared(&format!(
        r#"
        INSERT INTO forum_topics
            (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
        VALUES
            ({ntopid}, {tid}, {cid}, 'open', '{{}}', FALSE, FALSE, 1);
        INSERT INTO forum_replies
            (id, tenant_id, topic_id, status, position)
        VALUES
            ({nrepid}, {tid}, {ntopid}, 'approved', 2);
        "#
    ))
    .await?;
    assert_new_subject_revisions(db, new_topic, new_reply).await
}

async fn assert_new_subject_revisions(
    db: &DatabaseConnection,
    topic_id: Uuid,
    reply_id: Uuid,
) -> TestResult<()> {
    let backend = db.get_database_backend();
    let topid = sql_uuid(backend, topic_id);
    let repid = sql_uuid(backend, reply_id);
    let topic = scalar_i64(
        db,
        &format!(
            "SELECT revision AS value FROM forum_topic_moderation_subject_revisions WHERE topic_id = {topid}"
        ),
    )
    .await?;
    let reply = scalar_i64(
        db,
        &format!(
            "SELECT revision AS value FROM forum_reply_moderation_subject_revisions WHERE reply_id = {repid}"
        ),
    )
    .await?;
    assert_eq!(topic, 1);
    assert_eq!(reply, 1);
    Ok(())
}

async fn topic_revision(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<i64> {
    let backend = db.get_database_backend();
    let tid = sql_uuid(backend, tenant_id);
    let topid = sql_uuid(backend, topic_id);
    scalar_i64(
        db,
        &format!(
            "SELECT revision AS value FROM forum_topic_moderation_subject_revisions WHERE tenant_id = {tid} AND topic_id = {topid}"
        ),
    )
    .await
}

async fn reply_revision(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> TestResult<i64> {
    let backend = db.get_database_backend();
    let tid = sql_uuid(backend, tenant_id);
    let repid = sql_uuid(backend, reply_id);
    scalar_i64(
        db,
        &format!(
            "SELECT revision AS value FROM forum_reply_moderation_subject_revisions WHERE tenant_id = {tid} AND reply_id = {repid}"
        ),
    )
    .await
}

async fn scalar_i64(db: &DatabaseConnection, sql: &str) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            sql.to_string(),
        ))
        .await?
        .ok_or_else(|| test_error("Forum moderation revision scalar query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

async fn sqlite_database() -> TestResult<DatabaseConnection> {
    let db = Database::connect("sqlite::memory:").await?;
    db.execute_unprepared("PRAGMA foreign_keys = ON").await?;
    Ok(db)
}

fn postgres_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect_postgres(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

fn test_error(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    std::io::Error::other(message.into()).into()
}
