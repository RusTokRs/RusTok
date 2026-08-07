use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product migrations require PostgreSQL".to_owned(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE product_sales_channel_index_relation_snapshots
    ADD COLUMN visibility_key TEXT,
    ADD COLUMN channel_identity_generation BIGINT;

-- Existing relation snapshots predate freshness evidence. Keep them as replay history but mark them
-- explicitly stale so no canonical live Product record can treat them as a current witness.
UPDATE product_sales_channel_index_relation_snapshots
   SET visibility_key = 'legacy-stale',
       channel_identity_generation = 0
 WHERE visibility_key IS NULL
    OR channel_identity_generation IS NULL;

ALTER TABLE product_sales_channel_index_relation_snapshots
    ALTER COLUMN visibility_key SET NOT NULL,
    ALTER COLUMN channel_identity_generation SET NOT NULL,
    ADD CONSTRAINT chk_product_sales_channel_index_relation_visibility_key
        CHECK (char_length(visibility_key) BETWEEN 1 AND 131072),
    ADD CONSTRAINT chk_product_sales_channel_index_relation_channel_generation
        CHECK (channel_identity_generation >= 0);

CREATE OR REPLACE FUNCTION rustok_product_guard_channel_relation_snapshot()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    previous_epoch BIGINT;
    previous_channel_ids JSONB;
    previous_visibility_key TEXT;
    previous_channel_identity_generation BIGINT;
    lock_key TEXT;
BEGIN
    lock_key := NEW.tenant_id::text
        || E'\x1f' || NEW.product_id::text
        || E'\x1fproduct-sales-channel-index-relation';
    PERFORM pg_advisory_xact_lock(hashtextextended(lock_key, 0));

    SELECT
        relation_epoch,
        channel_ids,
        visibility_key,
        channel_identity_generation
      INTO
        previous_epoch,
        previous_channel_ids,
        previous_visibility_key,
        previous_channel_identity_generation
      FROM product_sales_channel_index_relation_snapshots
     WHERE tenant_id = NEW.tenant_id
       AND product_id = NEW.product_id
     ORDER BY relation_epoch DESC
     LIMIT 1;

    IF previous_epoch IS NULL THEN
        IF NEW.relation_epoch <> 1 THEN
            RAISE EXCEPTION 'first Product-SalesChannel relation epoch must equal 1';
        END IF;
        IF NEW.visibility_key = 'legacy-stale' THEN
            RAISE EXCEPTION 'new Product-SalesChannel relation snapshots require observed freshness';
        END IF;
        RETURN NEW;
    END IF;

    IF previous_epoch = 9223372036854775807 THEN
        RAISE EXCEPTION 'Product-SalesChannel relation epoch is exhausted';
    END IF;
    IF NEW.relation_epoch <> previous_epoch + 1 THEN
        RAISE EXCEPTION 'Product-SalesChannel relation epoch must advance exactly once';
    END IF;
    IF NEW.visibility_key = 'legacy-stale'
       AND NOT (
           NEW.channel_ids = '[]'::jsonb
           AND previous_visibility_key = 'legacy-stale'
       )
    THEN
        RAISE EXCEPTION 'new Product-SalesChannel relation snapshots require observed freshness';
    END IF;
    IF NEW.channel_ids = previous_channel_ids
       AND NEW.visibility_key = previous_visibility_key
       AND NEW.channel_identity_generation = previous_channel_identity_generation
    THEN
        RAISE EXCEPTION 'unchanged Product-SalesChannel relation witness must not append a new epoch';
    END IF;

    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_product_retain_empty_channel_relation_on_delete()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    previous_epoch BIGINT;
    previous_channel_ids JSONB;
    previous_visibility_key TEXT;
    previous_channel_identity_generation BIGINT;
    lock_key TEXT;
BEGIN
    lock_key := OLD.tenant_id::text
        || E'\x1f' || OLD.id::text
        || E'\x1fproduct-sales-channel-index-relation';
    PERFORM pg_advisory_xact_lock(hashtextextended(lock_key, 0));

    SELECT
        relation_epoch,
        channel_ids,
        visibility_key,
        channel_identity_generation
      INTO
        previous_epoch,
        previous_channel_ids,
        previous_visibility_key,
        previous_channel_identity_generation
      FROM product_sales_channel_index_relation_snapshots
     WHERE tenant_id = OLD.tenant_id
       AND product_id = OLD.id
     ORDER BY relation_epoch DESC
     LIMIT 1;

    IF previous_epoch IS NOT NULL AND previous_channel_ids <> '[]'::jsonb THEN
        IF previous_epoch = 9223372036854775807 THEN
            RAISE EXCEPTION 'Product-SalesChannel relation epoch is exhausted';
        END IF;
        INSERT INTO product_sales_channel_index_relation_snapshots (
            tenant_id,
            product_id,
            relation_epoch,
            channel_ids,
            visibility_key,
            channel_identity_generation
        ) VALUES (
            OLD.tenant_id,
            OLD.id,
            previous_epoch + 1,
            '[]'::jsonb,
            previous_visibility_key,
            previous_channel_identity_generation
        );
    END IF;

    RETURN OLD;
END;
$$;
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "Product-SalesChannel relation freshness evidence migration is intentionally irreversible"
                .to_owned(),
        ))
    }
}
