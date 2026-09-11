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

DROP TRIGGER IF EXISTS trg_flex_prune_standalone_field_policies ON flex_schemas;
CREATE TRIGGER trg_flex_prune_standalone_field_policies
AFTER UPDATE OF fields_config ON flex_schemas
FOR EACH ROW
WHEN (OLD.fields_config IS DISTINCT FROM NEW.fields_config)
EXECUTE FUNCTION rustok_flex_prune_standalone_field_policies();

-- Reconcile rows created before this migration so deployment starts from the same invariant.
DELETE FROM flex_standalone_field_policies AS policy
WHERE NOT EXISTS (
    SELECT 1
    FROM flex_schemas AS schema
    CROSS JOIN LATERAL jsonb_array_elements(
        COALESCE(schema.fields_config::jsonb, '[]'::jsonb)
    ) AS field
    WHERE schema.id = policy.schema_id
      AND schema.tenant_id = policy.tenant_id
      AND field ->> 'field_key' = policy.field_key
      AND COALESCE((field ->> 'is_active')::boolean, TRUE)
      AND COALESCE((field ->> 'is_localized')::boolean, FALSE)
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
DROP TRIGGER IF EXISTS trg_flex_prune_standalone_field_policies ON flex_schemas;
DROP FUNCTION IF EXISTS rustok_flex_prune_standalone_field_policies();
"#,
            )
            .await?;

        Ok(())
    }
}
