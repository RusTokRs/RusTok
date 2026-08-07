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
DO $$
BEGIN
    IF to_regclass('product_index_graph_projection_snapshots') IS NULL
       AND to_regclass('product_index_graph_v3_projection_snapshots') IS NOT NULL
    THEN
        ALTER TABLE product_index_graph_v3_projection_snapshots
            RENAME TO product_index_graph_projection_snapshots;
    END IF;

    IF to_regclass('product_index_graph_projection_snapshots') IS NULL THEN
        RAISE EXCEPTION 'Product Index graph projection storage is missing';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'pk_product_index_graph_v3_projection_snapshots'
          AND conrelid = 'product_index_graph_projection_snapshots'::regclass
    ) THEN
        ALTER TABLE product_index_graph_projection_snapshots
            RENAME CONSTRAINT pk_product_index_graph_v3_projection_snapshots
            TO pk_product_index_graph_projection_snapshots;
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'uq_product_index_graph_v3_projection_tenant_sequence'
          AND conrelid = 'product_index_graph_projection_snapshots'::regclass
    ) THEN
        ALTER TABLE product_index_graph_projection_snapshots
            RENAME CONSTRAINT uq_product_index_graph_v3_projection_tenant_sequence
            TO uq_product_index_graph_projection_tenant_sequence;
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'uq_product_index_graph_v3_projection_input_pair'
          AND conrelid = 'product_index_graph_projection_snapshots'::regclass
    ) THEN
        ALTER TABLE product_index_graph_projection_snapshots
            RENAME CONSTRAINT uq_product_index_graph_v3_projection_input_pair
            TO uq_product_index_graph_projection_input_pair;
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'chk_product_index_graph_v3_projection_sequence_positive'
          AND conrelid = 'product_index_graph_projection_snapshots'::regclass
    ) THEN
        ALTER TABLE product_index_graph_projection_snapshots
            RENAME CONSTRAINT chk_product_index_graph_v3_projection_sequence_positive
            TO chk_product_index_graph_projection_sequence_positive;
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'chk_product_index_graph_v3_projection_tenant_non_nil'
          AND conrelid = 'product_index_graph_projection_snapshots'::regclass
    ) THEN
        ALTER TABLE product_index_graph_projection_snapshots
            RENAME CONSTRAINT chk_product_index_graph_v3_projection_tenant_non_nil
            TO chk_product_index_graph_projection_tenant_non_nil;
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'chk_product_index_graph_v3_projection_product_non_nil'
          AND conrelid = 'product_index_graph_projection_snapshots'::regclass
    ) THEN
        ALTER TABLE product_index_graph_projection_snapshots
            RENAME CONSTRAINT chk_product_index_graph_v3_projection_product_non_nil
            TO chk_product_index_graph_projection_product_non_nil;
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'chk_product_index_graph_v3_projection_epoch_positive'
          AND conrelid = 'product_index_graph_projection_snapshots'::regclass
    ) THEN
        ALTER TABLE product_index_graph_projection_snapshots
            RENAME CONSTRAINT chk_product_index_graph_v3_projection_epoch_positive
            TO chk_product_index_graph_projection_epoch_positive;
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'chk_product_index_graph_v3_projection_product_source_positive'
          AND conrelid = 'product_index_graph_projection_snapshots'::regclass
    ) THEN
        ALTER TABLE product_index_graph_projection_snapshots
            RENAME CONSTRAINT chk_product_index_graph_v3_projection_product_source_positive
            TO chk_product_index_graph_projection_product_source_positive;
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'chk_product_index_graph_v3_projection_relation_epoch_positive'
          AND conrelid = 'product_index_graph_projection_snapshots'::regclass
    ) THEN
        ALTER TABLE product_index_graph_projection_snapshots
            RENAME CONSTRAINT chk_product_index_graph_v3_projection_relation_epoch_positive
            TO chk_product_index_graph_projection_relation_epoch_positive;
    END IF;
END;
$$;

DO $$
BEGIN
    IF to_regclass('idx_product_index_graph_v3_projection_current') IS NOT NULL
       AND to_regclass('idx_product_index_graph_projection_current') IS NULL
    THEN
        ALTER INDEX idx_product_index_graph_v3_projection_current
            RENAME TO idx_product_index_graph_projection_current;
    END IF;
END;
$$;

DROP TRIGGER IF EXISTS trg_product_index_graph_v3_projection_insert
    ON product_index_graph_projection_snapshots;
DROP TRIGGER IF EXISTS trg_product_index_graph_v3_projection_update
    ON product_index_graph_projection_snapshots;
