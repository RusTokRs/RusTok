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
    resource_revision VARCHAR(96) NOT NULL,
    lifecycle VARCHAR(16) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_flex_standalone_translation_change_tx_resource
        UNIQUE (tx_id, tenant_id, entry_id, schema_id),
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
        CHECK (lifecycle IN ('active', 'archived', 'deleted'))
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

-- Collapse every aggregate mutation in one database transaction into one visible revision for one
-- stable neutral identity. Schema moves intentionally use two identities, so the journal uniqueness
-- includes schema_id and can retain the old tombstone together with the new active/archived row.
CREATE OR REPLACE FUNCTION rustok_flex_bump_standalone_translation_resource(
    p_tenant_id UUID,
    p_entry_id UUID,
    p_schema_id UUID,
    p_lifecycle TEXT
) RETURNS TEXT
LANGUAGE plpgsql
AS $$
DECLARE
    current_tx BIGINT := txid_current();
    next_revision BIGINT;
    revision_token TEXT;
BEGIN
    IF p_lifecycle NOT IN ('active', 'archived') THEN
        RAISE EXCEPTION 'invalid standalone Translation lifecycle: %', p_lifecycle;
    END IF;

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
        current_tx, p_tenant_id, p_entry_id, p_schema_id, revision_token, p_lifecycle
    )
    ON CONFLICT (tx_id, tenant_id, entry_id, schema_id)
    DO UPDATE SET
        resource_revision = EXCLUDED.resource_revision,
        lifecycle = EXCLUDED.lifecycle;

    RETURN revision_token;
END;
$$;

-- Touch a currently existing entry using the owning schema lifecycle. If the entry disappeared
-- because a parent delete is cascading, the canonical entry/schema delete trigger owns the final
-- tombstone and this helper intentionally does nothing instead of resurrecting durable state.
CREATE OR REPLACE FUNCTION rustok_flex_touch_standalone_translation_entry(
    p_tenant_id UUID,
    p_entry_id UUID
) RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    row_schema UUID;
    row_lifecycle TEXT;
BEGIN
    SELECT entry.schema_id,
           CASE WHEN schema.is_active THEN 'active' ELSE 'archived' END
      INTO row_schema, row_lifecycle
    FROM flex_entries entry
    JOIN flex_schemas schema
      ON schema.tenant_id = entry.tenant_id
     AND schema.id = entry.schema_id
    WHERE entry.tenant_id = p_tenant_id
      AND entry.id = p_entry_id;

    IF row_schema IS NOT NULL THEN
        PERFORM rustok_flex_bump_standalone_translation_resource(
            p_tenant_id, p_entry_id, row_schema, row_lifecycle
        );
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_record_flex_standalone_translation_entry_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    new_lifecycle TEXT;
    identity_changed BOOLEAN;
