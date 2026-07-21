use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum relation seed migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum relation seed rollback does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE OR REPLACE FUNCTION forum_seed_topic_relation_revision()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM forum_relation_revisions revision
        WHERE revision.tenant_id = NEW.tenant_id
          AND revision.target_kind = 'topic'
          AND revision.target_id = NEW.topic_id
          AND revision.locale = NEW.locale
    ) THEN
        INSERT INTO forum_relation_revisions (
            tenant_id,
            target_kind,
            target_id,
            locale,
            projection_fingerprint
        ) VALUES (
            NEW.tenant_id,
            'topic',
            NEW.topic_id,
            NEW.locale,
            'legacy'
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_seed_reply_relation_revision()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM forum_relation_revisions revision
        WHERE revision.tenant_id = NEW.tenant_id
          AND revision.target_kind = 'reply'
          AND revision.target_id = NEW.reply_id
          AND revision.locale = NEW.locale
    ) THEN
        INSERT INTO forum_relation_revisions (
            tenant_id,
            target_kind,
            target_id,
            locale,
            projection_fingerprint
        ) VALUES (
            NEW.tenant_id,
            'reply',
            NEW.reply_id,
            NEW.locale,
            'legacy'
        );
    END IF;
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
"#,
        )
        .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TRIGGER IF EXISTS forum_reply_body_relation_revision_seed
    ON forum_reply_bodies;
DROP TRIGGER IF EXISTS forum_topic_translation_relation_revision_seed
    ON forum_topic_translations;
DROP FUNCTION IF EXISTS forum_seed_reply_relation_revision();
DROP FUNCTION IF EXISTS forum_seed_topic_relation_revision();
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_topic_translation_relation_revision_seed",
        r#"CREATE TRIGGER forum_topic_translation_relation_revision_seed
            AFTER INSERT ON forum_topic_translations
            FOR EACH ROW
            WHEN NOT EXISTS (
                SELECT 1
                FROM forum_relation_revisions revision
                WHERE revision.tenant_id = NEW.tenant_id
                  AND revision.target_kind = 'topic'
                  AND revision.target_id = NEW.topic_id
                  AND revision.locale = NEW.locale
            )
            BEGIN
                INSERT INTO forum_relation_revisions (
                    tenant_id,
                    target_kind,
                    target_id,
                    locale,
                    projection_fingerprint
                ) VALUES (
                    NEW.tenant_id,
                    'topic',
                    NEW.topic_id,
                    NEW.locale,
                    'legacy'
                );
            END"#,
        "DROP TRIGGER IF EXISTS forum_reply_body_relation_revision_seed",
        r#"CREATE TRIGGER forum_reply_body_relation_revision_seed
            AFTER INSERT ON forum_reply_bodies
            FOR EACH ROW
            WHEN NOT EXISTS (
                SELECT 1
                FROM forum_relation_revisions revision
                WHERE revision.tenant_id = NEW.tenant_id
                  AND revision.target_kind = 'reply'
                  AND revision.target_id = NEW.reply_id
                  AND revision.locale = NEW.locale
            )
            BEGIN
                INSERT INTO forum_relation_revisions (
                    tenant_id,
                    target_kind,
                    target_id,
                    locale,
                    projection_fingerprint
                ) VALUES (
                    NEW.tenant_id,
                    'reply',
                    NEW.reply_id,
                    NEW.locale,
                    'legacy'
                );
            END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_reply_body_relation_revision_seed",
        "DROP TRIGGER IF EXISTS forum_topic_translation_relation_revision_seed",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
