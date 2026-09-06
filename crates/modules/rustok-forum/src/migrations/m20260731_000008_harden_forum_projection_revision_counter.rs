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
                "Forum projection revision counter hardening does not support database backend {backend:?}"
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
                "Forum projection revision counter hardening does not support database backend {backend:?}"
            ))),
        }
    }
}

const POSTGRES_UP: &str = r#"
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forum_projection_revision_counters AS counters
        LEFT JOIN forum_projection_revision_ledger AS ledger
          ON ledger.tenant_id = counters.tenant_id
        GROUP BY counters.tenant_id, counters.revision
        HAVING COUNT(ledger.revision) <> counters.revision
            OR MIN(ledger.revision) IS DISTINCT FROM 1
            OR MAX(ledger.revision) IS DISTINCT FROM counters.revision
    ) OR EXISTS (
        SELECT 1
        FROM forum_projection_revision_ledger AS ledger
        LEFT JOIN forum_projection_revision_counters AS counters
          ON counters.tenant_id = ledger.tenant_id
        WHERE counters.tenant_id IS NULL
    ) THEN
        RAISE EXCEPTION 'existing forum projection revision storage is inconsistent';
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION forum_enforce_projection_revision_counter()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.revision <> 1 THEN
            RAISE EXCEPTION 'forum projection revision counter must start at 1';
        END IF;
        RETURN NEW;
    END IF;

    IF TG_OP = 'UPDATE' THEN
        IF NEW.tenant_id <> OLD.tenant_id OR NEW.revision <> OLD.revision + 1 THEN
            RAISE EXCEPTION 'forum projection revision counter must advance by exactly 1';
        END IF;
        RETURN NEW;
    END IF;

    RAISE EXCEPTION 'forum projection revision counter cannot be deleted';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_projection_revision_counter_insert
    ON forum_projection_revision_counters;
CREATE TRIGGER forum_projection_revision_counter_insert
BEFORE INSERT ON forum_projection_revision_counters
FOR EACH ROW EXECUTE FUNCTION forum_enforce_projection_revision_counter();

DROP TRIGGER IF EXISTS forum_projection_revision_counter_update
    ON forum_projection_revision_counters;
CREATE TRIGGER forum_projection_revision_counter_update
BEFORE UPDATE ON forum_projection_revision_counters
FOR EACH ROW EXECUTE FUNCTION forum_enforce_projection_revision_counter();

DROP TRIGGER IF EXISTS forum_projection_revision_counter_delete
    ON forum_projection_revision_counters;
CREATE TRIGGER forum_projection_revision_counter_delete
BEFORE DELETE ON forum_projection_revision_counters
FOR EACH ROW EXECUTE FUNCTION forum_enforce_projection_revision_counter();

CREATE OR REPLACE FUNCTION forum_require_projection_revision_ledger_row()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM forum_projection_revision_ledger
        WHERE tenant_id = NEW.tenant_id
          AND revision = NEW.revision
    ) THEN
        RAISE EXCEPTION 'forum projection revision counter requires a matching ledger row';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_projection_revision_counter_ledger_commit
    ON forum_projection_revision_counters;
CREATE CONSTRAINT TRIGGER forum_projection_revision_counter_ledger_commit
AFTER INSERT OR UPDATE ON forum_projection_revision_counters
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION forum_require_projection_revision_ledger_row();

CREATE OR REPLACE FUNCTION forum_reject_projection_revision_truncate()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum projection revision storage cannot be truncated';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_projection_revision_counter_truncate
    ON forum_projection_revision_counters;
CREATE TRIGGER forum_projection_revision_counter_truncate
BEFORE TRUNCATE ON forum_projection_revision_counters
FOR EACH STATEMENT EXECUTE FUNCTION forum_reject_projection_revision_truncate();

DROP TRIGGER IF EXISTS forum_projection_revision_ledger_truncate
    ON forum_projection_revision_ledger;
CREATE TRIGGER forum_projection_revision_ledger_truncate
BEFORE TRUNCATE ON forum_projection_revision_ledger
FOR EACH STATEMENT EXECUTE FUNCTION forum_reject_projection_revision_truncate();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_projection_revision_ledger_truncate
    ON forum_projection_revision_ledger;
DROP TRIGGER IF EXISTS forum_projection_revision_counter_truncate
    ON forum_projection_revision_counters;
DROP TRIGGER IF EXISTS forum_projection_revision_counter_ledger_commit
    ON forum_projection_revision_counters;
DROP TRIGGER IF EXISTS forum_projection_revision_counter_delete
    ON forum_projection_revision_counters;
DROP TRIGGER IF EXISTS forum_projection_revision_counter_update
    ON forum_projection_revision_counters;
DROP TRIGGER IF EXISTS forum_projection_revision_counter_insert
    ON forum_projection_revision_counters;
DROP FUNCTION IF EXISTS forum_reject_projection_revision_truncate();
DROP FUNCTION IF EXISTS forum_require_projection_revision_ledger_row();
DROP FUNCTION IF EXISTS forum_enforce_projection_revision_counter();
"#;
