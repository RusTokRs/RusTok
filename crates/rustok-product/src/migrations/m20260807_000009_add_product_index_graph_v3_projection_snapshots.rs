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
CREATE TABLE product_index_graph_v3_projection_snapshots (
    sequence_no BIGSERIAL NOT NULL,
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    projection_epoch BIGINT NOT NULL,
    product_source_version BIGINT NOT NULL,
    relation_epoch BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_product_index_graph_v3_projection_snapshots
        PRIMARY KEY (tenant_id, product_id, projection_epoch),
    CONSTRAINT uq_product_index_graph_v3_projection_tenant_sequence
        UNIQUE (tenant_id, sequence_no),
    CONSTRAINT uq_product_index_graph_v3_projection_input_pair
        UNIQUE (tenant_id, product_id, product_source_version, relation_epoch),
    CONSTRAINT chk_product_index_graph_v3_projection_sequence_positive
        CHECK (sequence_no > 0),
    CONSTRAINT chk_product_index_graph_v3_projection_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_index_graph_v3_projection_product_non_nil
        CHECK (product_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_index_graph_v3_projection_epoch_positive
        CHECK (projection_epoch > 0),
    CONSTRAINT chk_product_index_graph_v3_projection_product_source_positive
        CHECK (product_source_version > 0),
    CONSTRAINT chk_product_index_graph_v3_projection_relation_epoch_positive
        CHECK (relation_epoch > 0)
);

CREATE INDEX idx_product_index_graph_v3_projection_current
    ON product_index_graph_v3_projection_snapshots (
        tenant_id,
        product_id,
        projection_epoch DESC
    );

CREATE OR REPLACE FUNCTION rustok_product_reject_index_graph_v3_projection_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'Product Index graph v3 projection snapshots are append-only';
    RETURN NULL;
END;
$$;

CREATE TRIGGER trg_product_index_graph_v3_projection_update
BEFORE UPDATE ON product_index_graph_v3_projection_snapshots
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_index_graph_v3_projection_mutation();

CREATE TRIGGER trg_product_index_graph_v3_projection_delete
BEFORE DELETE ON product_index_graph_v3_projection_snapshots
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_index_graph_v3_projection_mutation();

CREATE OR REPLACE FUNCTION rustok_product_reconcile_index_graph_v3_projection(
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
        RAISE EXCEPTION 'Product Index graph v3 projection identity is invalid';
    END IF;

    lock_key := target_tenant_id::text
        || E'\x1f' || target_product_id::text
        || E'\x1fproduct-index-graph-v3-projection';
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
        RAISE EXCEPTION 'Product Index graph v3 projection input version is invalid';
    END IF;

    SELECT
        projection.projection_epoch,
        projection.product_source_version,
        projection.relation_epoch
      INTO
        previous_projection_epoch,
        previous_product_source_version,
        previous_relation_epoch
      FROM product_index_graph_v3_projection_snapshots projection
     WHERE projection.tenant_id = target_tenant_id
       AND projection.product_id = target_product_id
     ORDER BY projection.projection_epoch DESC
     LIMIT 1;

    IF previous_projection_epoch IS NULL THEN
        INSERT INTO product_index_graph_v3_projection_snapshots (
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
        RAISE EXCEPTION 'Product Index graph v3 projection epoch is exhausted';
    END IF;

    INSERT INTO product_index_graph_v3_projection_snapshots (
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

WITH live_product_versions AS (
    SELECT
        product.tenant_id,
        product.id AS product_id,
        product.index_revision AS product_source_version
    FROM products product
),
retained_product_versions AS (
    SELECT
        tombstone.tenant_id,
        tombstone.product_id,
        MAX(tombstone.source_version) AS product_source_version
    FROM product_index_tombstones tombstone
    WHERE NOT EXISTS (
        SELECT 1
        FROM products product
        WHERE product.tenant_id = tombstone.tenant_id
          AND product.id = tombstone.product_id
    )
    GROUP BY tombstone.tenant_id, tombstone.product_id
),
product_versions AS (
    SELECT tenant_id, product_id, product_source_version FROM live_product_versions
    UNION ALL
    SELECT tenant_id, product_id, product_source_version FROM retained_product_versions
),
current_relations AS (
    SELECT DISTINCT ON (relation.tenant_id, relation.product_id)
        relation.tenant_id,
        relation.product_id,
        relation.relation_epoch
    FROM product_sales_channel_index_relation_snapshots relation
    ORDER BY relation.tenant_id, relation.product_id, relation.relation_epoch DESC
)
INSERT INTO product_index_graph_v3_projection_snapshots (
    tenant_id,
    product_id,
    projection_epoch,
    product_source_version,
    relation_epoch
)
SELECT
    product_version.tenant_id,
    product_version.product_id,
    1,
    product_version.product_source_version,
    relation.relation_epoch
FROM product_versions product_version
JOIN current_relations relation
  ON relation.tenant_id = product_version.tenant_id
 AND relation.product_id = product_version.product_id
ORDER BY product_version.tenant_id, product_version.product_id;

CREATE OR REPLACE FUNCTION rustok_product_capture_index_graph_v3_projection_from_product()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM rustok_product_reconcile_index_graph_v3_projection(OLD.tenant_id, OLD.id);
        RETURN OLD;
    END IF;

    PERFORM rustok_product_reconcile_index_graph_v3_projection(NEW.tenant_id, NEW.id);
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_products_index_graph_v3_projection_insert
AFTER INSERT ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_capture_index_graph_v3_projection_from_product();

CREATE TRIGGER trg_products_index_graph_v3_projection_update
AFTER UPDATE OF index_revision ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_capture_index_graph_v3_projection_from_product();

CREATE TRIGGER trg_products_index_graph_v3_projection_delete
AFTER DELETE ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_capture_index_graph_v3_projection_from_product();

CREATE OR REPLACE FUNCTION rustok_product_capture_index_graph_v3_projection_from_relation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM rustok_product_reconcile_index_graph_v3_projection(NEW.tenant_id, NEW.product_id);
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_product_channel_relation_index_graph_v3_projection_insert
AFTER INSERT ON product_sales_channel_index_relation_snapshots
FOR EACH ROW
EXECUTE FUNCTION rustok_product_capture_index_graph_v3_projection_from_relation();
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
DROP TRIGGER IF EXISTS trg_product_channel_relation_index_graph_v3_projection_insert
    ON product_sales_channel_index_relation_snapshots;
DROP TRIGGER IF EXISTS trg_products_index_graph_v3_projection_delete ON products;
DROP TRIGGER IF EXISTS trg_products_index_graph_v3_projection_update ON products;
DROP TRIGGER IF EXISTS trg_products_index_graph_v3_projection_insert ON products;
DROP TRIGGER IF EXISTS trg_product_index_graph_v3_projection_delete
    ON product_index_graph_v3_projection_snapshots;
DROP TRIGGER IF EXISTS trg_product_index_graph_v3_projection_update
    ON product_index_graph_v3_projection_snapshots;
DROP FUNCTION IF EXISTS rustok_product_capture_index_graph_v3_projection_from_relation();
DROP FUNCTION IF EXISTS rustok_product_capture_index_graph_v3_projection_from_product();
DROP FUNCTION IF EXISTS rustok_product_reconcile_index_graph_v3_projection(UUID, UUID);
DROP FUNCTION IF EXISTS rustok_product_reject_index_graph_v3_projection_mutation();
DROP TABLE IF EXISTS product_index_graph_v3_projection_snapshots;
"#,
            )
            .await?;

        Ok(())
    }
}
