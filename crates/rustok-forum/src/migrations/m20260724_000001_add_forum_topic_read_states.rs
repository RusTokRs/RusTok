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
                "rustok-forum topic read state migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum topic read state migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE TABLE IF NOT EXISTS forum_topic_read_states (
    tenant_id uuid NOT NULL,
    topic_id uuid NOT NULL,
    user_id uuid NOT NULL,
    last_read_position bigint NOT NULL DEFAULT 0,
    last_read_revision bigint NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT forum_topic_read_states_pkey
        PRIMARY KEY (tenant_id, topic_id, user_id),
    CONSTRAINT chk_forum_topic_read_states_position_nonnegative
        CHECK (last_read_position >= 0),
    CONSTRAINT chk_forum_topic_read_states_revision_nonnegative
        CHECK (last_read_revision >= 0),
    CONSTRAINT fk_forum_topic_read_states_topic_tenant
        FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_read_states_tenant_user_updated
    ON forum_topic_read_states (tenant_id, user_id, updated_at DESC, topic_id);

CREATE OR REPLACE FUNCTION forum_prevent_topic_read_state_regression()
RETURNS trigger AS $$
BEGIN
    IF NEW.last_read_position < OLD.last_read_position THEN
        RAISE EXCEPTION 'forum topic read position cannot move backwards';
    END IF;
    IF NEW.last_read_revision < OLD.last_read_revision THEN
        RAISE EXCEPTION 'forum topic read revision cannot move backwards';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_topic_read_states_monotonic_update
    ON forum_topic_read_states;
CREATE TRIGGER forum_topic_read_states_monotonic_update
BEFORE UPDATE OF last_read_position, last_read_revision
ON forum_topic_read_states
FOR EACH ROW
EXECUTE FUNCTION forum_prevent_topic_read_state_regression();
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
DROP TRIGGER IF EXISTS forum_topic_read_states_monotonic_update
    ON forum_topic_read_states;
DROP FUNCTION IF EXISTS forum_prevent_topic_read_state_regression();
DROP TABLE IF EXISTS forum_topic_read_states;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE TABLE IF NOT EXISTS forum_topic_read_states (
    tenant_id TEXT NOT NULL,
    topic_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    last_read_position INTEGER NOT NULL DEFAULT 0
        CHECK (last_read_position >= 0),
    last_read_revision INTEGER NOT NULL DEFAULT 0
        CHECK (last_read_revision >= 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, topic_id, user_id),
    CONSTRAINT fk_forum_topic_read_states_topic_tenant
        FOREIGN KEY (tenant_id, topic_id)
        REFERENCES forum_topics (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_forum_topic_read_states_tenant_user_updated
    ON forum_topic_read_states (tenant_id, user_id, updated_at DESC, topic_id);

DROP TRIGGER IF EXISTS forum_topic_read_states_monotonic_update;
CREATE TRIGGER forum_topic_read_states_monotonic_update
BEFORE UPDATE OF last_read_position, last_read_revision
ON forum_topic_read_states
FOR EACH ROW
WHEN NEW.last_read_position < OLD.last_read_position
  OR NEW.last_read_revision < OLD.last_read_revision
BEGIN
    SELECT RAISE(ABORT, 'forum topic read state cannot move backwards');
END;
"#,
        )
        .await?;
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TRIGGER IF EXISTS forum_topic_read_states_monotonic_update;
DROP TABLE IF EXISTS forum_topic_read_states;
"#,
        )
        .await?;
    Ok(())
}