DROP TRIGGER IF EXISTS trg_product_index_graph_v3_projection_delete
    ON product_index_graph_projection_snapshots;
DROP TRIGGER IF EXISTS trg_products_index_graph_v3_projection_insert ON products;
DROP TRIGGER IF EXISTS trg_products_index_graph_v3_projection_update ON products;
DROP TRIGGER IF EXISTS trg_products_zz_index_graph_v3_projection_delete ON products;
DROP TRIGGER IF EXISTS trg_product_channel_relation_index_graph_v3_projection_insert
    ON product_sales_channel_index_relation_snapshots;

DROP FUNCTION IF EXISTS rustok_product_capture_index_graph_v3_projection_from_relation();
DROP FUNCTION IF EXISTS rustok_product_capture_index_graph_v3_projection_from_product();
DROP FUNCTION IF EXISTS rustok_product_reconcile_index_graph_v3_projection(UUID, UUID);
DROP FUNCTION IF EXISTS rustok_product_reject_index_graph_v3_projection_mutation();
DROP FUNCTION IF EXISTS rustok_product_guard_index_graph_v3_projection_snapshot();

CREATE OR REPLACE FUNCTION rustok_product_guard_index_graph_projection_snapshot()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    previous_projection_epoch BIGINT;
    previous_product_source_version BIGINT;
    previous_relation_epoch BIGINT;
    lock_key TEXT;
BEGIN
    lock_key := NEW.tenant_id::text
        || E'\x1f' || NEW.product_id::text
        || E'\x1fproduct-index-graph-projection';
    PERFORM pg_advisory_xact_lock(hashtextextended(lock_key, 0));

    SELECT
        projection.projection_epoch,
        projection.product_source_version,
        projection.relation_epoch
      INTO
        previous_projection_epoch,
        previous_product_source_version,
        previous_relation_epoch
      FROM product_index_graph_projection_snapshots projection
     WHERE projection.tenant_id = NEW.tenant_id
       AND projection.product_id = NEW.product_id
     ORDER BY projection.projection_epoch DESC
     LIMIT 1;

    IF previous_projection_epoch IS NULL THEN
        IF NEW.projection_epoch <> 1 THEN
            RAISE EXCEPTION 'first Product Index graph projection epoch must equal 1';
        END IF;
        RETURN NEW;
    END IF;

    IF previous_projection_epoch = 9223372036854775807 THEN
        RAISE EXCEPTION 'Product Index graph projection epoch is exhausted';
    END IF;
    IF NEW.projection_epoch <> previous_projection_epoch + 1 THEN
        RAISE EXCEPTION 'Product Index graph projection epoch must advance exactly once';
    END IF;
    IF NEW.product_source_version < previous_product_source_version
       OR NEW.relation_epoch < previous_relation_epoch
    THEN
        RAISE EXCEPTION 'Product Index graph projection input watermark regressed';
    END IF;
    IF NEW.product_source_version = previous_product_source_version
       AND NEW.relation_epoch = previous_relation_epoch
    THEN
        RAISE EXCEPTION 'unchanged Product Index graph projection input must not append a new epoch';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_product_index_graph_projection_insert
BEFORE INSERT ON product_index_graph_projection_snapshots
FOR EACH ROW
EXECUTE FUNCTION rustok_product_guard_index_graph_projection_snapshot();

CREATE OR REPLACE FUNCTION rustok_product_reject_index_graph_projection_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'Product Index graph projection snapshots are append-only';
    RETURN NULL;
END;
$$;

CREATE TRIGGER trg_product_index_graph_projection_update
BEFORE UPDATE ON product_index_graph_projection_snapshots
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_index_graph_projection_mutation();

CREATE TRIGGER trg_product_index_graph_projection_delete
BEFORE DELETE ON product_index_graph_projection_snapshots
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_index_graph_projection_mutation();

