use sea_orm_migration::{prelude::*, sea_orm::DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        require_postgres(manager)?;
        manager.get_connection().execute_unprepared(r#"
CREATE FUNCTION rustok_product_allocate_channel_convergence_sequence()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended('product:channel-convergence:' || NEW.tenant_id::text, 0));
    NEW.sequence_no := nextval(pg_get_serial_sequence(
        format('%I.%I', TG_TABLE_SCHEMA, TG_TABLE_NAME), 'sequence_no'
    )::regclass);
    RETURN NEW;
END;
$$;

ALTER TABLE product_sales_channel_index_relation_convergence_requests
    ALTER COLUMN sequence_no DROP DEFAULT;

CREATE TRIGGER trg_product_channel_convergence_sequence
BEFORE INSERT ON product_sales_channel_index_relation_convergence_requests
FOR EACH ROW EXECUTE FUNCTION rustok_product_allocate_channel_convergence_sequence();

ALTER TABLE product_sales_channel_index_relation_convergence_state
    ADD COLUMN sweep_required BOOLEAN NOT NULL DEFAULT FALSE;

-- The old cursor could have advanced past a late commit. Resume any active sweep,
-- then check the tenant from the beginning without rewinding the visibility cursor.
UPDATE product_sales_channel_index_relation_convergence_state
SET sweep_required = TRUE
WHERE visibility_cursor > 0;
"#).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        require_postgres(manager)?;
        manager.get_connection().execute_unprepared(r#"
DROP TRIGGER trg_product_channel_convergence_sequence ON product_sales_channel_index_relation_convergence_requests;
ALTER TABLE product_sales_channel_index_relation_convergence_requests
    ALTER COLUMN sequence_no
    SET DEFAULT nextval(pg_get_serial_sequence(
        'product_sales_channel_index_relation_convergence_requests', 'sequence_no'
    )::regclass);
DROP FUNCTION rustok_product_allocate_channel_convergence_sequence();
ALTER TABLE product_sales_channel_index_relation_convergence_state DROP COLUMN sweep_required;
"#).await?;
        Ok(())
    }
}

fn require_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if manager.get_database_backend() != DatabaseBackend::Postgres {
        return Err(DbErr::Custom(
            "rustok-product migrations require PostgreSQL".to_owned(),
        ));
    }
    Ok(())
}
