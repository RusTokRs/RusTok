use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn schema(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    for statement in [
        r##"CREATE TABLE IF NOT EXISTS forum_domain_events (
    sequence_no INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    tenant_id TEXT NOT NULL,
    aggregate_type TEXT NOT NULL
        CHECK (aggregate_type IN ('category', 'topic', 'reply')),
    aggregate_id TEXT NOT NULL,
    event_type TEXT NOT NULL
        CHECK (event_type IN (
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
        )),
    schema_version INTEGER NOT NULL DEFAULT 1
        CHECK (schema_version = 1),
    actor_id TEXT,
    payload TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
)"##,
        r##"CREATE INDEX IF NOT EXISTS idx_forum_domain_events_tenant_sequence
   ON forum_domain_events (tenant_id, sequence_no)"##,
        r##"CREATE INDEX IF NOT EXISTS idx_forum_domain_events_tenant_aggregate
   ON forum_domain_events (tenant_id, aggregate_type, aggregate_id, sequence_no)"##,
        r##"CREATE INDEX IF NOT EXISTS idx_forum_domain_events_tenant_type
   ON forum_domain_events (tenant_id, event_type, sequence_no)"##,
        r##"DROP TRIGGER IF EXISTS forum_domain_events_immutable_update"##,
        r##"DROP TRIGGER IF EXISTS forum_domain_events_immutable_delete"##,
        r##"CREATE TRIGGER forum_domain_events_immutable_update
   BEFORE UPDATE ON forum_domain_events
   FOR EACH ROW
   BEGIN
       SELECT RAISE(ABORT, 'forum domain events are append-only');
   END"##,
        r##"CREATE TRIGGER forum_domain_events_immutable_delete
   BEFORE DELETE ON forum_domain_events
   FOR EACH ROW
   BEGIN
       SELECT RAISE(ABORT, 'forum domain events are append-only');
   END"##,
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}
