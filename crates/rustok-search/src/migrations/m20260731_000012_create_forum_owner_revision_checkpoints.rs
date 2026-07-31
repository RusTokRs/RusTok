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
                "Forum owner revision checkpoints do not support database backend {backend:?}"
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
                "Forum owner revision checkpoints do not support database backend {backend:?}"
            ))),
        }
    }
}

const POSTGRES_UP: &str = r#"
CREATE TABLE IF NOT EXISTS search_projection_owner_checkpoints (
    tenant_id UUID NOT NULL,
    source_module VARCHAR(64) NOT NULL,
    owner_revision BIGINT NOT NULL,
    event_id UUID NOT NULL,
    outcome VARCHAR(32) NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, source_module),
    CONSTRAINT ck_search_projection_owner_checkpoint_identity CHECK (
        source_module = 'forum'
    ),
    CONSTRAINT ck_search_projection_owner_checkpoint_revision CHECK (
        owner_revision > 0
    ),
    CONSTRAINT ck_search_projection_owner_checkpoint_outcome CHECK (
        outcome IN ('delivery_covered', 'rebuild_repaired')
    )
);

CREATE TABLE IF NOT EXISTS search_projection_owner_scan_cursors (
    source_module VARCHAR(64) PRIMARY KEY,
    after_tenant_id UUID NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT ck_search_projection_owner_scan_source CHECK (
        source_module = 'forum'
    )
);

CREATE OR REPLACE FUNCTION search_enforce_projection_owner_checkpoint()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.owner_revision <> 1
            AND NOT EXISTS (
                SELECT 1
                FROM search_projection_owner_checkpoints
                WHERE tenant_id = NEW.tenant_id
                  AND source_module = NEW.source_module
            )
        THEN
            RAISE EXCEPTION 'search projection owner checkpoint must start at revision 1';
        END IF;
        RETURN NEW;
    END IF;

    IF TG_OP = 'UPDATE' THEN
        IF NEW.tenant_id <> OLD.tenant_id
            OR NEW.source_module <> OLD.source_module
            OR NEW.owner_revision <> OLD.owner_revision + 1
        THEN
            RAISE EXCEPTION 'search projection owner checkpoint must advance by exactly 1';
        END IF;
        RETURN NEW;
    END IF;

    RAISE EXCEPTION 'search projection owner checkpoint cannot be deleted';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS search_projection_owner_checkpoint_insert
    ON search_projection_owner_checkpoints;
CREATE TRIGGER search_projection_owner_checkpoint_insert
BEFORE INSERT ON search_projection_owner_checkpoints
FOR EACH ROW EXECUTE FUNCTION search_enforce_projection_owner_checkpoint();

DROP TRIGGER IF EXISTS search_projection_owner_checkpoint_update
    ON search_projection_owner_checkpoints;
CREATE TRIGGER search_projection_owner_checkpoint_update
BEFORE UPDATE ON search_projection_owner_checkpoints
FOR EACH ROW EXECUTE FUNCTION search_enforce_projection_owner_checkpoint();

DROP TRIGGER IF EXISTS search_projection_owner_checkpoint_delete
    ON search_projection_owner_checkpoints;
CREATE TRIGGER search_projection_owner_checkpoint_delete
BEFORE DELETE ON search_projection_owner_checkpoints
FOR EACH ROW EXECUTE FUNCTION search_enforce_projection_owner_checkpoint();

CREATE OR REPLACE FUNCTION search_reject_projection_owner_checkpoint_truncate()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'search projection owner checkpoint storage cannot be truncated';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS search_projection_owner_checkpoint_truncate
    ON search_projection_owner_checkpoints;
CREATE TRIGGER search_projection_owner_checkpoint_truncate
BEFORE TRUNCATE ON search_projection_owner_checkpoints
FOR EACH STATEMENT EXECUTE FUNCTION search_reject_projection_owner_checkpoint_truncate();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS search_projection_owner_checkpoint_truncate
    ON search_projection_owner_checkpoints;
DROP TRIGGER IF EXISTS search_projection_owner_checkpoint_delete
    ON search_projection_owner_checkpoints;
DROP TRIGGER IF EXISTS search_projection_owner_checkpoint_update
    ON search_projection_owner_checkpoints;
DROP TRIGGER IF EXISTS search_projection_owner_checkpoint_insert
    ON search_projection_owner_checkpoints;
DROP FUNCTION IF EXISTS search_reject_projection_owner_checkpoint_truncate();
DROP FUNCTION IF EXISTS search_enforce_projection_owner_checkpoint();
DROP TABLE IF EXISTS search_projection_owner_scan_cursors;
DROP TABLE IF EXISTS search_projection_owner_checkpoints;
"#;
