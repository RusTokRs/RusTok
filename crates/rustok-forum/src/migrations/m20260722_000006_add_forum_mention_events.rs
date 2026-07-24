use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const EVENT_TYPES_WITH_MENTIONS: &str = r#"
        'forum.category.created',
        'forum.category.updated',
        'forum.category.deleted',
        'forum.topic.created',
        'forum.topic.updated',
        'forum.topic.deleted',
        'forum.topic.status_changed',
        'forum.topic.pinned_changed',
        'forum.topic.lock_changed',
        'forum.reply.created',
        'forum.reply.updated',
        'forum.reply.deleted',
        'forum.reply.status_changed',
        'forum.solution.marked',
        'forum.solution.unmarked',
        'forum.topic.vote_changed',
        'forum.reply.vote_changed',
        'forum.category.subscription_changed',
        'forum.topic.subscription_changed',
        'forum.topic.tags_changed',
        'forum.mention.user_added',
        'forum.mention.audience_added'
"#;

const EVENT_TYPES_WITHOUT_MENTIONS: &str = r#"
        'forum.category.created',
        'forum.category.updated',
        'forum.category.deleted',
        'forum.topic.created',
        'forum.topic.updated',
        'forum.topic.deleted',
        'forum.topic.status_changed',
        'forum.topic.pinned_changed',
        'forum.topic.lock_changed',
        'forum.reply.created',
        'forum.reply.updated',
        'forum.reply.deleted',
        'forum.reply.status_changed',
        'forum.solution.marked',
        'forum.solution.unmarked',
        'forum.topic.vote_changed',
        'forum.reply.vote_changed',
        'forum.category.subscription_changed',
        'forum.topic.subscription_changed',
        'forum.topic.tags_changed'
"#;

const MENTION_EVENT_FILTER: &str =
    "event_type NOT IN ('forum.mention.user_added', 'forum.mention.audience_added')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => update_postgres_constraint(manager, true).await,
            DatabaseBackend::Sqlite => rebuild_sqlite_journal(manager, true).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum mention event migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => rollback_postgres(manager).await,
            DatabaseBackend::Sqlite => rebuild_sqlite_journal(manager, false).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum mention event rollback does not support {backend:?}"
            ))),
        }
    }
}

async fn update_postgres_constraint(
    manager: &SchemaManager<'_>,
    include_mentions: bool,
) -> Result<(), DbErr> {
    let event_types = if include_mentions {
        EVENT_TYPES_WITH_MENTIONS
    } else {
        EVENT_TYPES_WITHOUT_MENTIONS
    };
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
ALTER TABLE forum_domain_events
    DROP CONSTRAINT IF EXISTS chk_forum_domain_events_event_type;
ALTER TABLE forum_domain_events
    ADD CONSTRAINT chk_forum_domain_events_event_type
    CHECK (event_type IN ({event_types}));
"#
        ))
        .await?;
    Ok(())
}

async fn rollback_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
DROP TRIGGER IF EXISTS forum_domain_events_immutable_delete ON forum_domain_events;
DELETE FROM forum_domain_events WHERE {MENTION_EVENT_FILTER} IS FALSE;
ALTER TABLE forum_domain_events
    DROP CONSTRAINT IF EXISTS chk_forum_domain_events_event_type;
ALTER TABLE forum_domain_events
    ADD CONSTRAINT chk_forum_domain_events_event_type
    CHECK (event_type IN ({EVENT_TYPES_WITHOUT_MENTIONS}));
CREATE TRIGGER forum_domain_events_immutable_delete
BEFORE DELETE ON forum_domain_events
FOR EACH ROW EXECUTE FUNCTION forum_reject_domain_event_mutation();
"#
        ))
        .await?;
    Ok(())
}

