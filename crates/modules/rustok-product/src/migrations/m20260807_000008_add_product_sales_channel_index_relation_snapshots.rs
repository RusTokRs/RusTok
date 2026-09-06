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
CREATE OR REPLACE FUNCTION rustok_product_validate_channel_relation_ids(value JSONB)
RETURNS BOOLEAN
LANGUAGE plpgsql
IMMUTABLE
AS $$
DECLARE
    item JSONB;
    item_text TEXT;
    item_uuid UUID;
    previous_uuid UUID;
BEGIN
    IF jsonb_typeof(value) <> 'array' OR jsonb_array_length(value) > 1024 THEN
        RETURN FALSE;
    END IF;

    FOR item IN
        SELECT element
        FROM jsonb_array_elements(value) AS elements(element)
    LOOP
        IF jsonb_typeof(item) <> 'string' THEN
            RETURN FALSE;
        END IF;
        item_text := item #>> '{}';
        BEGIN
            item_uuid := item_text::uuid;
        EXCEPTION WHEN invalid_text_representation THEN
            RETURN FALSE;
        END;
        IF item_uuid = '00000000-0000-0000-0000-000000000000'::uuid
            OR item_text <> item_uuid::text
        THEN
            RETURN FALSE;
        END IF;
        IF previous_uuid IS NOT NULL AND item_uuid <= previous_uuid THEN
            RETURN FALSE;
        END IF;
        previous_uuid := item_uuid;
    END LOOP;

    RETURN TRUE;
END;
$$;

CREATE TABLE product_sales_channel_index_relation_snapshots (
    sequence_no BIGSERIAL NOT NULL,
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    relation_epoch BIGINT NOT NULL,
    channel_ids JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_product_sales_channel_index_relation_snapshots
        PRIMARY KEY (tenant_id, product_id, relation_epoch),
    CONSTRAINT uq_product_sales_channel_index_relation_tenant_sequence
        UNIQUE (tenant_id, sequence_no),
    CONSTRAINT chk_product_sales_channel_index_relation_sequence_positive
        CHECK (sequence_no > 0),
    CONSTRAINT chk_product_sales_channel_index_relation_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_sales_channel_index_relation_product_non_nil
        CHECK (product_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_sales_channel_index_relation_epoch_positive
        CHECK (relation_epoch > 0),
    CONSTRAINT chk_product_sales_channel_index_relation_ids_canonical
        CHECK (rustok_product_validate_channel_relation_ids(channel_ids))
);

CREATE INDEX idx_product_sales_channel_index_relation_current
    ON product_sales_channel_index_relation_snapshots (
        tenant_id,
        product_id,
        relation_epoch DESC
    );

CREATE OR REPLACE FUNCTION rustok_product_guard_channel_relation_snapshot()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    previous_epoch BIGINT;
    previous_channel_ids JSONB;
    lock_key TEXT;
BEGIN
    lock_key := NEW.tenant_id::text
        || E'\x1f' || NEW.product_id::text
        || E'\x1fproduct-sales-channel-index-relation';
    PERFORM pg_advisory_xact_lock(hashtextextended(lock_key, 0));

    SELECT relation_epoch, channel_ids
      INTO previous_epoch, previous_channel_ids
      FROM product_sales_channel_index_relation_snapshots
     WHERE tenant_id = NEW.tenant_id
       AND product_id = NEW.product_id
     ORDER BY relation_epoch DESC
     LIMIT 1;

    IF previous_epoch IS NULL THEN
        IF NEW.relation_epoch <> 1 THEN
            RAISE EXCEPTION 'first Product-SalesChannel relation epoch must equal 1';
        END IF;
        RETURN NEW;
    END IF;

    IF previous_epoch = 9223372036854775807 THEN
        RAISE EXCEPTION 'Product-SalesChannel relation epoch is exhausted';
    END IF;
    IF NEW.relation_epoch <> previous_epoch + 1 THEN
        RAISE EXCEPTION 'Product-SalesChannel relation epoch must advance exactly once';
    END IF;
    IF NEW.channel_ids = previous_channel_ids THEN
        RAISE EXCEPTION 'unchanged Product-SalesChannel membership must not append a new epoch';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_product_channel_relation_snapshot_insert
BEFORE INSERT ON product_sales_channel_index_relation_snapshots
FOR EACH ROW
EXECUTE FUNCTION rustok_product_guard_channel_relation_snapshot();

CREATE OR REPLACE FUNCTION rustok_product_reject_channel_relation_snapshot_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'Product-SalesChannel relation snapshots are append-only';
    RETURN NULL;
END;
$$;

CREATE TRIGGER trg_product_channel_relation_snapshot_update
BEFORE UPDATE ON product_sales_channel_index_relation_snapshots
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_channel_relation_snapshot_mutation();

CREATE TRIGGER trg_product_channel_relation_snapshot_delete
BEFORE DELETE ON product_sales_channel_index_relation_snapshots
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_channel_relation_snapshot_mutation();

CREATE OR REPLACE FUNCTION rustok_product_retain_empty_channel_relation_on_delete()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    previous_epoch BIGINT;
    previous_channel_ids JSONB;
    lock_key TEXT;
BEGIN
    lock_key := OLD.tenant_id::text
        || E'\x1f' || OLD.id::text
        || E'\x1fproduct-sales-channel-index-relation';
    PERFORM pg_advisory_xact_lock(hashtextextended(lock_key, 0));

    SELECT relation_epoch, channel_ids
      INTO previous_epoch, previous_channel_ids
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
            channel_ids
        ) VALUES (
            OLD.tenant_id,
            OLD.id,
            previous_epoch + 1,
            '[]'::jsonb
        );
    END IF;

    RETURN OLD;
END;
$$;

CREATE TRIGGER trg_products_retain_empty_channel_relation
AFTER DELETE ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_retain_empty_channel_relation_on_delete();
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product migrations require PostgreSQL".to_owned(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP TRIGGER IF EXISTS trg_products_retain_empty_channel_relation ON products;
DROP TRIGGER IF EXISTS trg_product_channel_relation_snapshot_delete
    ON product_sales_channel_index_relation_snapshots;
DROP TRIGGER IF EXISTS trg_product_channel_relation_snapshot_update
    ON product_sales_channel_index_relation_snapshots;
DROP TRIGGER IF EXISTS trg_product_channel_relation_snapshot_insert
    ON product_sales_channel_index_relation_snapshots;
DROP TABLE IF EXISTS product_sales_channel_index_relation_snapshots;
DROP FUNCTION IF EXISTS rustok_product_retain_empty_channel_relation_on_delete();
DROP FUNCTION IF EXISTS rustok_product_reject_channel_relation_snapshot_mutation();
DROP FUNCTION IF EXISTS rustok_product_guard_channel_relation_snapshot();
DROP FUNCTION IF EXISTS rustok_product_validate_channel_relation_ids(JSONB);
"#,
            )
            .await?;

        Ok(())
    }
}
