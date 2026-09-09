use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

const STATE_TABLE: &str = "flex_attached_translation_resource_state";
const JOURNAL_TABLE: &str = "flex_attached_translation_change_journal";
const VALUES_TABLE: &str = "flex_attached_localized_values";
const DEFINITIONS_TABLE: &str = "flex_attached_field_definitions";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let backend = manager.get_database_backend();

        connection
            .execute_unprepared(&format!(
                r#"
CREATE TABLE IF NOT EXISTS {STATE_TABLE} (
    tenant_id UUID NOT NULL,
    entity_type VARCHAR(64) NOT NULL,
    entity_id UUID NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, entity_type, entity_id)
)
"#
            ))
            .await?;

        match backend {
            sea_orm_migration::sea_orm::DatabaseBackend::Postgres => {
                backfill_postgres(connection).await?;
                install_postgres_journal(connection).await?;
                install_postgres_triggers(connection).await?;
            }
            sea_orm_migration::sea_orm::DatabaseBackend::Sqlite => {
                backfill_sqlite(connection).await?;
                install_sqlite_triggers(connection).await?;
            }
            _ => {
                return Err(DbErr::Custom(
                    "Flex attached Translation revision state supports PostgreSQL and SQLite only"
                        .to_string(),
                ));
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        match manager.get_database_backend() {
            sea_orm_migration::sea_orm::DatabaseBackend::Postgres => {
                connection
                    .execute_unprepared(
                        r#"
DROP TRIGGER IF EXISTS trg_flex_attached_translation_definition_change ON flex_attached_field_definitions;
DROP TRIGGER IF EXISTS trg_flex_attached_translation_value_change ON flex_attached_localized_values;
DROP FUNCTION IF EXISTS flex_attached_translation_definition_changed();
DROP FUNCTION IF EXISTS flex_attached_translation_value_changed();
DROP FUNCTION IF EXISTS flex_touch_attached_translation_resource(UUID, TEXT, UUID);
DROP TABLE IF EXISTS flex_attached_translation_change_journal;
"#,
                    )
                    .await?;
            }
            sea_orm_migration::sea_orm::DatabaseBackend::Sqlite => {
                connection
                    .execute_unprepared(
                        r#"
DROP TRIGGER IF EXISTS trg_flex_attached_translation_definition_delete;
DROP TRIGGER IF EXISTS trg_flex_attached_translation_definition_update;
DROP TRIGGER IF EXISTS trg_flex_attached_translation_definition_insert;
DROP TRIGGER IF EXISTS trg_flex_attached_translation_value_delete;
DROP TRIGGER IF EXISTS trg_flex_attached_translation_value_update;
DROP TRIGGER IF EXISTS trg_flex_attached_translation_value_insert;
"#,
                    )
                    .await?;
            }
            _ => {}
        }
        connection
            .execute_unprepared(&format!("DROP TABLE IF EXISTS {STATE_TABLE}"))
            .await?;
        Ok(())
    }
}

async fn backfill_postgres<C>(connection: &C) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    connection
        .execute_unprepared(&format!(
            r#"
INSERT INTO {STATE_TABLE} (tenant_id, entity_type, entity_id, revision, updated_at)
SELECT DISTINCT v.tenant_id, v.entity_type, v.entity_id, 1, CURRENT_TIMESTAMP
FROM {VALUES_TABLE} v
JOIN {DEFINITIONS_TABLE} d
  ON d.tenant_id = v.tenant_id
 AND d.entity_type = v.entity_type
 AND d.field_key = v.field_key
WHERE d.is_active = TRUE
  AND d.is_localized = TRUE
  AND d.field_type IN ('text', 'textarea')
ON CONFLICT (tenant_id, entity_type, entity_id) DO NOTHING
"#
        ))
        .await?;
    Ok(())
}

async fn backfill_sqlite<C>(connection: &C) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    connection
        .execute_unprepared(&format!(
            r#"
INSERT OR IGNORE INTO {STATE_TABLE} (tenant_id, entity_type, entity_id, revision, updated_at)
SELECT DISTINCT v.tenant_id, v.entity_type, v.entity_id, 1, CURRENT_TIMESTAMP
FROM {VALUES_TABLE} v
JOIN {DEFINITIONS_TABLE} d
  ON d.tenant_id = v.tenant_id
 AND d.entity_type = v.entity_type
 AND d.field_key = v.field_key
WHERE d.is_active = 1
  AND d.is_localized = 1
  AND d.field_type IN ('text', 'textarea')
"#
        ))
        .await?;
    Ok(())
}