BEGIN
    IF TG_OP = 'INSERT' THEN
        SELECT CASE WHEN schema.is_active THEN 'active' ELSE 'archived' END
          INTO new_lifecycle
        FROM flex_schemas schema
        WHERE schema.tenant_id = NEW.tenant_id
          AND schema.id = NEW.schema_id;
        IF new_lifecycle IS NULL THEN
            RAISE EXCEPTION 'standalone Translation entry % has no owning schema %', NEW.id, NEW.schema_id;
        END IF;
        PERFORM rustok_flex_bump_standalone_translation_resource(
            NEW.tenant_id, NEW.id, NEW.schema_id, new_lifecycle
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
        ON CONFLICT (tx_id, tenant_id, entry_id, schema_id)
        DO UPDATE SET
            resource_revision = EXCLUDED.resource_revision,
            lifecycle = 'deleted';

        DELETE FROM flex_standalone_translation_resource_state
        WHERE tenant_id = OLD.tenant_id AND entry_id = OLD.id;
        RETURN OLD;
    END IF;

    -- Shared JSON and status are not part of the neutral Translation snapshot. Only a stable
    -- identity move changes the entry-side snapshot; localized values have their own trigger.
    identity_changed := OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.id IS DISTINCT FROM NEW.id
        OR OLD.schema_id IS DISTINCT FROM NEW.schema_id;
    IF NOT identity_changed THEN
        RETURN NEW;
    END IF;

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
    ON CONFLICT (tx_id, tenant_id, entry_id, schema_id)
    DO UPDATE SET
        resource_revision = EXCLUDED.resource_revision,
        lifecycle = 'deleted';

    DELETE FROM flex_standalone_translation_resource_state
    WHERE tenant_id = OLD.tenant_id AND entry_id = OLD.id;

    SELECT CASE WHEN schema.is_active THEN 'active' ELSE 'archived' END
      INTO new_lifecycle
    FROM flex_schemas schema
    WHERE schema.tenant_id = NEW.tenant_id
      AND schema.id = NEW.schema_id;
    IF new_lifecycle IS NULL THEN
        RAISE EXCEPTION 'standalone Translation moved entry % has no owning schema %', NEW.id, NEW.schema_id;
    END IF;
    PERFORM rustok_flex_bump_standalone_translation_resource(
        NEW.tenant_id, NEW.id, NEW.schema_id, new_lifecycle
    );
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_flex_standalone_translation_entry_change
AFTER INSERT OR UPDATE OR DELETE ON flex_entries
FOR EACH ROW EXECUTE FUNCTION rustok_record_flex_standalone_translation_entry_change();

-- Serialize every localized-value mutation through its parent entry. This makes source/target CAS
-- and the revision journal safe even for SQL writers that bypass the canonical host facade.
CREATE OR REPLACE FUNCTION rustok_lock_flex_standalone_translation_locale_parent()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM 1
        FROM flex_entries entry
        WHERE entry.tenant_id = NEW.tenant_id
          AND entry.id = NEW.entry_id
        FOR UPDATE;
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        PERFORM 1
        FROM flex_entries entry
        WHERE entry.tenant_id = OLD.tenant_id
          AND entry.id = OLD.entry_id
        FOR UPDATE;
        RETURN OLD;
    END IF;

    IF OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
       AND OLD.entry_id IS NOT DISTINCT FROM NEW.entry_id THEN
        PERFORM 1
        FROM flex_entries entry
        WHERE entry.tenant_id = NEW.tenant_id
          AND entry.id = NEW.entry_id
        FOR UPDATE;
    ELSE
        PERFORM 1
        FROM flex_entries entry
        WHERE (entry.tenant_id = OLD.tenant_id AND entry.id = OLD.entry_id)
           OR (entry.tenant_id = NEW.tenant_id AND entry.id = NEW.entry_id)
        ORDER BY entry.tenant_id, entry.id
        FOR UPDATE;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_flex_standalone_translation_locale_parent_lock
BEFORE INSERT OR UPDATE OR DELETE ON flex_entry_localized_values
FOR EACH ROW EXECUTE FUNCTION rustok_lock_flex_standalone_translation_locale_parent();

CREATE OR REPLACE FUNCTION rustok_record_flex_standalone_translation_locale_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    same_resource BOOLEAN;
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM rustok_flex_touch_standalone_translation_entry(NEW.tenant_id, NEW.entry_id);
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        PERFORM rustok_flex_touch_standalone_translation_entry(OLD.tenant_id, OLD.entry_id);
        RETURN OLD;
    END IF;

    IF OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
       AND OLD.entry_id IS NOT DISTINCT FROM NEW.entry_id
       AND OLD.locale IS NOT DISTINCT FROM NEW.locale
       AND OLD.data::text IS NOT DISTINCT FROM NEW.data::text THEN
        RETURN NEW;
    END IF;

    same_resource := OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
        AND OLD.entry_id IS NOT DISTINCT FROM NEW.entry_id;
    PERFORM rustok_flex_touch_standalone_translation_entry(OLD.tenant_id, OLD.entry_id);
    IF NOT same_resource THEN
        PERFORM rustok_flex_touch_standalone_translation_entry(NEW.tenant_id, NEW.entry_id);
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_flex_standalone_translation_locale_change
AFTER INSERT OR UPDATE OR DELETE ON flex_entry_localized_values
FOR EACH ROW EXECUTE FUNCTION rustok_record_flex_standalone_translation_locale_change();

-- A schema definition/lifecycle edit changes the Translation field set/lifecycle for every entry
-- even when no entry row changes. Lock affected entries before changing the schema so exact-locale
-- applies serialize before or after the schema revision instead of observing an interleaving.
CREATE OR REPLACE FUNCTION rustok_record_flex_standalone_translation_schema_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    resource RECORD;
    new_lifecycle TEXT;
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM 1
        FROM flex_entries entry
        WHERE entry.tenant_id = OLD.tenant_id
          AND entry.schema_id = OLD.id
        ORDER BY entry.id
        FOR UPDATE;

        FOR resource IN
            SELECT entry.tenant_id, entry.id AS entry_id, entry.schema_id
            FROM flex_entries entry
            WHERE entry.tenant_id = OLD.tenant_id
              AND entry.schema_id = OLD.id
            ORDER BY entry.id
        LOOP
            INSERT INTO flex_standalone_translation_change_journal (
                tx_id, tenant_id, entry_id, schema_id, resource_revision, lifecycle
            )
            SELECT
                txid_current(), resource.tenant_id, resource.entry_id, resource.schema_id,
                format('deleted:%s:%s', resource.schema_id, resource.entry_id), 'deleted'
            WHERE EXISTS (
                SELECT 1 FROM flex_standalone_translation_resource_state state
                WHERE state.tenant_id = resource.tenant_id
                  AND state.entry_id = resource.entry_id
            )
            ON CONFLICT (tx_id, tenant_id, entry_id, schema_id)
            DO UPDATE SET
                resource_revision = EXCLUDED.resource_revision,
                lifecycle = 'deleted';

            DELETE FROM flex_standalone_translation_resource_state
            WHERE tenant_id = resource.tenant_id
              AND entry_id = resource.entry_id;
        END LOOP;
        RETURN OLD;
    END IF;

    IF OLD.fields_config::text IS NOT DISTINCT FROM NEW.fields_config::text
       AND OLD.is_active IS NOT DISTINCT FROM NEW.is_active THEN
        RETURN NEW;
    END IF;

    PERFORM 1
    FROM flex_entries entry
    WHERE entry.tenant_id = OLD.tenant_id
      AND entry.schema_id = OLD.id
    ORDER BY entry.id
    FOR UPDATE;

    new_lifecycle := CASE WHEN NEW.is_active THEN 'active' ELSE 'archived' END;
    FOR resource IN
        SELECT entry.tenant_id, entry.id AS entry_id, entry.schema_id
        FROM flex_entries entry
        WHERE entry.tenant_id = OLD.tenant_id
          AND entry.schema_id = OLD.id
        ORDER BY entry.id
    LOOP
        PERFORM rustok_flex_bump_standalone_translation_resource(
            resource.tenant_id, resource.entry_id, resource.schema_id, new_lifecycle
        );
    END LOOP;

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
DROP TRIGGER IF EXISTS trg_flex_standalone_translation_locale_parent_lock ON flex_entry_localized_values;
DROP FUNCTION IF EXISTS rustok_lock_flex_standalone_translation_locale_parent();
DROP TRIGGER IF EXISTS trg_flex_standalone_translation_entry_change ON flex_entries;
DROP FUNCTION IF EXISTS rustok_record_flex_standalone_translation_entry_change();
DROP FUNCTION IF EXISTS rustok_flex_touch_standalone_translation_entry(UUID, UUID);
DROP FUNCTION IF EXISTS rustok_flex_bump_standalone_translation_resource(UUID, UUID, UUID, TEXT);
DROP TABLE IF EXISTS flex_standalone_translation_change_journal;
DROP TABLE IF EXISTS flex_standalone_translation_resource_state;
"#,
            )
            .await?;
        Ok(())
    }
}
