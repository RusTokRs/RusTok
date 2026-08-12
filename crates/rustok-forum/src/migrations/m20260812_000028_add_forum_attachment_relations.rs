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
                "rustok-forum attachment relation migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum attachment relation rollback does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE TABLE IF NOT EXISTS forum_attachment_relation_revisions (
    tenant_id UUID NOT NULL,
    target_kind VARCHAR(16) NOT NULL,
    target_id UUID NOT NULL,
    source_revision BIGINT NOT NULL,
    locale VARCHAR(32) NOT NULL,
    projection_fingerprint VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, target_kind, target_id, source_revision, locale),
    CONSTRAINT chk_forum_attachment_revision_target_kind
        CHECK (target_kind IN ('topic', 'reply')),
    CONSTRAINT chk_forum_attachment_revision_source_revision
        CHECK (source_revision > 0),
    CONSTRAINT chk_forum_attachment_revision_fingerprint
        CHECK (char_length(projection_fingerprint) = 64)
);

CREATE TABLE IF NOT EXISTS forum_attachment_relations (
    tenant_id UUID NOT NULL,
    target_kind VARCHAR(16) NOT NULL,
    target_id UUID NOT NULL,
    source_revision BIGINT NOT NULL,
    locale VARCHAR(32) NOT NULL,
    position INTEGER NOT NULL,
    media_id UUID NOT NULL,
    usage VARCHAR(16) NOT NULL,
    caption TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (
        tenant_id, target_kind, target_id, source_revision, locale, position
    ),
    CONSTRAINT chk_forum_attachment_relation_target_kind
        CHECK (target_kind IN ('topic', 'reply')),
    CONSTRAINT chk_forum_attachment_relation_source_revision
        CHECK (source_revision > 0),
    CONSTRAINT chk_forum_attachment_relation_position
        CHECK (position >= 0 AND position < 32),
    CONSTRAINT chk_forum_attachment_relation_usage
        CHECK (usage IN ('inline', 'attachment')),
    CONSTRAINT fk_forum_attachment_relation_revision
        FOREIGN KEY (tenant_id, target_kind, target_id, source_revision, locale)
        REFERENCES forum_attachment_relation_revisions
            (tenant_id, target_kind, target_id, source_revision, locale)
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_forum_attachment_relations_media
    ON forum_attachment_relations (tenant_id, media_id);

CREATE OR REPLACE FUNCTION forum_reject_attachment_relation_update()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum attachment relation projections are immutable';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_attachment_revision_immutable_guard
    ON forum_attachment_relation_revisions;
CREATE TRIGGER forum_attachment_revision_immutable_guard
BEFORE UPDATE ON forum_attachment_relation_revisions
FOR EACH ROW
EXECUTE FUNCTION forum_reject_attachment_relation_update();

DROP TRIGGER IF EXISTS forum_attachment_relation_immutable_guard
    ON forum_attachment_relations;
CREATE TRIGGER forum_attachment_relation_immutable_guard
BEFORE UPDATE ON forum_attachment_relations
FOR EACH ROW
EXECUTE FUNCTION forum_reject_attachment_relation_update();
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
DROP TRIGGER IF EXISTS forum_attachment_relation_immutable_guard
    ON forum_attachment_relations;
DROP TRIGGER IF EXISTS forum_attachment_revision_immutable_guard
    ON forum_attachment_relation_revisions;
DROP FUNCTION IF EXISTS forum_reject_attachment_relation_update();
DROP TABLE IF EXISTS forum_attachment_relations;
DROP TABLE IF EXISTS forum_attachment_relation_revisions;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        r#"CREATE TABLE IF NOT EXISTS forum_attachment_relation_revisions (
            tenant_id TEXT NOT NULL,
            target_kind TEXT NOT NULL CHECK (target_kind IN ('topic', 'reply')),
            target_id TEXT NOT NULL,
            source_revision INTEGER NOT NULL CHECK (source_revision > 0),
            locale TEXT NOT NULL,
            projection_fingerprint TEXT NOT NULL CHECK (length(projection_fingerprint) = 64),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, target_kind, target_id, source_revision, locale)
        )"#,
        r#"CREATE TABLE IF NOT EXISTS forum_attachment_relations (
            tenant_id TEXT NOT NULL,
            target_kind TEXT NOT NULL CHECK (target_kind IN ('topic', 'reply')),
            target_id TEXT NOT NULL,
            source_revision INTEGER NOT NULL CHECK (source_revision > 0),
            locale TEXT NOT NULL,
            position INTEGER NOT NULL CHECK (position >= 0 AND position < 32),
            media_id TEXT NOT NULL,
            usage TEXT NOT NULL CHECK (usage IN ('inline', 'attachment')),
            caption TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (
                tenant_id, target_kind, target_id, source_revision, locale, position
            ),
            FOREIGN KEY (tenant_id, target_kind, target_id, source_revision, locale)
                REFERENCES forum_attachment_relation_revisions
                    (tenant_id, target_kind, target_id, source_revision, locale)
                ON DELETE CASCADE
        )"#,
        r#"CREATE INDEX IF NOT EXISTS idx_forum_attachment_relations_media
            ON forum_attachment_relations (tenant_id, media_id)"#,
        "DROP TRIGGER IF EXISTS forum_attachment_revision_immutable_guard",
        r#"CREATE TRIGGER forum_attachment_revision_immutable_guard
            BEFORE UPDATE ON forum_attachment_relation_revisions
            FOR EACH ROW
            BEGIN
                SELECT RAISE(ABORT, 'forum attachment relation projections are immutable');
            END"#,
        "DROP TRIGGER IF EXISTS forum_attachment_relation_immutable_guard",
        r#"CREATE TRIGGER forum_attachment_relation_immutable_guard
            BEFORE UPDATE ON forum_attachment_relations
            FOR EACH ROW
            BEGIN
                SELECT RAISE(ABORT, 'forum attachment relation projections are immutable');
            END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_attachment_relation_immutable_guard",
        "DROP TRIGGER IF EXISTS forum_attachment_revision_immutable_guard",
        "DROP INDEX IF EXISTS idx_forum_attachment_relations_media",
        "DROP TABLE IF EXISTS forum_attachment_relations",
        "DROP TABLE IF EXISTS forum_attachment_relation_revisions",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