async fn install_postgres_journal<C>(connection: &C) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    connection
        .execute_unprepared(&format!(
            r#"
CREATE TABLE IF NOT EXISTS {JOURNAL_TABLE} (
    change_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    root_txid BIGINT NOT NULL,
    tenant_id UUID NOT NULL,
    entity_type VARCHAR(64) NOT NULL,
    entity_id UUID NOT NULL,
    resource_revision VARCHAR(256) NOT NULL,
    lifecycle VARCHAR(16) NOT NULL CHECK (lifecycle IN ('active', 'unavailable', 'deleted')),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_flex_attached_translation_change_tx
        UNIQUE (root_txid, tenant_id, entity_type, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_flex_attached_translation_change_tenant_seq
    ON {JOURNAL_TABLE} (tenant_id, entity_type, change_seq);
CREATE INDEX IF NOT EXISTS idx_flex_attached_translation_change_resource
    ON {JOURNAL_TABLE} (tenant_id, entity_type, entity_id, change_seq);
"#
        ))
        .await?;
    Ok(())
}

async fn install_postgres_triggers<C>(connection: &C) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    connection
        .execute_unprepared(&format!(
            r#"
CREATE OR REPLACE FUNCTION flex_touch_attached_translation_resource(
    p_tenant_id UUID,
    p_entity_type TEXT,
    p_entity_id UUID
) RETURNS BIGINT
LANGUAGE plpgsql
AS $$
DECLARE
    next_revision BIGINT;
    next_lifecycle TEXT;
BEGIN
    INSERT INTO {STATE_TABLE} (tenant_id, entity_type, entity_id, revision, updated_at)
    VALUES (p_tenant_id, p_entity_type, p_entity_id, 1, CURRENT_TIMESTAMP)
    ON CONFLICT (tenant_id, entity_type, entity_id)
    DO UPDATE SET
        revision = {STATE_TABLE}.revision + 1,
        updated_at = CURRENT_TIMESTAMP
    RETURNING revision INTO next_revision;

    SELECT CASE WHEN EXISTS (
        SELECT 1
        FROM {VALUES_TABLE} v
        JOIN {DEFINITIONS_TABLE} d
          ON d.tenant_id = v.tenant_id
         AND d.entity_type = v.entity_type
         AND d.field_key = v.field_key
        WHERE v.tenant_id = p_tenant_id
          AND v.entity_type = p_entity_type
          AND v.entity_id = p_entity_id
          AND d.is_active = TRUE
          AND d.is_localized = TRUE
          AND d.field_type IN ('text', 'textarea')
    ) THEN 'active' ELSE 'unavailable' END
    INTO next_lifecycle;

    INSERT INTO {JOURNAL_TABLE} (
        root_txid,
        tenant_id,
        entity_type,
        entity_id,
        resource_revision,
        lifecycle,
        created_at
    ) VALUES (
        txid_current(),
        p_tenant_id,
        p_entity_type,
        p_entity_id,
        'flex-attached-resource:' || next_revision::TEXT,
        next_lifecycle,
        CURRENT_TIMESTAMP
    )
    ON CONFLICT (root_txid, tenant_id, entity_type, entity_id)
    DO UPDATE SET
        resource_revision = EXCLUDED.resource_revision,
        lifecycle = EXCLUDED.lifecycle,
        created_at = CURRENT_TIMESTAMP;

    RETURN next_revision;
END;
$$;

CREATE OR REPLACE FUNCTION flex_attached_translation_value_changed()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    old_relevant BOOLEAN := FALSE;
    new_relevant BOOLEAN := FALSE;
    same_resource BOOLEAN := FALSE;
BEGIN
    IF TG_OP IN ('UPDATE', 'DELETE') THEN
        SELECT EXISTS (
            SELECT 1 FROM {DEFINITIONS_TABLE} d
            WHERE d.tenant_id = OLD.tenant_id
              AND d.entity_type = OLD.entity_type
              AND d.field_key = OLD.field_key
              AND d.is_active = TRUE
              AND d.is_localized = TRUE
              AND d.field_type IN ('text', 'textarea')
        ) INTO old_relevant;
    END IF;

    IF TG_OP IN ('INSERT', 'UPDATE') THEN
        SELECT EXISTS (
            SELECT 1 FROM {DEFINITIONS_TABLE} d
            WHERE d.tenant_id = NEW.tenant_id
              AND d.entity_type = NEW.entity_type
              AND d.field_key = NEW.field_key
              AND d.is_active = TRUE
              AND d.is_localized = TRUE
              AND d.field_type IN ('text', 'textarea')
        ) INTO new_relevant;
    END IF;

    IF TG_OP = 'INSERT' THEN
        IF new_relevant THEN
            PERFORM flex_touch_attached_translation_resource(
                NEW.tenant_id, NEW.entity_type, NEW.entity_id
            );
        END IF;
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        IF old_relevant THEN
            PERFORM flex_touch_attached_translation_resource(
                OLD.tenant_id, OLD.entity_type, OLD.entity_id
            );
        END IF;
        RETURN OLD;
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

    IF same_resource THEN
        IF old_relevant OR new_relevant THEN
            PERFORM flex_touch_attached_translation_resource(
                NEW.tenant_id, NEW.entity_type, NEW.entity_id
            );
        END IF;
    ELSE
        IF old_relevant THEN
            PERFORM flex_touch_attached_translation_resource(
                OLD.tenant_id, OLD.entity_type, OLD.entity_id
            );
        END IF;
        IF new_relevant THEN
            PERFORM flex_touch_attached_translation_resource(
                NEW.tenant_id, NEW.entity_type, NEW.entity_id
            );
        END IF;
    END IF;

    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_flex_attached_translation_value_change ON {VALUES_TABLE};
CREATE TRIGGER trg_flex_attached_translation_value_change
AFTER INSERT OR UPDATE OR DELETE ON {VALUES_TABLE}
FOR EACH ROW EXECUTE FUNCTION flex_attached_translation_value_changed();

CREATE OR REPLACE FUNCTION flex_attached_translation_definition_changed()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    affected RECORD;
    old_eligible BOOLEAN := FALSE;
    new_eligible BOOLEAN := FALSE;
    schema_changed BOOLEAN := TRUE;
BEGIN
    IF TG_OP IN ('UPDATE', 'DELETE') THEN
        old_eligible := OLD.is_active
            AND OLD.is_localized
            AND OLD.field_type IN ('text', 'textarea');
    END IF;
    IF TG_OP IN ('INSERT', 'UPDATE') THEN
        new_eligible := NEW.is_active
            AND NEW.is_localized
            AND NEW.field_type IN ('text', 'textarea');
    END IF;

    IF TG_OP = 'UPDATE' THEN
        schema_changed := OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
            OR OLD.entity_type IS DISTINCT FROM NEW.entity_type
            OR OLD.field_key IS DISTINCT FROM NEW.field_key
            OR OLD.field_type IS DISTINCT FROM NEW.field_type
            OR OLD.is_localized IS DISTINCT FROM NEW.is_localized
            OR OLD.is_required IS DISTINCT FROM NEW.is_required
            OR OLD.validation IS DISTINCT FROM NEW.validation
            OR OLD.is_active IS DISTINCT FROM NEW.is_active;
        IF NOT schema_changed THEN
            RETURN NEW;
        END IF;
    END IF;

    IF TG_OP = 'INSERT' THEN
        IF new_eligible THEN
            FOR affected IN
                SELECT DISTINCT v.tenant_id, v.entity_type, v.entity_id
                FROM {VALUES_TABLE} v
                WHERE v.tenant_id = NEW.tenant_id
                  AND v.entity_type = NEW.entity_type
                  AND v.field_key = NEW.field_key
            LOOP
                PERFORM flex_touch_attached_translation_resource(
                    affected.tenant_id, affected.entity_type, affected.entity_id
                );
            END LOOP;
        END IF;
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        IF old_eligible THEN
            FOR affected IN
                SELECT DISTINCT v.tenant_id, v.entity_type, v.entity_id
                FROM {VALUES_TABLE} v
                WHERE v.tenant_id = OLD.tenant_id
                  AND v.entity_type = OLD.entity_type
                  AND v.field_key = OLD.field_key
            LOOP
                PERFORM flex_touch_attached_translation_resource(
                    affected.tenant_id, affected.entity_type, affected.entity_id
                );
            END LOOP;
        END IF;
        RETURN OLD;
    END IF;

    FOR affected IN
        SELECT DISTINCT tenant_id, entity_type, entity_id
        FROM (
            SELECT v.tenant_id, v.entity_type, v.entity_id
            FROM {VALUES_TABLE} v
            WHERE old_eligible
              AND v.tenant_id = OLD.tenant_id
              AND v.entity_type = OLD.entity_type
              AND v.field_key = OLD.field_key
            UNION
            SELECT v.tenant_id, v.entity_type, v.entity_id
            FROM {VALUES_TABLE} v
            WHERE new_eligible
              AND v.tenant_id = NEW.tenant_id
              AND v.entity_type = NEW.entity_type
              AND v.field_key = NEW.field_key
        ) resources
    LOOP
        PERFORM flex_touch_attached_translation_resource(
            affected.tenant_id, affected.entity_type, affected.entity_id
        );
    END LOOP;

    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_flex_attached_translation_definition_change ON {DEFINITIONS_TABLE};
CREATE TRIGGER trg_flex_attached_translation_definition_change
AFTER INSERT OR UPDATE OR DELETE ON {DEFINITIONS_TABLE}
FOR EACH ROW EXECUTE FUNCTION flex_attached_translation_definition_changed();
"#
        ))
        .await?;
    Ok(())
}

