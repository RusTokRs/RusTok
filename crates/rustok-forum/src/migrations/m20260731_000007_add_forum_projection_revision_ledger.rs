use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => manager
                .get_connection()
                .execute_unprepared(POSTGRES_UP)
                .await
                .map(|_| ()),
            DatabaseBackend::Sqlite => Ok(()),
            backend => Err(DbErr::Custom(format!(
                "Forum projection revision ledger does not support database backend {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => manager
                .get_connection()
                .execute_unprepared(POSTGRES_DOWN)
                .await
                .map(|_| ()),
            DatabaseBackend::Sqlite => Ok(()),
            backend => Err(DbErr::Custom(format!(
                "Forum projection revision ledger does not support database backend {backend:?}"
            ))),
        }
    }
}

const POSTGRES_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_projection_revision_counters (
    tenant_id UUID PRIMARY KEY,
    revision BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT ck_forum_projection_revision_counter_positive
        CHECK (revision > 0)
);

CREATE TABLE IF NOT EXISTS forum_projection_revision_ledger (
    tenant_id UUID NOT NULL,
    revision BIGINT NOT NULL,
    event_id UUID NOT NULL,
    target_type VARCHAR(64) NOT NULL,
    target_id UUID NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_forum_projection_revision_ledger
        PRIMARY KEY (tenant_id, revision),
    CONSTRAINT uq_forum_projection_revision_event UNIQUE (event_id),
    CONSTRAINT ck_forum_projection_revision_positive CHECK (revision > 0),
    CONSTRAINT ck_forum_projection_revision_target CHECK (
        (target_type = 'forum' AND target_id IS NULL)
        OR (target_type IN ('forum_category', 'forum_topic') AND target_id IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_projection_revision_ledger_target
    ON forum_projection_revision_ledger (tenant_id, target_type, target_id, revision DESC);

CREATE OR REPLACE FUNCTION forum_reject_projection_revision_ledger_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum projection revision ledger is append-only';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_projection_revision_ledger_update
    ON forum_projection_revision_ledger;
CREATE TRIGGER forum_projection_revision_ledger_update
BEFORE UPDATE ON forum_projection_revision_ledger
FOR EACH ROW EXECUTE FUNCTION forum_reject_projection_revision_ledger_mutation();

DROP TRIGGER IF EXISTS forum_projection_revision_ledger_delete
    ON forum_projection_revision_ledger;
CREATE TRIGGER forum_projection_revision_ledger_delete
BEFORE DELETE ON forum_projection_revision_ledger
FOR EACH ROW EXECUTE FUNCTION forum_reject_projection_revision_ledger_mutation();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_projection_revision_ledger_delete
    ON forum_projection_revision_ledger;
DROP TRIGGER IF EXISTS forum_projection_revision_ledger_update
    ON forum_projection_revision_ledger;
DROP TABLE IF EXISTS forum_projection_revision_ledger;
DROP TABLE IF EXISTS forum_projection_revision_counters;
DROP FUNCTION IF EXISTS forum_reject_projection_revision_ledger_mutation();
"#;