async fn rebuild_sqlite_journal(
    manager: &SchemaManager<'_>,
    include_mentions: bool,
) -> Result<(), DbErr> {
    let event_types = if include_mentions {
        EVENT_TYPES_WITH_MENTIONS
    } else {
        EVENT_TYPES_WITHOUT_MENTIONS
    };
    let copy_filter = if include_mentions {
        String::new()
    } else {
        format!("WHERE {MENTION_EVENT_FILTER}")
    };
    let connection = manager.get_connection();

    connection
        .execute_unprepared("DROP TRIGGER IF EXISTS forum_domain_events_immutable_update")
        .await?;
    connection
        .execute_unprepared("DROP TRIGGER IF EXISTS forum_domain_events_immutable_delete")
        .await?;
    connection
        .execute_unprepared("DROP TABLE IF EXISTS forum_domain_events_next")
        .await?;
    connection
        .execute_unprepared(&format!(
            r#"
CREATE TABLE forum_domain_events_next (
    sequence_no INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    tenant_id TEXT NOT NULL,
    aggregate_type TEXT NOT NULL
        CHECK (aggregate_type IN ('category', 'topic', 'reply')),
    aggregate_id TEXT NOT NULL,
    event_type TEXT NOT NULL
        CHECK (event_type IN ({event_types})),
    schema_version INTEGER NOT NULL DEFAULT 1
        CHECK (schema_version = 1),
    actor_id TEXT,
    payload TEXT NOT NULL DEFAULT '{{}}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
)
"#
        ))
        .await?;
    connection
        .execute_unprepared(&format!(
            r#"
INSERT INTO forum_domain_events_next (
    sequence_no, event_id, tenant_id, aggregate_type, aggregate_id,
    event_type, schema_version, actor_id, payload, created_at
)
SELECT
    sequence_no, event_id, tenant_id, aggregate_type, aggregate_id,
    event_type, schema_version, actor_id, payload, created_at
FROM forum_domain_events
{copy_filter}
ORDER BY sequence_no
"#
        ))
        .await?;
    if manager.get_database_backend() == sea_orm_migration::sea_orm::DatabaseBackend::Sqlite {
        connection
            .execute_unprepared("DELETE FROM forum_domain_events")
            .await?;
        connection
            .execute_unprepared(
                r#"
INSERT INTO forum_domain_events (
    sequence_no, event_id, tenant_id, aggregate_type, aggregate_id,
    event_type, schema_version, actor_id, payload, created_at
)
SELECT
    sequence_no, event_id, tenant_id, aggregate_type, aggregate_id,
    event_type, schema_version, actor_id, payload, created_at
FROM forum_domain_events_next
"#,
            )
            .await?;
        connection
            .execute_unprepared("DROP TABLE forum_domain_events_next")
            .await?;
    } else {
        connection
            .execute_unprepared("DROP TABLE forum_domain_events")
            .await?;
        connection
            .execute_unprepared("ALTER TABLE forum_domain_events_next RENAME TO forum_domain_events")
            .await?;
    }

    for statement in [
        "CREATE INDEX IF NOT EXISTS idx_forum_domain_events_tenant_sequence \
         ON forum_domain_events (tenant_id, sequence_no)",
        "CREATE INDEX IF NOT EXISTS idx_forum_domain_events_tenant_aggregate \
         ON forum_domain_events (tenant_id, aggregate_type, aggregate_id, sequence_no)",
        "CREATE INDEX IF NOT EXISTS idx_forum_domain_events_tenant_type \
         ON forum_domain_events (tenant_id, event_type, sequence_no)",
        r#"CREATE TRIGGER forum_domain_events_immutable_update
           BEFORE UPDATE ON forum_domain_events
           FOR EACH ROW
           BEGIN
               SELECT RAISE(ABORT, 'forum domain events are append-only');
           END"#,
        r#"CREATE TRIGGER forum_domain_events_immutable_delete
           BEFORE DELETE ON forum_domain_events
           FOR EACH ROW
           BEGIN
               SELECT RAISE(ABORT, 'forum domain events are append-only');
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