async fn install_sqlite_triggers<C>(connection: &C) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    connection
        .execute_unprepared(&format!(
            r#"
CREATE TRIGGER IF NOT EXISTS trg_flex_attached_translation_value_insert
AFTER INSERT ON {VALUES_TABLE}
WHEN EXISTS (
    SELECT 1 FROM {DEFINITIONS_TABLE} d
    WHERE d.tenant_id = NEW.tenant_id
      AND d.entity_type = NEW.entity_type
      AND d.field_key = NEW.field_key
      AND d.is_active = 1
      AND d.is_localized = 1
      AND d.field_type IN ('text', 'textarea')
)
BEGIN
    INSERT INTO {STATE_TABLE} (tenant_id, entity_type, entity_id, revision, updated_at)
    VALUES (NEW.tenant_id, NEW.entity_type, NEW.entity_id, 1, CURRENT_TIMESTAMP)
    ON CONFLICT (tenant_id, entity_type, entity_id)
    DO UPDATE SET revision = revision + 1, updated_at = CURRENT_TIMESTAMP;
END;

CREATE TRIGGER IF NOT EXISTS trg_flex_attached_translation_value_delete
AFTER DELETE ON {VALUES_TABLE}
WHEN EXISTS (
    SELECT 1 FROM {DEFINITIONS_TABLE} d
    WHERE d.tenant_id = OLD.tenant_id
      AND d.entity_type = OLD.entity_type
      AND d.field_key = OLD.field_key
      AND d.is_active = 1
      AND d.is_localized = 1
      AND d.field_type IN ('text', 'textarea')
)
BEGIN
    INSERT INTO {STATE_TABLE} (tenant_id, entity_type, entity_id, revision, updated_at)
    VALUES (OLD.tenant_id, OLD.entity_type, OLD.entity_id, 1, CURRENT_TIMESTAMP)
    ON CONFLICT (tenant_id, entity_type, entity_id)
    DO UPDATE SET revision = revision + 1, updated_at = CURRENT_TIMESTAMP;
END;

CREATE TRIGGER IF NOT EXISTS trg_flex_attached_translation_value_update
AFTER UPDATE ON {VALUES_TABLE}
WHEN (
    OLD.tenant_id IS NOT NEW.tenant_id
    OR OLD.entity_type IS NOT NEW.entity_type
    OR OLD.entity_id IS NOT NEW.entity_id
    OR OLD.field_key IS NOT NEW.field_key
    OR OLD.locale IS NOT NEW.locale
    OR OLD.value IS NOT NEW.value
) AND EXISTS (
    SELECT 1 FROM {DEFINITIONS_TABLE} d
    WHERE d.tenant_id = NEW.tenant_id
      AND d.entity_type = NEW.entity_type
      AND d.field_key = NEW.field_key
      AND d.is_active = 1
      AND d.is_localized = 1
      AND d.field_type IN ('text', 'textarea')
)
BEGIN
    INSERT INTO {STATE_TABLE} (tenant_id, entity_type, entity_id, revision, updated_at)
    VALUES (NEW.tenant_id, NEW.entity_type, NEW.entity_id, 1, CURRENT_TIMESTAMP)
    ON CONFLICT (tenant_id, entity_type, entity_id)
    DO UPDATE SET revision = revision + 1, updated_at = CURRENT_TIMESTAMP;
END;

CREATE TRIGGER IF NOT EXISTS trg_flex_attached_translation_definition_insert
AFTER INSERT ON {DEFINITIONS_TABLE}
WHEN NEW.is_active = 1
 AND NEW.is_localized = 1
 AND NEW.field_type IN ('text', 'textarea')
BEGIN
    INSERT INTO {STATE_TABLE} (tenant_id, entity_type, entity_id, revision, updated_at)
    SELECT DISTINCT v.tenant_id, v.entity_type, v.entity_id, 1, CURRENT_TIMESTAMP
    FROM {VALUES_TABLE} v
    WHERE v.tenant_id = NEW.tenant_id
      AND v.entity_type = NEW.entity_type
      AND v.field_key = NEW.field_key
      AND 1 = 1
    ON CONFLICT (tenant_id, entity_type, entity_id)
    DO UPDATE SET revision = revision + 1, updated_at = CURRENT_TIMESTAMP;
END;

CREATE TRIGGER IF NOT EXISTS trg_flex_attached_translation_definition_update
AFTER UPDATE ON {DEFINITIONS_TABLE}
WHEN (
    OLD.tenant_id IS NOT NEW.tenant_id
    OR OLD.entity_type IS NOT NEW.entity_type
    OR OLD.field_key IS NOT NEW.field_key
    OR OLD.field_type IS NOT NEW.field_type
    OR OLD.is_localized IS NOT NEW.is_localized
    OR OLD.is_required IS NOT NEW.is_required
    OR OLD.validation IS NOT NEW.validation
    OR OLD.is_active IS NOT NEW.is_active
)
BEGIN
    INSERT INTO {STATE_TABLE} (tenant_id, entity_type, entity_id, revision, updated_at)
    SELECT DISTINCT v.tenant_id, v.entity_type, v.entity_id, 1, CURRENT_TIMESTAMP
    FROM {VALUES_TABLE} v
    WHERE (
        OLD.is_active = 1
        AND OLD.is_localized = 1
        AND OLD.field_type IN ('text', 'textarea')
        AND v.tenant_id = OLD.tenant_id
        AND v.entity_type = OLD.entity_type
        AND v.field_key = OLD.field_key
    ) OR (
        NEW.is_active = 1
        AND NEW.is_localized = 1
        AND NEW.field_type IN ('text', 'textarea')
        AND v.tenant_id = NEW.tenant_id
        AND v.entity_type = NEW.entity_type
        AND v.field_key = NEW.field_key
    )
    ON CONFLICT (tenant_id, entity_type, entity_id)
    DO UPDATE SET revision = revision + 1, updated_at = CURRENT_TIMESTAMP;
END;

CREATE TRIGGER IF NOT EXISTS trg_flex_attached_translation_definition_delete
AFTER DELETE ON {DEFINITIONS_TABLE}
WHEN OLD.is_active = 1
 AND OLD.is_localized = 1
 AND OLD.field_type IN ('text', 'textarea')
BEGIN
    INSERT INTO {STATE_TABLE} (tenant_id, entity_type, entity_id, revision, updated_at)
    SELECT DISTINCT v.tenant_id, v.entity_type, v.entity_id, 1, CURRENT_TIMESTAMP
    FROM {VALUES_TABLE} v
    WHERE v.tenant_id = OLD.tenant_id
      AND v.entity_type = OLD.entity_type
      AND v.field_key = OLD.field_key
      AND 1 = 1
    ON CONFLICT (tenant_id, entity_type, entity_id)
    DO UPDATE SET revision = revision + 1, updated_at = CURRENT_TIMESTAMP;
END;
"#
        ))
        .await?;
    Ok(())
}
