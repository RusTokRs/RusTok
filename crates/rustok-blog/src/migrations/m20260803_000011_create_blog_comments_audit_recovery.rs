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
            DbBackend::MySql => Ok(()),
            _ => unreachable!("unsupported SeaORM database backend"),
}
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "Comments schedule audit recovery epochs and immutable audit facts are security-sensitive and intentionally irreversible"
                .to_string(),
        ))
    }
}

async fn postgres_up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE blog_comments_tcp_delegation_schedule_audit_outbox
    ADD COLUMN handoff_recovery_epoch BIGINT NOT NULL DEFAULT 0,
    ADD CONSTRAINT ck_blog_comments_delegation_audit_handoff_recovery_epoch
        CHECK (handoff_recovery_epoch >= 0);

CREATE TABLE blog_comments_tcp_delegation_schedule_audit_recovery_audits (
    audit_id UUID NOT NULL,
    control_plane_tenant_id UUID NOT NULL,
    request_id UUID NOT NULL,
    actor_id UUID NOT NULL,
    action VARCHAR(32) NOT NULL,
    reason VARCHAR(512) NOT NULL,
    prior_attempt_count BIGINT NOT NULL,
    recovery_epoch BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_blog_comments_delegation_audit_recovery_audits
        PRIMARY KEY (audit_id),
    CONSTRAINT uq_blog_comments_delegation_audit_recovery_epoch
        UNIQUE (request_id, recovery_epoch),
    CONSTRAINT ck_blog_comments_delegation_audit_recovery_audit_id
        CHECK (audit_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT ck_blog_comments_delegation_audit_recovery_tenant_id
        CHECK (control_plane_tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT ck_blog_comments_delegation_audit_recovery_request_id
        CHECK (request_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT ck_blog_comments_delegation_audit_recovery_actor_id
        CHECK (actor_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT ck_blog_comments_delegation_audit_recovery_action
        CHECK (action = 'requeue'),
    CONSTRAINT ck_blog_comments_delegation_audit_recovery_reason
        CHECK (
            octet_length(reason) BETWEEN 1 AND 512
            AND reason = btrim(reason)
            AND reason !~ '[[:cntrl:]]'
        ),
    CONSTRAINT ck_blog_comments_delegation_audit_recovery_prior_attempt
        CHECK (prior_attempt_count > 0),
    CONSTRAINT ck_blog_comments_delegation_audit_recovery_epoch_positive
        CHECK (recovery_epoch > 0)
);

CREATE INDEX idx_blog_comments_delegation_audit_recovery_request
    ON blog_comments_tcp_delegation_schedule_audit_recovery_audits
        (control_plane_tenant_id, request_id, created_at);

CREATE OR REPLACE FUNCTION blog_comments_delegation_audit_recovery_reject_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'Comments schedule audit recovery facts are append-only';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER blog_comments_delegation_audit_recovery_immutable_update
BEFORE UPDATE ON blog_comments_tcp_delegation_schedule_audit_recovery_audits
FOR EACH ROW EXECUTE FUNCTION blog_comments_delegation_audit_recovery_reject_mutation();

CREATE TRIGGER blog_comments_delegation_audit_recovery_immutable_delete
BEFORE DELETE ON blog_comments_tcp_delegation_schedule_audit_recovery_audits
FOR EACH ROW EXECUTE FUNCTION blog_comments_delegation_audit_recovery_reject_mutation();
"#,
        )
        .await?;
    Ok(())
}

async fn sqlite_up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        r#"ALTER TABLE blog_comments_tcp_delegation_schedule_audit_outbox
ADD COLUMN handoff_recovery_epoch INTEGER NOT NULL DEFAULT 0
CHECK (handoff_recovery_epoch >= 0)"#,
        r#"CREATE TABLE blog_comments_tcp_delegation_schedule_audit_recovery_audits (
    audit_id TEXT NOT NULL PRIMARY KEY,
    control_plane_tenant_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action = 'requeue'),
    reason TEXT NOT NULL CHECK (
        length(reason) BETWEEN 1 AND 512
        AND reason = trim(reason)
    ),
    prior_attempt_count INTEGER NOT NULL CHECK (prior_attempt_count > 0),
    recovery_epoch INTEGER NOT NULL CHECK (recovery_epoch > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (request_id, recovery_epoch)
)"#,
        r#"CREATE INDEX idx_blog_comments_delegation_audit_recovery_request
ON blog_comments_tcp_delegation_schedule_audit_recovery_audits
    (control_plane_tenant_id, request_id, created_at)"#,
        r#"CREATE TRIGGER blog_comments_delegation_audit_recovery_immutable_update
BEFORE UPDATE ON blog_comments_tcp_delegation_schedule_audit_recovery_audits
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'Comments schedule audit recovery facts are append-only');
END"#,
        r#"CREATE TRIGGER blog_comments_delegation_audit_recovery_immutable_delete
BEFORE DELETE ON blog_comments_tcp_delegation_schedule_audit_recovery_audits
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'Comments schedule audit recovery facts are append-only');
END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
