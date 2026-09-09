use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DatabaseBackend},
};

const JOURNAL_TABLE: &str = "flex_attached_translation_change_journal";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ChangeCursor is a PostgreSQL production capability. SQLite remains useful for
        // portable owner tests, but must not pretend to provide durable ordered changes.
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE TABLE flex_attached_translation_change_journal (
    change_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tenant_id UUID NOT NULL,
    entity_type VARCHAR(64) NOT NULL,
    entity_id UUID NULL,
    change_kind VARCHAR(24) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT chk_flex_attached_translation_change_seq_positive
        CHECK (change_seq > 0),
    CONSTRAINT chk_flex_attached_translation_change_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_attached_translation_change_entity_type_nonblank
        CHECK (length(trim(entity_type)) > 0),
    CONSTRAINT chk_flex_attached_translation_change_entity_non_nil
        CHECK (entity_id IS NULL OR entity_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_attached_translation_change_kind
        CHECK (change_kind IN ('resource_changed', 'resource_deleted', 'schema_changed')),
    CONSTRAINT chk_flex_attached_translation_change_shape
        CHECK (
            (change_kind = 'schema_changed' AND entity_id IS NULL)
            OR
            (change_kind IN ('resource_changed', 'resource_deleted') AND entity_id IS NOT NULL)
        )
);

CREATE INDEX idx_flex_attached_translation_change_tenant_seq
    ON flex_attached_translation_change_journal (tenant_id, change_seq);

CREATE INDEX idx_flex_attached_translation_change_scope_seq
    ON flex_attached_translation_change_journal (tenant_id, entity_type, change_seq);

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
            INSERT INTO flex_attached_translation_change_journal (
                tenant_id, entity_type, entity_id, change_kind
            ) VALUES (
                NEW.tenant_id, NEW.entity_type, NEW.entity_id, 'resource_changed'
            );
        END IF;
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        IF old_relevant THEN
            INSERT INTO flex_attached_translation_change_journal (
                tenant_id, entity_type, entity_id, change_kind
            ) VALUES (
                OLD.tenant_id, OLD.entity_type, OLD.entity_id, 'resource_changed'
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
       AND OLD.value IS NOT DISTINCT FROM NEW.value THEN
        RETURN NEW;
    END IF;

    same_resource := OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
        AND OLD.entity_type IS NOT DISTINCT FROM NEW.entity_type
        AND OLD.entity_id IS NOT DISTINCT FROM NEW.entity_id;

    IF old_relevant THEN
        INSERT INTO flex_attached_translation_change_journal (
            tenant_id, entity_type, entity_id, change_kind
        ) VALUES (
            OLD.tenant_id, OLD.entity_type, OLD.entity_id, 'resource_changed'
        );
    END IF;

    IF new_relevant AND (NOT same_resource OR NOT old_relevant) THEN
        INSERT INTO flex_attached_translation_change_journal (
            tenant_id, entity_type, entity_id, change_kind
        ) VALUES (
            NEW.tenant_id, NEW.entity_type, NEW.entity_id, 'resource_changed'
        );
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_flex_attached_translation_value_change
AFTER INSERT OR UPDATE OR DELETE ON flex_attached_localized_values
FOR EACH ROW EXECUTE FUNCTION rustok_record_flex_attached_translation_resource_change();

CREATE OR REPLACE FUNCTION rustok_record_flex_attached_translation_schema_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    old_eligible BOOLEAN := FALSE;
    new_eligible BOOLEAN := FALSE;
    old_scope_changed BOOLEAN := FALSE;
BEGIN
    IF TG_OP <> 'INSERT' THEN
        old_eligible := OLD.is_active
            AND OLD.is_localized
            AND OLD.field_type IN ('text', 'textarea');
    END IF;
    IF TG_OP <> 'DELETE' THEN
        new_eligible := NEW.is_active
            AND NEW.is_localized
            AND NEW.field_type IN ('text', 'textarea');
    END IF;

    IF TG_OP = 'INSERT' THEN
        IF new_eligible THEN
            INSERT INTO flex_attached_translation_change_journal (
                tenant_id, entity_type, entity_id, change_kind
            ) VALUES (
                NEW.tenant_id, NEW.entity_type, NULL, 'schema_changed'
            );
        END IF;
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        IF old_eligible THEN
            INSERT INTO flex_attached_translation_change_journal (
                tenant_id, entity_type, entity_id, change_kind
            ) VALUES (
                OLD.tenant_id, OLD.entity_type, NULL, 'schema_changed'
            );
        END IF;
        RETURN OLD;
    END IF;

    IF NOT old_eligible AND NOT new_eligible THEN
        RETURN NEW;
    END IF;

    IF OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
       AND OLD.entity_type IS NOT DISTINCT FROM NEW.entity_type
       AND OLD.field_key IS NOT DISTINCT FROM NEW.field_key
       AND OLD.field_type IS NOT DISTINCT FROM NEW.field_type
       AND OLD.is_localized IS NOT DISTINCT FROM NEW.is_localized
       AND OLD.is_required IS NOT DISTINCT FROM NEW.is_required
       AND OLD.validation IS NOT DISTINCT FROM NEW.validation
       AND OLD.is_active IS NOT DISTINCT FROM NEW.is_active THEN
        RETURN NEW;
    END IF;

    old_scope_changed := OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.entity_type IS DISTINCT FROM NEW.entity_type;

    IF old_eligible THEN
        INSERT INTO flex_attached_translation_change_journal (
            tenant_id, entity_type, entity_id, change_kind
        ) VALUES (
            OLD.tenant_id, OLD.entity_type, NULL, 'schema_changed'
        );
    END IF;

    IF new_eligible AND (old_scope_changed OR NOT old_eligible) THEN
        INSERT INTO flex_attached_translation_change_journal (
            tenant_id, entity_type, entity_id, change_kind
        ) VALUES (
            NEW.tenant_id, NEW.entity_type, NULL, 'schema_changed'
        );
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_flex_attached_translation_schema_change
AFTER INSERT OR UPDATE OR DELETE ON flex_attached_field_definitions
FOR EACH ROW EXECUTE FUNCTION rustok_record_flex_attached_translation_schema_change();
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
DROP TRIGGER IF EXISTS trg_flex_attached_translation_schema_change
    ON flex_attached_field_definitions;
DROP FUNCTION IF EXISTS rustok_record_flex_attached_translation_schema_change();
DROP TRIGGER IF EXISTS trg_flex_attached_translation_value_change
    ON flex_attached_localized_values;
DROP FUNCTION IF EXISTS rustok_record_flex_attached_translation_resource_change();
DROP FUNCTION IF EXISTS rustok_flex_attached_translation_field_eligible(UUID, TEXT, TEXT);
DROP TABLE IF EXISTS flex_attached_translation_change_journal;
"#,
            )
            .await?;
        Ok(())
    }
}