CREATE OR REPLACE FUNCTION rustok_product_reconcile_index_graph_projection(
    target_tenant_id UUID,
    target_product_id UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    observed_product_source_version BIGINT;
    observed_relation_epoch BIGINT;
    previous_projection_epoch BIGINT;
    previous_product_source_version BIGINT;
    previous_relation_epoch BIGINT;
    effective_product_source_version BIGINT;
    effective_relation_epoch BIGINT;
    lock_key TEXT;
BEGIN
    IF target_tenant_id IS NULL
       OR target_product_id IS NULL
       OR target_tenant_id = '00000000-0000-0000-0000-000000000000'::uuid
       OR target_product_id = '00000000-0000-0000-0000-000000000000'::uuid
    THEN
        RAISE EXCEPTION 'Product Index graph projection identity is invalid';
    END IF;

    lock_key := target_tenant_id::text
        || E'\x1f' || target_product_id::text
        || E'\x1fproduct-index-graph-projection';
    PERFORM pg_advisory_xact_lock(hashtextextended(lock_key, 0));

    SELECT product.index_revision
      INTO observed_product_source_version
      FROM products product
     WHERE product.tenant_id = target_tenant_id
       AND product.id = target_product_id;

    IF observed_product_source_version IS NULL THEN
        SELECT MAX(tombstone.source_version)
          INTO observed_product_source_version
          FROM product_index_tombstones tombstone
         WHERE tombstone.tenant_id = target_tenant_id
           AND tombstone.product_id = target_product_id;
    END IF;

    SELECT relation.relation_epoch
      INTO observed_relation_epoch
      FROM product_sales_channel_index_relation_snapshots relation
     WHERE relation.tenant_id = target_tenant_id
       AND relation.product_id = target_product_id
     ORDER BY relation.relation_epoch DESC
     LIMIT 1;

    IF observed_product_source_version IS NULL OR observed_relation_epoch IS NULL THEN
        RETURN;
    END IF;
    IF observed_product_source_version <= 0 OR observed_relation_epoch <= 0 THEN
        RAISE EXCEPTION 'Product Index graph projection input version is invalid';
    END IF;

    SELECT
        projection.projection_epoch,
        projection.product_source_version,
        projection.relation_epoch
      INTO
        previous_projection_epoch,
        previous_product_source_version,
        previous_relation_epoch
      FROM product_index_graph_projection_snapshots projection
     WHERE projection.tenant_id = target_tenant_id
       AND projection.product_id = target_product_id
     ORDER BY projection.projection_epoch DESC
     LIMIT 1;

    IF previous_projection_epoch IS NULL THEN
        INSERT INTO product_index_graph_projection_snapshots (
            tenant_id,
            product_id,
            projection_epoch,
            product_source_version,
            relation_epoch
        ) VALUES (
            target_tenant_id,
            target_product_id,
            1,
            observed_product_source_version,
            observed_relation_epoch
        );
        RETURN;
    END IF;

    -- GREATEST merges retained component watermarks. projection_epoch remains the only complete
    -- Product graph mutation clock and advances exactly once whenever either component advances.
    effective_product_source_version := GREATEST(
        observed_product_source_version,
        previous_product_source_version
    );
    effective_relation_epoch := GREATEST(
        observed_relation_epoch,
        previous_relation_epoch
    );

    IF effective_product_source_version = previous_product_source_version
       AND effective_relation_epoch = previous_relation_epoch
    THEN
        RETURN;
    END IF;
    IF previous_projection_epoch = 9223372036854775807 THEN
        RAISE EXCEPTION 'Product Index graph projection epoch is exhausted';
    END IF;

    INSERT INTO product_index_graph_projection_snapshots (
        tenant_id,
        product_id,
        projection_epoch,
        product_source_version,
        relation_epoch
    ) VALUES (
        target_tenant_id,
        target_product_id,
        previous_projection_epoch + 1,
        effective_product_source_version,
        effective_relation_epoch
    );
END;
$$;

CREATE OR REPLACE FUNCTION rustok_product_capture_index_graph_projection_from_product()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM rustok_product_reconcile_index_graph_projection(OLD.tenant_id, OLD.id);
        RETURN OLD;
    END IF;

    PERFORM rustok_product_reconcile_index_graph_projection(NEW.tenant_id, NEW.id);
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_products_index_graph_projection_insert
AFTER INSERT ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_capture_index_graph_projection_from_product();

CREATE TRIGGER trg_products_index_graph_projection_update
AFTER UPDATE OF index_revision ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_capture_index_graph_projection_from_product();

-- Same-kind PostgreSQL triggers run in name order. This sorts after
-- trg_products_retain_empty_channel_relation so a hard delete first retains the final empty
-- relation epoch; the relation INSERT trigger then advances projection state before this trailing
-- idempotent reconciliation.
CREATE TRIGGER trg_products_zz_index_graph_projection_delete
AFTER DELETE ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_capture_index_graph_projection_from_product();

CREATE OR REPLACE FUNCTION rustok_product_capture_index_graph_projection_from_relation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM rustok_product_reconcile_index_graph_projection(NEW.tenant_id, NEW.product_id);
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_product_channel_relation_index_graph_projection_insert
AFTER INSERT ON product_sales_channel_index_relation_snapshots
FOR EACH ROW
EXECUTE FUNCTION rustok_product_capture_index_graph_projection_from_relation();
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "canonical Product Index graph projection migration is intentionally irreversible"
                .to_owned(),
        ))
    }
}
