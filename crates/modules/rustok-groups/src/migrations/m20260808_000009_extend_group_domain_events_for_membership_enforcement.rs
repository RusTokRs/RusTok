use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => postgres_up(manager).await,
            DatabaseBackend::Sqlite => sqlite_up(manager).await,
            backend => Err(DbErr::Migration(format!(
                "Groups membership enforcement events do not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        ensure_no_membership_events(manager).await?;
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => postgres_down(manager).await,
            DatabaseBackend::Sqlite => sqlite_down(manager).await,
            backend => Err(DbErr::Migration(format!(
                "Groups membership enforcement events do not support {backend:?}"
            ))),
        }
    }
}

async fn ensure_no_membership_events(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();
    let row = manager
        .get_connection()
        .query_one_raw(Statement::from_string(
            backend,
            "SELECT COUNT(*) AS event_count FROM group_domain_events WHERE aggregate_type = 'membership'"
                .to_string(),
        ))
        .await?
        .ok_or_else(|| DbErr::Migration("Groups domain-event count query returned no row".into()))?;
    let count: i64 = row.try_get("", "event_count")?;
    if count != 0 {
        return Err(DbErr::Migration(
            "cannot downgrade Groups membership enforcement events while append-only membership events exist"
                .to_string(),
        ));
    }
    Ok(())
}

async fn postgres_up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE group_domain_events
    DROP CONSTRAINT IF EXISTS chk_group_domain_events_aggregate_type,
    DROP CONSTRAINT IF EXISTS chk_group_domain_events_event_type,
    DROP CONSTRAINT IF EXISTS chk_group_domain_events_kind;

ALTER TABLE group_domain_events
    ADD CONSTRAINT chk_group_domain_events_kind CHECK (
        (aggregate_type = 'invitation' AND event_type = 'groups.invitation.targeted_created')
        OR
        (aggregate_type = 'membership' AND event_type IN (
            'groups.membership.suspended',
            'groups.membership.suspension_revoked'
        ))
    );
"#,
        )
        .await?;
    Ok(())
}

async fn postgres_down(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE group_domain_events
    DROP CONSTRAINT IF EXISTS chk_group_domain_events_kind;

ALTER TABLE group_domain_events
    ADD CONSTRAINT chk_group_domain_events_aggregate_type CHECK (aggregate_type = 'invitation'),
    ADD CONSTRAINT chk_group_domain_events_event_type CHECK (event_type = 'groups.invitation.targeted_created');
"#,
        )
        .await?;
    Ok(())
}

async fn sqlite_up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    rebuild_sqlite_events(manager, true).await
}

async fn sqlite_down(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    rebuild_sqlite_events(manager, false).await
}

async fn rebuild_sqlite_events(
    manager: &SchemaManager<'_>,
    allow_membership_events: bool,
) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS groups_targeted_invitation_created_event",
        "DROP TRIGGER IF EXISTS group_domain_events_immutable_update",
        "DROP TRIGGER IF EXISTS group_domain_events_immutable_delete",
        "DROP TABLE IF EXISTS group_domain_events_next",
    ] {
        connection.execute_unprepared(statement).await?;
    }

    let kind_check = if allow_membership_events {
        "CHECK ((aggregate_type = 'invitation' AND event_type = 'groups.invitation.targeted_created') OR (aggregate_type = 'membership' AND event_type IN ('groups.membership.suspended', 'groups.membership.suspension_revoked')))"
    } else {
        "CHECK (aggregate_type = 'invitation' AND event_type = 'groups.invitation.targeted_created')"
    };

    connection
        .execute_unprepared(&format!(
            r#"CREATE TABLE group_domain_events_next (
    sequence_no INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    tenant_id TEXT NOT NULL,
    aggregate_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version = 1),
    actor_id TEXT,
    payload TEXT NOT NULL DEFAULT '{{}}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    {kind_check}
)"#
        ))
        .await?;

    connection
        .execute_unprepared(
            r#"INSERT INTO group_domain_events_next (
    sequence_no, event_id, tenant_id, aggregate_type, aggregate_id,
    event_type, schema_version, actor_id, payload, created_at
)
SELECT
    sequence_no, event_id, tenant_id, aggregate_type, aggregate_id,
    event_type, schema_version, actor_id, payload, created_at
FROM group_domain_events
ORDER BY sequence_no"#,
        )
        .await?;
    connection
        .execute_unprepared("DROP TABLE group_domain_events")
        .await?;
    connection
        .execute_unprepared("ALTER TABLE group_domain_events_next RENAME TO group_domain_events")
        .await?;

    for statement in [
        "CREATE INDEX idx_group_domain_events_tenant_sequence ON group_domain_events (tenant_id, sequence_no)",
        "CREATE INDEX idx_group_domain_events_tenant_aggregate ON group_domain_events (tenant_id, aggregate_type, aggregate_id, sequence_no)",
        "CREATE INDEX idx_group_domain_events_tenant_type ON group_domain_events (tenant_id, event_type, sequence_no)",
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
