use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DatabaseBackend},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE TABLE flex_standalone_translation_resource_state (
    tenant_id UUID NOT NULL,
    entry_id UUID NOT NULL,
    schema_id UUID NOT NULL,
    revision BIGINT NOT NULL,
    last_tx_id BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, entry_id),
    CONSTRAINT chk_flex_standalone_translation_state_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_standalone_translation_state_entry_non_nil
        CHECK (entry_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_standalone_translation_state_schema_non_nil
        CHECK (schema_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_standalone_translation_state_revision_positive
        CHECK (revision > 0),
    CONSTRAINT chk_flex_standalone_translation_state_tx_nonnegative
        CHECK (last_tx_id >= 0)
);

CREATE TABLE flex_standalone_translation_change_journal (
    change_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tx_id BIGINT NOT NULL,
    tenant_id UUID NOT NULL,
    entry_id UUID NOT NULL,
    schema_id UUID NOT NULL,
    resource_revision VARCHAR(64) NOT NULL,
    lifecycle VARCHAR(16) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_flex_standalone_translation_change_tx_resource
        UNIQUE (tx_id, tenant_id, entry_id),
    CONSTRAINT chk_flex_standalone_translation_change_seq_positive CHECK (change_seq > 0),
    CONSTRAINT chk_flex_standalone_translation_change_tx_positive CHECK (tx_id > 0),
    CONSTRAINT chk_flex_standalone_translation_change_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_standalone_translation_change_entry_non_nil
        CHECK (entry_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_standalone_translation_change_schema_non_nil
        CHECK (schema_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_standalone_translation_change_revision_nonblank
        CHECK (length(trim(resource_revision)) > 0),
    CONSTRAINT chk_flex_standalone_translation_change_lifecycle
        CHECK (lifecycle IN ('active', 'deleted'))
);

CREATE INDEX idx_flex_standalone_translation_change_tenant_seq
    ON flex_standalone_translation_change_journal (tenant_id, change_seq);
CREATE INDEX idx_flex_standalone_translation_change_resource_seq
    ON flex_standalone_translation_change_journal (tenant_id, entry_id, change_seq DESC);
CREATE INDEX idx_flex_standalone_translation_change_schema_seq
    ON flex_standalone_translation_change_journal (tenant_id, schema_id, change_seq);

-- Existing standalone entries get a durable resource revision without fabricating history.
INSERT INTO flex_standalone_translation_resource_state (
    tenant_id, entry_id, schema_id, revision, last_tx_id
)
SELECT tenant_id, id, schema_id, 1, 0
FROM flex_entries
ON CONFLICT (tenant_id, entry_id) DO NOTHING;

-- Collapse every aggregate mutation in one database transaction into one visible revision.
CREATE OR REPLACE FUNCTION rustok_flex_bump_standalone_translation_resource(
    p_tenant_id UUID,
    p_entry_id UUID,
    p_schema_id UUID
) RETURNS TEXT
LANGUAGE plpgsql
AS $$
DECLARE
    current_tx BIGINT := txid_current();
    next_revision BIGINT;
    revision_token TEXT;
BEGIN
    INSERT INTO flex_standalone_translation_resource_state (
        tenant_id, entry_id, schema_id, revision, last_tx_id, updated_at
    ) VALUES (
        p_tenant_id, p_entry_id, p_schema_id, 1, current_tx, CURRENT_TIMESTAMP
    )
    ON CONFLICT (tenant_id, entry_id)
    DO UPDATE SET
        schema_id = EXCLUDED.schema_id,
        revision = CASE
            WHEN flex_standalone_translation_resource_state.last_tx_id = EXCLUDED.last_tx_id
                THEN flex_standalone_translation_resource_state.revision
            ELSE flex_standalone_translation_resource_state.revision + 1
        END,
        last_tx_id = EXCLUDED.last_tx_id,
        updated_at = CURRENT_TIMESTAMP
    RETURNING revision INTO next_revision;

    revision_token := format('standalone:%s', next_revision);
    INSERT INTO flex_standalone_translation_change_journal (
        tx_id, tenant_id, entry_id, schema_id, resource_revision, lifecycle
    ) VALUES (
        current_tx, p_tenant_id, p_entry_id, p_schema_id, revision_token, 'active'
    )
    ON CONFLICT (tx_id, tenant_id, entry_id)
    DO UPDATE SET
        schema_id = EXCLUDED.schema_id,
        resource_revision = EXCLUDED.resource_revision,
        lifecycle = 'active';

    RETURN revision_token;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_record_flex_standalone_translation_entry_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    revision_token TEXT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM rustok_flex_bump_standalone_translation_resource(
            NEW.tenant_id, NEW.id, NEW.schema_id
        );
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        INSERT INTO flex_standalone_translation_change_journal (
            tx_id, tenant_id, entry_id, schema_id, resource_revision, lifecycle
        )
        SELECT
            txid_current(), OLD.tenant_id, OLD.id, OLD.schema_id,
            format('deleted:%s:%s', OLD.schema_id, OLD.id), 'deleted'
        WHERE EXISTS (
            SELECT 1 FROM flex_standalone_translation_resource_state state
            WHERE state.tenant_id = OLD.tenant_id AND state.entry_id = OLD.id
        )
        ON CONFLICT (tx_id, tenant_id, entry_id)
        DO UPDATE SET
            schema_id = EXCLUDED.schema_id,
            resource_revision = EXCLUDED.resource_revision,
            lifecycle = 'deleted';

        DELETE FROM flex_standalone_translation_resource_state
        WHERE tenant_id = OLD.tenant_id AND entry_id = OLD.id;
        RETURN OLD;
    END IF;

    -- Shared JSON does not contain localized fields. Only identity/lifecycle changes alter the
    -- Translation snapshot; localized payload mutations are captured by their own trigger.
    IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
       OR OLD.schema_id IS DISTINCT FROM NEW.schema_id
       OR OLD.status IS DISTINCT FROM NEW.status THEN
        IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
           OR OLD.id IS DISTINCT FROM NEW.id THEN
            INSERT INTO flex_standalone_translation_change_journal (
                tx_id, tenant_id, entry_id, schema_id, resource_revision, lifecycle
            )
            SELECT
                txid_current(), OLD.tenant_id, OLD.id, OLD.schema_id,
                format('deleted:%s:%s', OLD.schema_id, OLD.id), 'deleted'
            WHERE EXISTS (
                SELECT 1 FROM flex_standalone_translation_resource_state state
                WHERE state.tenant_id = OLD.tenant_id AND state.entry_id = OLD.id
            )
            ON CONFLICT (tx_id, tenant_id, entry_id)
            DO UPDATE SET
                schema_id = EXCLUDED.schema_id,
                resource_revision = EXCLUDED.resource_revision,
                lifecycle = 'deleted';
            DELETE FROM flex_standalone_translation_resource_state
            WHERE tenant_id = OLD.tenant_id AND entry_id = OLD.id;
        END IF;
        PERFORM rustok_flex_bump_standalone_translation_resource(
            NEW.tenant_id, NEW.id, NEW.schema_id
        );
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_flex_standalone_translation_entry_change
AFTER INSERT OR UPDATE OR DELETE ON flex_entries
FOR EACH ROW EXECUTE FUNCTION rustok_record_flex_standalone_translation_entry_change();

CREATE OR REPLACE FUNCTION rustok_record_flex_standalone_translation_locale_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    row_tenant UUID;
    row_entry UUID;
    row_schema UUID;
BEGIN
    row_tenant := COALESCE(NEW.tenant_id, OLD.tenant_id);
    row_entry := COALESCE(NEW.entry_id, OLD.entry_id);

    SELECT entry.schema_id INTO row_schema
    FROM flex_entries entry
    WHERE entry.tenant_id = row_tenant AND entry.id = row_entry;

    IF row_schema IS NULL THEN
        SELECT state.schema_id INTO row_schema
        FROM flex_standalone_translation_resource_state state
        WHERE state.tenant_id = row_tenant AND state.entry_id = row_entry;
    END IF;

    IF row_schema IS NOT NULL THEN
        IF TG_OP = 'UPDATE'
           AND OLD.locale IS NOT DISTINCT FROM NEW.locale
           AND OLD.data::text IS NOT DISTINCT FROM NEW.data::text THEN
            RETURN NEW;
        END IF;
        PERFORM rustok_flex_bump_standalone_translation_resource(
            row_tenant, row_entry, row_schema
        );
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_flex_standalone_translation_locale_change
AFTER INSERT OR UPDATE OR DELETE ON flex_entry_localized_values
FOR EACH ROW EXECUTE FUNCTION rustok_record_flex_standalone_translation_locale_change();

-- A schema definition/lifecycle edit changes the Translation field set for every entry even when
-- no entry row changes. Fan it out while preserving the one-revision-per-transaction invariant.
CREATE OR REPLACE FUNCTION rustok_record_flex_standalone_translation_schema_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    resource RECORD;
BEGIN
    IF TG_OP = 'UPDATE'
       AND OLD.fields_config::text IS NOT DISTINCT FROM NEW.fields_config::text
       AND OLD.is_active IS NOT DISTINCT FROM NEW.is_active THEN
        RETURN NEW;
    END IF;

    FOR resource IN
        SELECT entry.tenant_id, entry.id AS entry_id, entry.schema_id
        FROM flex_entries entry
        WHERE entry.tenant_id = COALESCE(NEW.tenant_id, OLD.tenant_id)
          AND entry.schema_id = COALESCE(NEW.id, OLD.id)
    LOOP
        PERFORM rustok_flex_bump_standalone_translation_resource(
            resource.tenant_id, resource.entry_id, resource.schema_id
        );
    END LOOP;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_flex_standalone_translation_schema_change
BEFORE UPDATE OR DELETE ON flex_schemas
FOR EACH ROW EXECUTE FUNCTION rustok_record_flex_standalone_translation_schema_change();
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP TRIGGER IF EXISTS trg_flex_standalone_translation_schema_change ON flex_schemas;
DROP FUNCTION IF EXISTS rustok_record_flex_standalone_translation_schema_change();
DROP TRIGGER IF EXISTS trg_flex_standalone_translation_locale_change ON flex_entry_localized_values;
DROP FUNCTION IF EXISTS rustok_record_flex_standalone_translation_locale_change();
DROP TRIGGER IF EXISTS trg_flex_standalone_translation_entry_change ON flex_entries;
DROP FUNCTION IF EXISTS rustok_record_flex_standalone_translation_entry_change();
DROP FUNCTION IF EXISTS rustok_flex_bump_standalone_translation_resource(UUID, UUID, UUID);
DROP TABLE IF EXISTS flex_standalone_translation_change_journal;
DROP TABLE IF EXISTS flex_standalone_translation_resource_state;
"#,
            )
            .await?;
        Ok(())
    }
}
