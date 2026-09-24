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
                "rustok-forum attachment relations do not support {backend:?}"
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
    manager.get_connection().execute_unprepared(
        r#"
CREATE TABLE forum_attachment_relation_heads (
    tenant_id UUID NOT NULL,
    target_kind VARCHAR(16) NOT NULL
        CHECK (target_kind IN ('topic', 'reply')),
    target_id UUID NOT NULL,
    locale VARCHAR(32) NOT NULL,
    relation_revision BIGINT NOT NULL
        CHECK (relation_revision > 0),
    source_revision BIGINT NOT NULL
        CHECK (source_revision > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, target_kind, target_id, locale),
    FOREIGN KEY (tenant_id) REFERENCES tenants (id) ON DELETE CASCADE
);

CREATE TABLE forum_attachment_relations (
    reference_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    target_kind VARCHAR(16) NOT NULL
        CHECK (target_kind IN ('topic', 'reply')),
    target_id UUID NOT NULL,
    locale VARCHAR(32) NOT NULL,
    position INTEGER NOT NULL
        CHECK (position >= 0 AND position < 32),
    media_id UUID NOT NULL,
    usage VARCHAR(16) NOT NULL
        CHECK (usage IN ('inline', 'attachment')),
    caption VARCHAR(512),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id, target_kind, target_id, locale)
        REFERENCES forum_attachment_relation_heads
            (tenant_id, target_kind, target_id, locale)
        ON DELETE CASCADE
);

CREATE UNIQUE INDEX uq_forum_attachment_relations_target_position
    ON forum_attachment_relations
        (tenant_id, target_kind, target_id, locale, position);

CREATE INDEX idx_forum_attachment_relations_media
    ON forum_attachment_relations (tenant_id, media_id);

CREATE OR REPLACE FUNCTION forum_validate_attachment_relation_head()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND (
        NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
        OR NEW.target_kind IS DISTINCT FROM OLD.target_kind
        OR NEW.target_id IS DISTINCT FROM OLD.target_id
        OR NEW.locale IS DISTINCT FROM OLD.locale
    ) THEN
        RAISE EXCEPTION 'forum attachment relation head identity is immutable';
    END IF;

    IF TG_OP = 'UPDATE' AND NEW.relation_revision <= OLD.relation_revision THEN
        RAISE EXCEPTION 'forum attachment relation revision must advance monotonically';
    END IF;

    IF NEW.target_kind = 'topic' THEN
        IF NOT EXISTS (
            SELECT 1 FROM forum_topics topic
            WHERE topic.tenant_id = NEW.tenant_id
              AND topic.id = NEW.target_id
        ) THEN
            RAISE EXCEPTION 'forum attachment relation topic target does not exist';
        END IF;
    ELSIF NEW.target_kind = 'reply' THEN
        IF NOT EXISTS (
            SELECT 1 FROM forum_replies reply
            WHERE reply.tenant_id = NEW.tenant_id
              AND reply.id = NEW.target_id
        ) THEN
            RAISE EXCEPTION 'forum attachment relation reply target does not exist';
        END IF;
    ELSE
        RAISE EXCEPTION 'forum attachment relation target kind is invalid';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_attachment_relation_head_guard
    ON forum_attachment_relation_heads;
CREATE TRIGGER forum_attachment_relation_head_guard
BEFORE INSERT OR UPDATE ON forum_attachment_relation_heads
FOR EACH ROW
EXECUTE FUNCTION forum_validate_attachment_relation_head();

