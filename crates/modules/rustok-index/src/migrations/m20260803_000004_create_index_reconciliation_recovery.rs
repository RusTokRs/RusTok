use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DbBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DbBackend::Postgres => postgres_up(manager).await,
            DbBackend::Sqlite => sqlite_up(manager).await,
            _ => Err(DbErr::Migration(
                "Index job recovery supports PostgreSQL and SQLite only".to_string(),
            )),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DbBackend::Postgres => postgres_down(manager).await,
            DbBackend::Sqlite => sqlite_down(manager).await,
            _ => Ok(()),
        }
    }
}

async fn postgres_up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE index_jobs
    ADD COLUMN retry_epoch INTEGER NOT NULL DEFAULT 0
    CONSTRAINT chk_index_jobs_retry_epoch CHECK (retry_epoch >= 0),
    ADD COLUMN failure_count INTEGER NOT NULL DEFAULT 0
    CONSTRAINT chk_index_jobs_failure_count CHECK (failure_count >= 0);

CREATE TABLE index_job_recovery_audits (
    tenant_id UUID NOT NULL,
    audit_id UUID NOT NULL,
    job_id UUID NOT NULL,
    job_kind VARCHAR(16) NOT NULL CHECK (job_kind IN ('rebuild', 'reconcile')),
    actor_id UUID NOT NULL,
    action VARCHAR(32) NOT NULL,
    reason VARCHAR(512) NOT NULL,
    prior_attempt_count INTEGER NOT NULL,
    retry_epoch INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_index_job_recovery_audits
        PRIMARY KEY (tenant_id, audit_id),
    CONSTRAINT uq_index_job_recovery_job_epoch
        UNIQUE (tenant_id, job_id, retry_epoch),
    CONSTRAINT chk_index_job_recovery_action
        CHECK (action = 'requeue'),
    CONSTRAINT chk_index_job_recovery_reason
        CHECK (length(reason) BETWEEN 1 AND 512 AND reason = btrim(reason)),
    CONSTRAINT chk_index_job_recovery_prior_attempt
        CHECK (prior_attempt_count > 0),
    CONSTRAINT chk_index_job_recovery_epoch
        CHECK (retry_epoch > 0)
);

CREATE INDEX idx_index_job_recovery_job
    ON index_job_recovery_audits (tenant_id, job_id, created_at);

CREATE OR REPLACE FUNCTION index_job_recovery_reject_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'Index job recovery audits are append-only';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER index_job_recovery_audits_immutable_update
BEFORE UPDATE ON index_job_recovery_audits
FOR EACH ROW EXECUTE FUNCTION index_job_recovery_reject_mutation();

CREATE TRIGGER index_job_recovery_audits_immutable_delete
BEFORE DELETE ON index_job_recovery_audits
FOR EACH ROW EXECUTE FUNCTION index_job_recovery_reject_mutation();

CREATE TRIGGER index_job_recovery_audits_immutable_truncate
BEFORE TRUNCATE ON index_job_recovery_audits
FOR EACH STATEMENT EXECUTE FUNCTION index_job_recovery_reject_mutation();
"#,
        )
        .await?;
    Ok(())
}

async fn sqlite_up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let transaction = manager.begin().await?;
    let connection = transaction.get_connection();
    for statement in [
        r#"ALTER TABLE index_jobs
ADD COLUMN retry_epoch INTEGER NOT NULL DEFAULT 0
CHECK (retry_epoch >= 0)"#,
        "ALTER TABLE index_jobs ADD COLUMN failure_count INTEGER NOT NULL DEFAULT 0 CHECK (failure_count >= 0)",
        r#"CREATE TABLE index_job_recovery_audits (
    tenant_id TEXT NOT NULL,
    audit_id TEXT NOT NULL,
    job_id TEXT NOT NULL,
    job_kind TEXT NOT NULL CHECK (job_kind IN ('rebuild', 'reconcile')),
    actor_id TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action = 'requeue'),
    reason TEXT NOT NULL CHECK (
        length(reason) BETWEEN 1 AND 512
        AND reason = trim(reason)
    ),
    prior_attempt_count INTEGER NOT NULL CHECK (prior_attempt_count > 0),
    retry_epoch INTEGER NOT NULL CHECK (retry_epoch > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, audit_id),
    UNIQUE (tenant_id, job_id, retry_epoch)
)"#,
        r#"CREATE INDEX idx_index_job_recovery_job
ON index_job_recovery_audits (tenant_id, job_id, created_at)"#,
        // SQLite REPLACE can delete a conflicting row without firing DELETE triggers when
        // recursive_triggers is off. Reject reuse of either unique identity before that happens.
        r#"CREATE TRIGGER index_job_recovery_audits_no_replacement
BEFORE INSERT ON index_job_recovery_audits
FOR EACH ROW WHEN EXISTS (
    SELECT 1 FROM index_job_recovery_audits
    WHERE tenant_id = NEW.tenant_id
      AND (audit_id = NEW.audit_id OR (job_id = NEW.job_id AND retry_epoch = NEW.retry_epoch))
)
BEGIN
    SELECT RAISE(ABORT, 'Index job recovery audits are append-only');
END"#,
        r#"CREATE TRIGGER index_job_recovery_audits_immutable_update
BEFORE UPDATE ON index_job_recovery_audits
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'Index job recovery audits are append-only');
END"#,
        r#"CREATE TRIGGER index_job_recovery_audits_immutable_delete
BEFORE DELETE ON index_job_recovery_audits
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'Index job recovery audits are append-only');
END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    transaction.commit().await
}

async fn postgres_down(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TRIGGER IF EXISTS index_job_recovery_audits_immutable_update
    ON index_job_recovery_audits;
DROP TRIGGER IF EXISTS index_job_recovery_audits_immutable_delete
    ON index_job_recovery_audits;
DROP TRIGGER IF EXISTS index_job_recovery_audits_immutable_truncate
    ON index_job_recovery_audits;
DROP FUNCTION IF EXISTS index_job_recovery_reject_mutation();
DROP TABLE IF EXISTS index_job_recovery_audits;
ALTER TABLE index_jobs DROP COLUMN failure_count, DROP COLUMN retry_epoch;
"#,
        )
        .await?;
    Ok(())
}

async fn sqlite_down(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let transaction = manager.begin().await?;
    let connection = transaction.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS index_job_recovery_audits_no_replacement",
        "DROP TRIGGER IF EXISTS index_job_recovery_audits_immutable_update",
        "DROP TRIGGER IF EXISTS index_job_recovery_audits_immutable_delete",
        "DROP TABLE IF EXISTS index_job_recovery_audits",
        "ALTER TABLE index_jobs DROP COLUMN failure_count",
        "ALTER TABLE index_jobs DROP COLUMN retry_epoch",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    transaction.commit().await
}
