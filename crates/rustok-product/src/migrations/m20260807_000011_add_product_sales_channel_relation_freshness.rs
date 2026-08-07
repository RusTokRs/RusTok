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
CREATE TABLE product_sales_channel_index_relation_freshness_snapshots (
    sequence_no BIGSERIAL NOT NULL,
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    relation_epoch BIGINT NOT NULL,
    product_source_version BIGINT NOT NULL,
    visibility_key TEXT NOT NULL,
    channel_identity_generation BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_product_sales_channel_index_relation_freshness_snapshots
        PRIMARY KEY (tenant_id, product_id, sequence_no),
    CONSTRAINT uq_product_sales_channel_index_relation_freshness_tenant_sequence
        UNIQUE (tenant_id, sequence_no),
    CONSTRAINT fk_product_sales_channel_index_relation_freshness_relation
        FOREIGN KEY (tenant_id, product_id, relation_epoch)
        REFERENCES product_sales_channel_index_relation_snapshots (
            tenant_id,
            product_id,
            relation_epoch
        ),
    CONSTRAINT chk_product_sales_channel_index_relation_freshness_sequence_positive
        CHECK (sequence_no > 0),
    CONSTRAINT chk_product_sales_channel_index_relation_freshness_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_sales_channel_index_relation_freshness_product_non_nil
        CHECK (product_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_sales_channel_index_relation_freshness_relation_epoch_positive
        CHECK (relation_epoch > 0),
    CONSTRAINT chk_product_sales_channel_index_relation_freshness_product_source_positive
        CHECK (product_source_version > 0),
    CONSTRAINT chk_product_sales_channel_index_relation_freshness_visibility_key
        CHECK (octet_length(visibility_key) BETWEEN 1 AND 131072),
    CONSTRAINT chk_product_sales_channel_index_relation_freshness_channel_generation
        CHECK (channel_identity_generation >= 0)
);

CREATE INDEX idx_product_sales_channel_index_relation_freshness_current
    ON product_sales_channel_index_relation_freshness_snapshots (
        tenant_id,
        product_id,
        sequence_no DESC
    );

CREATE INDEX idx_product_sales_channel_index_relation_freshness_relation_current
    ON product_sales_channel_index_relation_freshness_snapshots (
        tenant_id,
        product_id,
        relation_epoch,
        sequence_no DESC
    );

CREATE OR REPLACE FUNCTION rustok_product_guard_channel_relation_freshness_snapshot()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    current_relation_epoch BIGINT;
    previous_relation_epoch BIGINT;
    previous_product_source_version BIGINT;
    previous_visibility_key TEXT;
    previous_channel_identity_generation BIGINT;
    relation_lock_key TEXT;
    freshness_lock_key TEXT;
BEGIN
    -- Match the owner service lock order exactly: live Product row -> relation advisory lock ->
    -- freshness advisory lock. This fences deletion and prevents a witness from being committed for
    -- an epoch concurrently superseded by another relation writer.
    PERFORM 1
      FROM products
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.product_id
     FOR KEY SHARE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Product-SalesChannel freshness witness requires a live Product';
    END IF;

    relation_lock_key := NEW.tenant_id::text
        || E'\x1f' || NEW.product_id::text
        || E'\x1fproduct-sales-channel-index-relation';
    PERFORM pg_advisory_xact_lock(hashtextextended(relation_lock_key, 0));

    freshness_lock_key := NEW.tenant_id::text
        || E'\x1f' || NEW.product_id::text
        || E'\x1fproduct-sales-channel-index-relation-freshness';
    PERFORM pg_advisory_xact_lock(hashtextextended(freshness_lock_key, 0));

    SELECT relation_epoch
      INTO current_relation_epoch
      FROM product_sales_channel_index_relation_snapshots
     WHERE tenant_id = NEW.tenant_id
       AND product_id = NEW.product_id
     ORDER BY relation_epoch DESC
     LIMIT 1;

    IF current_relation_epoch IS NULL OR NEW.relation_epoch <> current_relation_epoch THEN
        RAISE EXCEPTION 'Product-SalesChannel freshness witness requires the current relation epoch';
    END IF;

    SELECT
        relation_epoch,
        product_source_version,
        visibility_key,
        channel_identity_generation
      INTO
        previous_relation_epoch,
        previous_product_source_version,
        previous_visibility_key,
        previous_channel_identity_generation
      FROM product_sales_channel_index_relation_freshness_snapshots
     WHERE tenant_id = NEW.tenant_id
       AND product_id = NEW.product_id
     ORDER BY sequence_no DESC
     LIMIT 1;

    IF previous_relation_epoch IS NULL THEN
        RETURN NEW;
    END IF;

    IF NEW.relation_epoch < previous_relation_epoch THEN
        RAISE EXCEPTION 'Product-SalesChannel freshness relation epoch regressed';
    END IF;
    IF NEW.product_source_version < previous_product_source_version THEN
        RAISE EXCEPTION 'Product-SalesChannel freshness Product watermark regressed';
    END IF;
    IF NEW.channel_identity_generation < previous_channel_identity_generation THEN
        RAISE EXCEPTION 'Product-SalesChannel freshness Channel watermark regressed';
    END IF;
    IF NEW.relation_epoch = previous_relation_epoch
       AND NEW.product_source_version = previous_product_source_version
       AND NEW.visibility_key = previous_visibility_key
       AND NEW.channel_identity_generation = previous_channel_identity_generation
    THEN
        RAISE EXCEPTION 'unchanged Product-SalesChannel freshness witness must not append';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_product_channel_relation_freshness_snapshot_insert
BEFORE INSERT ON product_sales_channel_index_relation_freshness_snapshots
FOR EACH ROW
EXECUTE FUNCTION rustok_product_guard_channel_relation_freshness_snapshot();

CREATE OR REPLACE FUNCTION rustok_product_reject_channel_relation_freshness_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'Product-SalesChannel relation freshness snapshots are append-only';
    RETURN NULL;
END;
$$;

CREATE TRIGGER trg_product_channel_relation_freshness_snapshot_update
BEFORE UPDATE ON product_sales_channel_index_relation_freshness_snapshots
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_channel_relation_freshness_mutation();

CREATE TRIGGER trg_product_channel_relation_freshness_snapshot_delete
BEFORE DELETE ON product_sales_channel_index_relation_freshness_snapshots
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_channel_relation_freshness_mutation();
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
DROP TRIGGER IF EXISTS trg_product_channel_relation_freshness_snapshot_delete
    ON product_sales_channel_index_relation_freshness_snapshots;
DROP TRIGGER IF EXISTS trg_product_channel_relation_freshness_snapshot_update
    ON product_sales_channel_index_relation_freshness_snapshots;
DROP TRIGGER IF EXISTS trg_product_channel_relation_freshness_snapshot_insert
    ON product_sales_channel_index_relation_freshness_snapshots;
DROP TABLE IF EXISTS product_sales_channel_index_relation_freshness_snapshots;
DROP FUNCTION IF EXISTS rustok_product_reject_channel_relation_freshness_mutation();
DROP FUNCTION IF EXISTS rustok_product_guard_channel_relation_freshness_snapshot();
"#,
            )
            .await?;
        Ok(())
    }
}
