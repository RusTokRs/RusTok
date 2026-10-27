use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

/// Fingerprint recorded for a revision whose relation projection was never computed.
///
/// The append-only relation stream starts at `m20260722_000004`. Bodies written before that
/// migration, and bodies written afterwards by import, fork, split or translation writers that do
/// not run the relation owner, still have to be readable and reconcilable. They get one baseline
/// revision that says exactly that: the body exists, no projection was derived from it. The value
/// matches `valid_projection_fingerprint()` in `mention_reconciliation`, so a baseline is never
/// reported as drift.
const LEGACY_FINGERPRINT: &str = "legacy";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum relation revision seed does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // The baseline rows are append-only revision history, so the rollback removes only the
        // seeding mechanism. Deleting them would rewrite a stream the owner promises never to
        // rewrite, and a row that still describes the body it was written for stays truthful.
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum relation revision seed rollback does not support {backend:?}"
            ))),
        }
    }
}

const POSTGRES_UP: &str = r#"
CREATE OR REPLACE FUNCTION forum_seed_topic_relation_revision()
RETURNS trigger AS $$
BEGIN
    INSERT INTO forum_relation_revisions
        (tenant_id, target_kind, target_id, locale, projection_fingerprint)
    SELECT NEW.tenant_id, 'topic', NEW.topic_id, NEW.locale, 'legacy'
    WHERE NOT EXISTS (
        SELECT 1
        FROM forum_relation_revisions existing
        WHERE existing.tenant_id = NEW.tenant_id
          AND existing.target_kind = 'topic'
          AND existing.target_id = NEW.topic_id
          AND existing.locale = NEW.locale
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_seed_reply_relation_revision()
RETURNS trigger AS $$
BEGIN
    INSERT INTO forum_relation_revisions
        (tenant_id, target_kind, target_id, locale, projection_fingerprint)
    SELECT NEW.tenant_id, 'reply', NEW.reply_id, NEW.locale, 'legacy'
    WHERE NOT EXISTS (
        SELECT 1
        FROM forum_relation_revisions existing
        WHERE existing.tenant_id = NEW.tenant_id
          AND existing.target_kind = 'reply'
          AND existing.target_id = NEW.reply_id
          AND existing.locale = NEW.locale
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_topic_translation_relation_revision_seed
    ON forum_topic_translations;
CREATE TRIGGER forum_topic_translation_relation_revision_seed
AFTER INSERT ON forum_topic_translations
FOR EACH ROW
EXECUTE FUNCTION forum_seed_topic_relation_revision();

DROP TRIGGER IF EXISTS forum_reply_body_relation_revision_seed
    ON forum_reply_bodies;
CREATE TRIGGER forum_reply_body_relation_revision_seed
AFTER INSERT ON forum_reply_bodies
FOR EACH ROW
EXECUTE FUNCTION forum_seed_reply_relation_revision();

INSERT INTO forum_relation_revisions
    (tenant_id, target_kind, target_id, locale, projection_fingerprint)
SELECT translation.tenant_id, 'topic', translation.topic_id, translation.locale, 'legacy'
FROM forum_topic_translations translation
WHERE NOT EXISTS (
    SELECT 1
    FROM forum_relation_revisions existing
    WHERE existing.tenant_id = translation.tenant_id
      AND existing.target_kind = 'topic'
      AND existing.target_id = translation.topic_id
      AND existing.locale = translation.locale
);

INSERT INTO forum_relation_revisions
    (tenant_id, target_kind, target_id, locale, projection_fingerprint)
SELECT body.tenant_id, 'reply', body.reply_id, body.locale, 'legacy'
FROM forum_reply_bodies body
WHERE NOT EXISTS (
    SELECT 1
    FROM forum_relation_revisions existing
    WHERE existing.tenant_id = body.tenant_id
      AND existing.target_kind = 'reply'
      AND existing.target_id = body.reply_id
      AND existing.locale = body.locale
);
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_reply_body_relation_revision_seed ON forum_reply_bodies;
DROP TRIGGER IF EXISTS forum_topic_translation_relation_revision_seed
    ON forum_topic_translations;
DROP FUNCTION IF EXISTS forum_seed_reply_relation_revision();
DROP FUNCTION IF EXISTS forum_seed_topic_relation_revision();
"#;

const SQLITE_UP: &[&str] = &[
    r#"DROP TRIGGER IF EXISTS forum_topic_translation_relation_revision_seed"#,
    r#"CREATE TRIGGER forum_topic_translation_relation_revision_seed
AFTER INSERT ON forum_topic_translations
FOR EACH ROW
BEGIN
    INSERT INTO forum_relation_revisions
        (tenant_id, target_kind, target_id, locale, projection_fingerprint)
    SELECT NEW.tenant_id, 'topic', NEW.topic_id, NEW.locale, 'legacy'
    WHERE NOT EXISTS (
        SELECT 1
        FROM forum_relation_revisions existing
        WHERE existing.tenant_id = NEW.tenant_id
          AND existing.target_kind = 'topic'
          AND existing.target_id = NEW.topic_id
          AND existing.locale = NEW.locale
    );
END"#,
    r#"DROP TRIGGER IF EXISTS forum_reply_body_relation_revision_seed"#,
    r#"CREATE TRIGGER forum_reply_body_relation_revision_seed
AFTER INSERT ON forum_reply_bodies
FOR EACH ROW
BEGIN
    INSERT INTO forum_relation_revisions
        (tenant_id, target_kind, target_id, locale, projection_fingerprint)
    SELECT NEW.tenant_id, 'reply', NEW.reply_id, NEW.locale, 'legacy'
    WHERE NOT EXISTS (
        SELECT 1
        FROM forum_relation_revisions existing
        WHERE existing.tenant_id = NEW.tenant_id
          AND existing.target_kind = 'reply'
          AND existing.target_id = NEW.reply_id
          AND existing.locale = NEW.locale
    );
END"#,
    r#"INSERT INTO forum_relation_revisions
    (tenant_id, target_kind, target_id, locale, projection_fingerprint)
SELECT translation.tenant_id, 'topic', translation.topic_id, translation.locale, 'legacy'
FROM forum_topic_translations translation
WHERE NOT EXISTS (
    SELECT 1
    FROM forum_relation_revisions existing
    WHERE existing.tenant_id = translation.tenant_id
      AND existing.target_kind = 'topic'
      AND existing.target_id = translation.topic_id
      AND existing.locale = translation.locale
)"#,
    r#"INSERT INTO forum_relation_revisions
    (tenant_id, target_kind, target_id, locale, projection_fingerprint)
SELECT body.tenant_id, 'reply', body.reply_id, body.locale, 'legacy'
FROM forum_reply_bodies body
WHERE NOT EXISTS (
    SELECT 1
    FROM forum_relation_revisions existing
    WHERE existing.tenant_id = body.tenant_id
      AND existing.target_kind = 'reply'
      AND existing.target_id = body.reply_id
      AND existing.locale = body.locale
)"#,
];

const SQLITE_DOWN: &[&str] = &[
    r#"DROP TRIGGER IF EXISTS forum_reply_body_relation_revision_seed"#,
    r#"DROP TRIGGER IF EXISTS forum_topic_translation_relation_revision_seed"#,
];

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(POSTGRES_UP)
        .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(POSTGRES_DOWN)
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in SQLITE_UP {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in SQLITE_DOWN {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
