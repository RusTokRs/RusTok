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
-- Freeze both sides of the invariant while the helper, triggers, and existing rows are reconciled.
LOCK TABLE flex_schemas IN SHARE ROW EXCLUSIVE MODE;
LOCK TABLE flex_standalone_field_policies IN SHARE ROW EXCLUSIVE MODE;

-- Keep the database predicate aligned with flex_standalone_translation_field_eligible():
-- active + localized + a translatable textual field type.
CREATE OR REPLACE FUNCTION rustok_flex_standalone_field_is_translation_target(
    fields_config JSONB,
    candidate_field_key TEXT
)
RETURNS BOOLEAN
LANGUAGE sql
IMMUTABLE
AS $$
    SELECT EXISTS (
        SELECT 1
        FROM jsonb_array_elements(COALESCE(fields_config, '[]'::jsonb)) AS field
        WHERE field ->> 'field_key' = candidate_field_key
          AND COALESCE((field ->> 'is_active')::boolean, TRUE)
          AND COALESCE((field ->> 'is_localized')::boolean, FALSE)
          AND field ->> 'field_type' IN ('text', 'textarea')
    );
$$;

-- Schema mutations are the authoritative lifecycle boundary for policy rows. Replacing the
-- previous function also closes the field-type transition gap (for example text -> json).
CREATE OR REPLACE FUNCTION rustok_flex_prune_standalone_field_policies()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    DELETE FROM flex_standalone_field_policies AS policy
    WHERE policy.schema_id = NEW.id
      AND (
          policy.tenant_id <> NEW.tenant_id
          OR NOT rustok_flex_standalone_field_is_translation_target(
              NEW.fields_config::jsonb,
              policy.field_key
          )
      );

    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_flex_prune_standalone_field_policies ON flex_schemas;
CREATE TRIGGER trg_flex_prune_standalone_field_policies
AFTER UPDATE OF fields_config, tenant_id ON flex_schemas
FOR EACH ROW
WHEN (
    OLD.fields_config IS DISTINCT FROM NEW.fields_config
    OR OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
)
EXECUTE FUNCTION rustok_flex_prune_standalone_field_policies();

-- Policy writes must prove both ownership and current translation-target eligibility at the
-- database boundary. FOR SHARE serializes these writes with concurrent schema mutations.
CREATE OR REPLACE FUNCTION rustok_flex_validate_standalone_field_policy_target()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    schema_fields_config JSONB;
BEGIN
    SELECT flex_schema.fields_config::jsonb
      INTO schema_fields_config
      FROM flex_schemas AS flex_schema
     WHERE flex_schema.id = NEW.schema_id
       AND flex_schema.tenant_id = NEW.tenant_id
     FOR SHARE;

    IF NOT FOUND THEN
        RAISE EXCEPTION USING
            ERRCODE = '23514',
            MESSAGE = format(
                'standalone field policy schema %s does not belong to tenant %s',
                NEW.schema_id,
                NEW.tenant_id
            );
    END IF;

    IF NOT rustok_flex_standalone_field_is_translation_target(
        schema_fields_config,
        NEW.field_key
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = '23514',
            MESSAGE = format(
                'standalone field policy target %s/%s is not an active localized text field',
                NEW.schema_id,
                NEW.field_key
            );
    END IF;

    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_flex_validate_standalone_field_policy_target
    ON flex_standalone_field_policies;
CREATE TRIGGER trg_flex_validate_standalone_field_policy_target
BEFORE INSERT OR UPDATE ON flex_standalone_field_policies
FOR EACH ROW
EXECUTE FUNCTION rustok_flex_validate_standalone_field_policy_target();

-- Reconcile rows that predate the write guard, including tenant mismatches and fields whose
-- type changed away from text/textarea.
DELETE FROM flex_standalone_field_policies AS policy
WHERE NOT EXISTS (
    SELECT 1
    FROM flex_schemas AS flex_schema
    WHERE flex_schema.id = policy.schema_id
      AND flex_schema.tenant_id = policy.tenant_id
      AND rustok_flex_standalone_field_is_translation_target(
          flex_schema.fields_config::jsonb,
          policy.field_key
      )
);
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
LOCK TABLE flex_schemas IN SHARE ROW EXCLUSIVE MODE;
LOCK TABLE flex_standalone_field_policies IN SHARE ROW EXCLUSIVE MODE;

DROP TRIGGER IF EXISTS trg_flex_validate_standalone_field_policy_target
    ON flex_standalone_field_policies;
DROP FUNCTION IF EXISTS rustok_flex_validate_standalone_field_policy_target();

-- Restore the lifecycle trigger installed by m20260911_000007.
DROP TRIGGER IF EXISTS trg_flex_prune_standalone_field_policies ON flex_schemas;
CREATE OR REPLACE FUNCTION rustok_flex_prune_standalone_field_policies()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    DELETE FROM flex_standalone_field_policies AS policy
    WHERE policy.tenant_id = NEW.tenant_id
      AND policy.schema_id = NEW.id
      AND NOT EXISTS (
          SELECT 1
          FROM jsonb_array_elements(COALESCE(NEW.fields_config::jsonb, '[]'::jsonb)) AS field
          WHERE field ->> 'field_key' = policy.field_key
            AND COALESCE((field ->> 'is_active')::boolean, TRUE)
            AND COALESCE((field ->> 'is_localized')::boolean, FALSE)
      );

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_flex_prune_standalone_field_policies
AFTER UPDATE OF fields_config ON flex_schemas
FOR EACH ROW
WHEN (OLD.fields_config IS DISTINCT FROM NEW.fields_config)
EXECUTE FUNCTION rustok_flex_prune_standalone_field_policies();

DROP FUNCTION IF EXISTS rustok_flex_standalone_field_is_translation_target(JSONB, TEXT);
"#,
            )
            .await?;

        Ok(())
    }
}
