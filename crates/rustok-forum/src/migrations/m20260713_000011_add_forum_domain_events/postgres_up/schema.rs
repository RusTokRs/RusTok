use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn schema(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE OR REPLACE FUNCTION forum_generate_event_uuid()
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

CREATE TABLE IF NOT EXISTS forum_domain_events (
    sequence_no BIGSERIAL PRIMARY KEY,
    event_id UUID NOT NULL DEFAULT forum_generate_event_uuid(),
    tenant_id UUID NOT NULL,
    aggregate_type VARCHAR(32) NOT NULL,
    aggregate_id UUID NOT NULL,
    event_type VARCHAR(96) NOT NULL,
    schema_version SMALLINT NOT NULL DEFAULT 1,
    actor_id UUID,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_forum_domain_events_event_id UNIQUE (event_id),
    CONSTRAINT chk_forum_domain_events_aggregate_type
        CHECK (aggregate_type IN ('category', 'topic', 'reply')),
    CONSTRAINT chk_forum_domain_events_schema_version
        CHECK (schema_version = 1),
    CONSTRAINT chk_forum_domain_events_event_type
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
        ))
);

CREATE INDEX IF NOT EXISTS idx_forum_domain_events_tenant_sequence
    ON forum_domain_events (tenant_id, sequence_no);
CREATE INDEX IF NOT EXISTS idx_forum_domain_events_tenant_aggregate
    ON forum_domain_events (tenant_id, aggregate_type, aggregate_id, sequence_no);
CREATE INDEX IF NOT EXISTS idx_forum_domain_events_tenant_type
    ON forum_domain_events (tenant_id, event_type, sequence_no);

CREATE OR REPLACE FUNCTION forum_append_domain_event(
    p_tenant_id uuid,
    p_aggregate_type text,
    p_aggregate_id uuid,
    p_event_type text,
    p_actor_id uuid,
    p_payload jsonb
)
RETURNS void AS $$
BEGIN
    INSERT INTO forum_domain_events (
        tenant_id,
        aggregate_type,
        aggregate_id,
        event_type,
        schema_version,
        actor_id,
        payload
    )
    VALUES (
        p_tenant_id,
        p_aggregate_type,
        p_aggregate_id,
        p_event_type,
        1,
        p_actor_id,
        COALESCE(p_payload, '{}'::jsonb)
    );
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_reject_domain_event_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum domain events are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_domain_events_immutable_update ON forum_domain_events;
CREATE TRIGGER forum_domain_events_immutable_update
BEFORE UPDATE ON forum_domain_events
FOR EACH ROW EXECUTE FUNCTION forum_reject_domain_event_mutation();

DROP TRIGGER IF EXISTS forum_domain_events_immutable_delete ON forum_domain_events;
CREATE TRIGGER forum_domain_events_immutable_delete
BEFORE DELETE ON forum_domain_events
FOR EACH ROW EXECUTE FUNCTION forum_reject_domain_event_mutation();
"#,
        )
        .await?;
    Ok(())
}
