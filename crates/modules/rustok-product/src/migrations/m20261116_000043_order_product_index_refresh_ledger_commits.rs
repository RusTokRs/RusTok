use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        require_postgres(manager)?;
        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE FUNCTION rustok_product_allocate_index_refresh_sequence()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    -- Hold one lock per tenant across both streams until commit. Assigning a
    -- SERIAL default before acquiring this lock would let the relay skip rows.
    PERFORM pg_advisory_xact_lock(hashtextextended('product:index-refresh:' || NEW.tenant_id::text, 0));
    NEW.sequence_no := nextval(pg_get_serial_sequence(
        format('%I.%I', TG_TABLE_SCHEMA, TG_TABLE_NAME), 'sequence_no'
    )::regclass);
    RETURN NEW;
END;
$$;

ALTER TABLE product_index_locale_refresh_ledger ALTER COLUMN sequence_no DROP DEFAULT;
ALTER TABLE product_variant_index_refresh_ledger ALTER COLUMN sequence_no DROP DEFAULT;

CREATE TRIGGER trg_product_index_refresh_sequence
BEFORE INSERT ON product_index_locale_refresh_ledger
FOR EACH ROW EXECUTE FUNCTION rustok_product_allocate_index_refresh_sequence();

CREATE TRIGGER trg_product_index_refresh_sequence
BEFORE INSERT ON product_variant_index_refresh_ledger
FOR EACH ROW EXECUTE FUNCTION rustok_product_allocate_index_refresh_sequence();
"#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        require_postgres(manager)?;
        manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP TRIGGER trg_product_index_refresh_sequence ON product_index_locale_refresh_ledger;
DROP TRIGGER trg_product_index_refresh_sequence ON product_variant_index_refresh_ledger;

ALTER TABLE product_index_locale_refresh_ledger ALTER COLUMN sequence_no
    SET DEFAULT nextval('product_index_locale_refresh_ledger_sequence_no_seq'::regclass);
ALTER TABLE product_variant_index_refresh_ledger ALTER COLUMN sequence_no
    SET DEFAULT nextval('product_variant_index_refresh_ledger_sequence_no_seq'::regclass);

DROP FUNCTION rustok_product_allocate_index_refresh_sequence();
"#,
            )
            .await?;
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
