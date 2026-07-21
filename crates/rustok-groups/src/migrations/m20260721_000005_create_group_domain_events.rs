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
            DbBackend::MySql => Err(DbErr::Migration(
                "Groups targeted invitation events support PostgreSQL and SQLite only".to_string(),
            )),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DbBackend::Postgres => postgres_down(manager).await,
            DbBackend::Sqlite => sqlite_down(manager).await,
            DbBackend::MySql => Ok(()),
        }
    }
}

async fn postgres_up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE OR REPLACE FUNCTION groups_generate_event_uuid()
RETURNS uuid AS $$
DECLARE
    source text;
BEGIN
    source := md5(random()::text || clock_timestamp()::text || txid_current()::text);
    RETURN (
        substr(source, 1, 8) || '-' ||
        substr(source, 9, 4) || '-' ||
        substr(source, 13, 4) || '-' ||
        substr(source, 17, 4) || '-' ||
        substr(source, 21, 12)
    )::uuid;
END;
$$ LANGUAGE plpgsql VOLATILE;

CREATE TABLE IF NOT EXISTS group_domain_events (
    sequence_no BIGSERIAL PRIMARY KEY,
    event_id UUID NOT NULL DEFAULT groups_generate_event_uuid(),
    tenant_id UUID NOT NULL,
    aggregate_type VARCHAR(32) NOT NULL,
    aggregate_id UUID NOT NULL,
    event_type VARCHAR(96) NOT NULL,
    schema_version SMALLINT NOT NULL DEFAULT 1,
    actor_id UUID,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_group_domain_events_event_id UNIQUE (event_id),
    CONSTRAINT chk_group_domain_events_aggregate_type CHECK (aggregate_type = 'invitation'),
    CONSTRAINT chk_group_domain_events_event_type CHECK (event_type = 'groups.invitation.targeted_created'),
    CONSTRAINT chk_group_domain_events_schema_version CHECK (schema_version = 1)
);

CREATE INDEX IF NOT EXISTS idx_group_domain_events_tenant_sequence
    ON group_domain_events (tenant_id, sequence_no);
CREATE INDEX IF NOT EXISTS idx_group_domain_events_tenant_aggregate
    ON group_domain_events (tenant_id, aggregate_type, aggregate_id, sequence_no);
CREATE INDEX IF NOT EXISTS idx_group_domain_events_tenant_type
    ON group_domain_events (tenant_id, event_type, sequence_no);

CREATE OR REPLACE FUNCTION groups_reject_domain_event_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'group domain events are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS group_domain_events_immutable_update ON group_domain_events;
CREATE TRIGGER group_domain_events_immutable_update
BEFORE UPDATE ON group_domain_events
FOR EACH ROW EXECUTE FUNCTION groups_reject_domain_event_mutation();

DROP TRIGGER IF EXISTS group_domain_events_immutable_delete ON group_domain_events;
CREATE TRIGGER group_domain_events_immutable_delete
BEFORE DELETE ON group_domain_events
FOR EACH ROW EXECUTE FUNCTION groups_reject_domain_event_mutation();

CREATE OR REPLACE FUNCTION groups_append_targeted_invitation_event()
RETURNS trigger AS $$
BEGIN
    IF NEW.target_user_id IS NOT NULL THEN
        INSERT INTO group_domain_events (
            tenant_id,
            aggregate_type,
            aggregate_id,
            event_type,
            schema_version,
            actor_id,
            payload
        ) VALUES (
            NEW.tenant_id,
            'invitation',
            NEW.id,
            'groups.invitation.targeted_created',
            1,
            NEW.invited_by_user_id,
            jsonb_build_object(
                'invitation_id', NEW.id,
                'group_id', NEW.group_id,
                'target_user_id', NEW.target_user_id
            )
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS groups_targeted_invitation_created_event ON group_invitations;
CREATE TRIGGER groups_targeted_invitation_created_event
AFTER INSERT ON group_invitations
FOR EACH ROW EXECUTE FUNCTION groups_append_targeted_invitation_event();
"#,
        )
        .await?;
    Ok(())
}

async fn sqlite_up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        r#"CREATE TABLE IF NOT EXISTS group_domain_events (
    sequence_no INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    tenant_id TEXT NOT NULL,
    aggregate_type TEXT NOT NULL CHECK (aggregate_type = 'invitation'),
    aggregate_id TEXT NOT NULL,
    event_type TEXT NOT NULL CHECK (event_type = 'groups.invitation.targeted_created'),
    schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version = 1),
    actor_id TEXT,
    payload TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
)"#,
        r#"CREATE INDEX IF NOT EXISTS idx_group_domain_events_tenant_sequence
    ON group_domain_events (tenant_id, sequence_no)"#,
        r#"CREATE INDEX IF NOT EXISTS idx_group_domain_events_tenant_aggregate
    ON group_domain_events (tenant_id, aggregate_type, aggregate_id, sequence_no)"#,
        r#"CREATE INDEX IF NOT EXISTS idx_group_domain_events_tenant_type
    ON group_domain_events (tenant_id, event_type, sequence_no)"#,
        r#"DROP TRIGGER IF EXISTS group_domain_events_immutable_update"#,
        r#"DROP TRIGGER IF EXISTS group_domain_events_immutable_delete"#,
        r#"CREATE TRIGGER group_domain_events_immutable_update
BEFORE UPDATE ON group_domain_events
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'group domain events are append-only');
END"#,
        r#"CREATE TRIGGER group_domain_events_immutable_delete
BEFORE DELETE ON group_domain_events
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'group domain events are append-only');
END"#,
        r#"DROP TRIGGER IF EXISTS groups_targeted_invitation_created_event"#,
        r#"CREATE TRIGGER groups_targeted_invitation_created_event
AFTER INSERT ON group_invitations
FOR EACH ROW
WHEN NEW.target_user_id IS NOT NULL
BEGIN
    INSERT INTO group_domain_events (
        event_id,
        tenant_id,
        aggregate_type,
        aggregate_id,
        event_type,
        schema_version,
        actor_id,
        payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
        lower(hex(randomblob(2))) || '-' ||
        lower(hex(randomblob(2))) || '-' ||
        lower(hex(randomblob(2))) || '-' ||
        lower(hex(randomblob(6))),
        NEW.tenant_id,
        'invitation',
        NEW.id,
        'groups.invitation.targeted_created',
        1,
        NEW.invited_by_user_id,
        json_object(
            'invitation_id', NEW.id,
            'group_id', NEW.group_id,
            'target_user_id', NEW.target_user_id
        )
    );
END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn postgres_down(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TRIGGER IF EXISTS groups_targeted_invitation_created_event ON group_invitations;
DROP FUNCTION IF EXISTS groups_append_targeted_invitation_event();
DROP TRIGGER IF EXISTS group_domain_events_immutable_update ON group_domain_events;
DROP TRIGGER IF EXISTS group_domain_events_immutable_delete ON group_domain_events;
DROP FUNCTION IF EXISTS groups_reject_domain_event_mutation();
DROP TABLE IF EXISTS group_domain_events;
DROP FUNCTION IF EXISTS groups_generate_event_uuid();
"#,
        )
        .await?;
    Ok(())
}

async fn sqlite_down(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS groups_targeted_invitation_created_event",
        "DROP TRIGGER IF EXISTS group_domain_events_immutable_update",
        "DROP TRIGGER IF EXISTS group_domain_events_immutable_delete",
        "DROP TABLE IF EXISTS group_domain_events",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
