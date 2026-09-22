use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DatabaseBackend},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Durable ordered ChangeCursor is a PostgreSQL production capability. SQLite remains
        // available for portable owner tests, but must not pretend to provide event ordering.
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE TABLE flex_attached_translation_resource_state (
    tenant_id UUID NOT NULL,
    entity_type VARCHAR(64) NOT NULL,
    entity_id UUID NOT NULL,
    revision BIGINT NOT NULL,
    last_tx_id BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, entity_type, entity_id),
    CONSTRAINT chk_flex_attached_translation_state_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_attached_translation_state_entity_type_nonblank
        CHECK (length(trim(entity_type)) > 0),
    CONSTRAINT chk_flex_attached_translation_state_entity_non_nil
        CHECK (entity_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_attached_translation_state_revision_positive
        CHECK (revision > 0),
    CONSTRAINT chk_flex_attached_translation_state_tx_nonnegative
        CHECK (last_tx_id >= 0)
);

CREATE TABLE flex_attached_translation_change_journal (
    change_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tx_id BIGINT NOT NULL,
    tenant_id UUID NOT NULL,
    entity_type VARCHAR(64) NOT NULL,
    entity_id UUID NOT NULL,
    resource_revision VARCHAR(64) NOT NULL,
    lifecycle VARCHAR(16) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_flex_attached_translation_change_tx_resource
        UNIQUE (tx_id, tenant_id, entity_type, entity_id),
    CONSTRAINT chk_flex_attached_translation_change_seq_positive
        CHECK (change_seq > 0),
    CONSTRAINT chk_flex_attached_translation_change_tx_positive
        CHECK (tx_id > 0),
    CONSTRAINT chk_flex_attached_translation_change_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_attached_translation_change_entity_type_nonblank
        CHECK (length(trim(entity_type)) > 0),
    CONSTRAINT chk_flex_attached_translation_change_entity_non_nil
        CHECK (entity_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_attached_translation_change_revision_nonblank
        CHECK (length(trim(resource_revision)) > 0),
    CONSTRAINT chk_flex_attached_translation_change_lifecycle
        CHECK (lifecycle IN ('active', 'deleted'))
);

CREATE INDEX idx_flex_attached_translation_change_tenant_seq
    ON flex_attached_translation_change_journal (tenant_id, change_seq);

CREATE INDEX idx_flex_attached_translation_change_scope_seq
    ON flex_attached_translation_change_journal (tenant_id, entity_type, change_seq);

CREATE INDEX idx_flex_attached_translation_change_resource_seq
    ON flex_attached_translation_change_journal (
        tenant_id,
        entity_type,
        entity_id,
        change_seq DESC
    );

-- Existing exact attached Translation resources start at revision 1. This establishes state
-- without manufacturing historical ChangeCursor rows for mutations that predate the journal.
INSERT INTO flex_attached_translation_resource_state (
    tenant_id,
    entity_type,
    entity_id,
    revision,
    last_tx_id
)
SELECT DISTINCT
    value.tenant_id,
    value.entity_type,
    value.entity_id,
    1,
    0
FROM flex_attached_localized_values value
JOIN flex_attached_field_definitions definition
  ON definition.tenant_id = value.tenant_id
 AND definition.entity_type = value.entity_type
 AND definition.field_key = value.field_key
WHERE definition.is_active = TRUE
  AND definition.is_localized = TRUE
  AND definition.field_type IN ('text', 'textarea')
ON CONFLICT (tenant_id, entity_type, entity_id) DO NOTHING;

-- One resource gets at most one externally visible revision per database transaction. Flex
-- writes can touch several field rows, and schema mutations can fan out through several trigger
-- paths, but readers can observe only the committed aggregate state.
CREATE OR REPLACE FUNCTION rustok_flex_bump_attached_translation_resource(
    p_tenant_id UUID,
    p_entity_type TEXT,
    p_entity_id UUID
) RETURNS TEXT
LANGUAGE plpgsql
AS $$
DECLARE
    current_tx BIGINT := txid_current();
    next_revision BIGINT;
    revision_token TEXT;
BEGIN
    INSERT INTO flex_attached_translation_resource_state (
        tenant_id,
        entity_type,
        entity_id,
        revision,
        last_tx_id,
        updated_at
    ) VALUES (
        p_tenant_id,
        p_entity_type,
        p_entity_id,
        1,
        current_tx,
        CURRENT_TIMESTAMP
    )
    ON CONFLICT (tenant_id, entity_type, entity_id)
    DO UPDATE SET
        revision = CASE
            WHEN flex_attached_translation_resource_state.last_tx_id = EXCLUDED.last_tx_id
                THEN flex_attached_translation_resource_state.revision
            ELSE flex_attached_translation_resource_state.revision + 1
        END,
        last_tx_id = EXCLUDED.last_tx_id,
        updated_at = CURRENT_TIMESTAMP
    RETURNING revision INTO next_revision;

    revision_token := format('attached:%s', next_revision);
    INSERT INTO flex_attached_translation_change_journal (
        tx_id,
        tenant_id,
        entity_type,
        entity_id,
        resource_revision,
        lifecycle
    ) VALUES (
        current_tx,
        p_tenant_id,
        p_entity_type,
        p_entity_id,
        revision_token,
        'active'
    )
    ON CONFLICT (tx_id, tenant_id, entity_type, entity_id)
    DO UPDATE SET
        resource_revision = EXCLUDED.resource_revision,
        lifecycle = 'active';

    RETURN revision_token;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_flex_attached_translation_field_eligible(
    p_tenant_id UUID,
    p_entity_type TEXT,
    p_field_key TEXT
) RETURNS BOOLEAN
LANGUAGE sql
STABLE
AS $$
    SELECT EXISTS (
        SELECT 1
        FROM flex_attached_field_definitions definition
        WHERE definition.tenant_id = p_tenant_id
          AND definition.entity_type = p_entity_type
          AND definition.field_key = p_field_key
          AND definition.is_active = TRUE
          AND definition.is_localized = TRUE
          AND definition.field_type IN ('text', 'textarea')
    );
$$;

CREATE OR REPLACE FUNCTION rustok_record_flex_attached_translation_resource_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    old_relevant BOOLEAN := FALSE;
    new_relevant BOOLEAN := FALSE;
    same_resource BOOLEAN := FALSE;
BEGIN
    IF TG_OP <> 'INSERT' THEN
        old_relevant := rustok_flex_attached_translation_field_eligible(
            OLD.tenant_id,
            OLD.entity_type,
            OLD.field_key
        );
    END IF;
    IF TG_OP <> 'DELETE' THEN
        new_relevant := rustok_flex_attached_translation_field_eligible(
            NEW.tenant_id,
            NEW.entity_type,
            NEW.field_key
        );
    END IF;

    IF TG_OP = 'INSERT' THEN
        IF new_relevant THEN
            PERFORM rustok_flex_bump_attached_translation_resource(
                NEW.tenant_id,
                NEW.entity_type,
                NEW.entity_id
            );
        END IF;
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        IF old_relevant THEN
            PERFORM rustok_flex_bump_attached_translation_resource(
                OLD.tenant_id,
                OLD.entity_type,
                OLD.entity_id
            );
        END IF;
        RETURN OLD;
    END IF;

    IF NOT old_relevant AND NOT new_relevant THEN
        RETURN NEW;
    END IF;

    IF OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
       AND OLD.entity_type IS NOT DISTINCT FROM NEW.entity_type
       AND OLD.entity_id IS NOT DISTINCT FROM NEW.entity_id
       AND OLD.field_key IS NOT DISTINCT FROM NEW.field_key
       AND OLD.locale IS NOT DISTINCT FROM NEW.locale
       AND OLD.value::text IS NOT DISTINCT FROM NEW.value::text THEN
        RETURN NEW;
    END IF;

    same_resource := OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
        AND OLD.entity_type IS NOT DISTINCT FROM NEW.entity_type
        AND OLD.entity_id IS NOT DISTINCT FROM NEW.entity_id;

    IF old_relevant THEN
        PERFORM rustok_flex_bump_attached_translation_resource(
            OLD.tenant_id,
            OLD.entity_type,
            OLD.entity_id
        );
    END IF;

    IF new_relevant AND (NOT same_resource OR NOT old_relevant) THEN
        PERFORM rustok_flex_bump_attached_translation_resource(
            NEW.tenant_id,
            NEW.entity_type,
            NEW.entity_id
        );
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_flex_attached_translation_value_change
AFTER INSERT OR UPDATE OR DELETE ON flex_attached_localized_values
FOR EACH ROW EXECUTE FUNCTION rustok_record_flex_attached_translation_resource_change();

-- Field-definition DELETE/UPDATE can cascade into localized rows. Capture the OLD resource set
-- before those FK cascades run so removing or moving a translation-relevant field cannot erase
-- the very resource ids whose snapshots changed.
CREATE OR REPLACE FUNCTION rustok_record_flex_attached_translation_schema_change_before()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    old_eligible BOOLEAN := FALSE;
    resource RECORD;
BEGIN
    IF TG_OP = 'UPDATE'
       AND OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
       AND OLD.entity_type IS NOT DISTINCT FROM NEW.entity_type
       AND OLD.field_key IS NOT DISTINCT FROM NEW.field_key
       AND OLD.field_type IS NOT DISTINCT FROM NEW.field_type
       AND OLD.is_localized IS NOT DISTINCT FROM NEW.is_localized
       AND OLD.is_required IS NOT DISTINCT FROM NEW.is_required
       AND OLD.validation::text IS NOT DISTINCT FROM NEW.validation::text
       AND OLD.is_active IS NOT DISTINCT FROM NEW.is_active THEN
        RETURN NEW;
    END IF;

    old_eligible := OLD.is_active
        AND OLD.is_localized
        AND OLD.field_type IN ('text', 'textarea');
    IF old_eligible THEN
        FOR resource IN
            SELECT DISTINCT value.tenant_id, value.entity_type, value.entity_id
            FROM flex_attached_localized_values value
            WHERE value.tenant_id = OLD.tenant_id
              AND value.entity_type = OLD.entity_type
              AND value.field_key = OLD.field_key
        LOOP
            PERFORM rustok_flex_bump_attached_translation_resource(
                resource.tenant_id,
                resource.entity_type,
                resource.entity_id
            );
        END LOOP;
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_flex_attached_translation_schema_change_before
BEFORE UPDATE OR DELETE ON flex_attached_field_definitions
FOR EACH ROW EXECUTE FUNCTION rustok_record_flex_attached_translation_schema_change_before();

-- INSERT/UPDATE NEW-side fan-out runs after FK cascades so renamed/moved field definitions touch
-- the resources at their post-statement identity. The per-transaction resource guard collapses
-- OLD- and NEW-side touches into one externally visible revision.
CREATE OR REPLACE FUNCTION rustok_record_flex_attached_translation_schema_change_after()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    new_eligible BOOLEAN := FALSE;
    resource RECORD;
BEGIN
    IF TG_OP = 'UPDATE'
       AND OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
       AND OLD.entity_type IS NOT DISTINCT FROM NEW.entity_type
       AND OLD.field_key IS NOT DISTINCT FROM NEW.field_key
       AND OLD.field_type IS NOT DISTINCT FROM NEW.field_type
       AND OLD.is_localized IS NOT DISTINCT FROM NEW.is_localized
       AND OLD.is_required IS NOT DISTINCT FROM NEW.is_required
       AND OLD.validation::text IS NOT DISTINCT FROM NEW.validation::text
       AND OLD.is_active IS NOT DISTINCT FROM NEW.is_active THEN
        RETURN NEW;
    END IF;

    new_eligible := NEW.is_active
        AND NEW.is_localized
        AND NEW.field_type IN ('text', 'textarea');
    IF new_eligible THEN
        FOR resource IN
            SELECT DISTINCT value.tenant_id, value.entity_type, value.entity_id
            FROM flex_attached_localized_values value
            WHERE value.tenant_id = NEW.tenant_id
              AND value.entity_type = NEW.entity_type
              AND value.field_key = NEW.field_key
        LOOP
            PERFORM rustok_flex_bump_attached_translation_resource(
                resource.tenant_id,
                resource.entity_type,
                resource.entity_id
            );
        END LOOP;
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_flex_attached_translation_schema_change_after
AFTER INSERT OR UPDATE ON flex_attached_field_definitions
FOR EACH ROW EXECUTE FUNCTION rustok_record_flex_attached_translation_schema_change_after();
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
DROP TRIGGER IF EXISTS trg_flex_attached_translation_schema_change_after
    ON flex_attached_field_definitions;
DROP FUNCTION IF EXISTS rustok_record_flex_attached_translation_schema_change_after();
DROP TRIGGER IF EXISTS trg_flex_attached_translation_schema_change_before
    ON flex_attached_field_definitions;
DROP FUNCTION IF EXISTS rustok_record_flex_attached_translation_schema_change_before();
DROP TRIGGER IF EXISTS trg_flex_attached_translation_value_change
    ON flex_attached_localized_values;
DROP FUNCTION IF EXISTS rustok_record_flex_attached_translation_resource_change();
DROP FUNCTION IF EXISTS rustok_flex_attached_translation_field_eligible(UUID, TEXT, TEXT);
DROP FUNCTION IF EXISTS rustok_flex_bump_attached_translation_resource(UUID, TEXT, UUID);
DROP TABLE IF EXISTS flex_attached_translation_change_journal;
DROP TABLE IF EXISTS flex_attached_translation_resource_state;
"#,
            )
            .await?;
        Ok(())
    }
}