CREATE OR REPLACE FUNCTION forum_forbid_attachment_relation_update()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum attachment relation rows are immutable; replace the relation set';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER forum_attachment_relation_update_guard
BEFORE UPDATE ON forum_attachment_relations
FOR EACH ROW
EXECUTE FUNCTION forum_forbid_attachment_relation_update();
"#,
    ).await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(
        r#"
DROP TRIGGER IF EXISTS forum_attachment_relation_update_guard
    ON forum_attachment_relations;
DROP FUNCTION IF EXISTS forum_forbid_attachment_relation_update();
DROP TRIGGER IF EXISTS forum_attachment_relation_head_guard
    ON forum_attachment_relation_heads;
DROP FUNCTION IF EXISTS forum_validate_attachment_relation_head();
DROP INDEX IF EXISTS idx_forum_attachment_relations_media;
DROP INDEX IF EXISTS uq_forum_attachment_relations_target_position;
DROP TABLE IF EXISTS forum_attachment_relations;
DROP TABLE IF EXISTS forum_attachment_relation_heads;
"#,
    ).await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    for statement in [
        r#"CREATE TABLE forum_attachment_relation_heads (
            tenant_id TEXT NOT NULL,
            target_kind TEXT NOT NULL CHECK (target_kind IN ('topic', 'reply')),
            target_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            relation_revision INTEGER NOT NULL CHECK (relation_revision > 0),
            source_revision INTEGER NOT NULL CHECK (source_revision > 0),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, target_kind, target_id, locale),
            FOREIGN KEY (tenant_id) REFERENCES tenants (id) ON DELETE CASCADE
        )"#,
        r#"CREATE TABLE forum_attachment_relations (
            reference_id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            target_kind TEXT NOT NULL CHECK (target_kind IN ('topic', 'reply')),
            target_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            position INTEGER NOT NULL CHECK (position >= 0 AND position < 32),
            media_id TEXT NOT NULL,
            usage TEXT NOT NULL CHECK (usage IN ('inline', 'attachment')),
            caption TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (tenant_id, target_kind, target_id, locale)
                REFERENCES forum_attachment_relation_heads
                    (tenant_id, target_kind, target_id, locale)
                ON DELETE CASCADE
        )"#,
        "CREATE UNIQUE INDEX uq_forum_attachment_relations_target_position ON forum_attachment_relations (tenant_id, target_kind, target_id, locale, position)",
        "CREATE INDEX idx_forum_attachment_relations_media ON forum_attachment_relations (tenant_id, media_id)",
        r#"CREATE TRIGGER forum_attachment_relation_head_insert_guard
            BEFORE INSERT ON forum_attachment_relation_heads
            FOR EACH ROW
            WHEN (NEW.target_kind = 'topic' AND NOT EXISTS (
                SELECT 1 FROM forum_topics topic
                WHERE topic.tenant_id = NEW.tenant_id AND topic.id = NEW.target_id
            ))
            OR (NEW.target_kind = 'reply' AND NOT EXISTS (
                SELECT 1 FROM forum_replies reply
                WHERE reply.tenant_id = NEW.tenant_id AND reply.id = NEW.target_id
            ))
            OR NEW.target_kind NOT IN ('topic', 'reply')
            BEGIN
                SELECT RAISE(ABORT, 'forum attachment relation target does not exist');
            END"#,
        r#"CREATE TRIGGER forum_attachment_relation_head_revision_guard
            BEFORE UPDATE OF relation_revision ON forum_attachment_relation_heads
            FOR EACH ROW
            WHEN NEW.relation_revision <= OLD.relation_revision
            BEGIN
                SELECT RAISE(ABORT, 'forum attachment relation revision must advance monotonically');
            END"#,
        r#"CREATE TRIGGER forum_attachment_relation_head_identity_guard
            BEFORE UPDATE ON forum_attachment_relation_heads
            FOR EACH ROW
            WHEN NEW.tenant_id IS NOT OLD.tenant_id
              OR NEW.target_kind IS NOT OLD.target_kind
              OR NEW.target_id IS NOT OLD.target_id
              OR NEW.locale IS NOT OLD.locale
            BEGIN
                SELECT RAISE(ABORT, 'forum attachment relation head identity is immutable');
            END"#,
        r#"CREATE TRIGGER forum_attachment_relation_immutable_update_guard
            BEFORE UPDATE ON forum_attachment_relations
            FOR EACH ROW
            BEGIN
                SELECT RAISE(ABORT, 'forum attachment relation rows are immutable; replace the relation set');
            END"#,
        r#"CREATE TRIGGER forum_attachment_relation_head_update_target_guard
            BEFORE UPDATE ON forum_attachment_relation_heads
            FOR EACH ROW
            WHEN (NEW.target_kind = 'topic' AND NOT EXISTS (
                SELECT 1 FROM forum_topics topic
                WHERE topic.tenant_id = NEW.tenant_id AND topic.id = NEW.target_id
            ))
            OR (NEW.target_kind = 'reply' AND NOT EXISTS (
                SELECT 1 FROM forum_replies reply
                WHERE reply.tenant_id = NEW.tenant_id AND reply.id = NEW.target_id
            ))
            OR NEW.target_kind NOT IN ('topic', 'reply')
            BEGIN
                SELECT RAISE(ABORT, 'forum attachment relation target does not exist');
            END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_attachment_relation_head_update_target_guard",
        "DROP TRIGGER IF EXISTS forum_attachment_relation_head_revision_guard",
        "DROP TRIGGER IF EXISTS forum_attachment_relation_head_identity_guard",
        "DROP TRIGGER IF EXISTS forum_attachment_relation_immutable_update_guard",
        "DROP TRIGGER IF EXISTS forum_attachment_relation_head_insert_guard",
        "DROP INDEX IF EXISTS idx_forum_attachment_relations_media",
        "DROP INDEX IF EXISTS uq_forum_attachment_relations_target_position",
        "DROP TABLE IF EXISTS forum_attachment_relations",
        "DROP TABLE IF EXISTS forum_attachment_relation_heads",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
