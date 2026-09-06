use sea_orm::{ConnectionTrait, DatabaseBackend, TransactionTrait};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        let txn = manager.get_connection().begin().await?;
        txn.execute_unprepared(
            r#"
SELECT rustok_product_assert_category_tree();

DO $$
BEGIN
    IF to_regclass('catalog_category_closure') IS NULL THEN
        RAISE EXCEPTION 'catalog_category_closure must exist before CAT-34 storage retirement';
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM pg_trigger trigger
        JOIN pg_class relation ON relation.oid = trigger.tgrelid
        JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
        WHERE namespace.nspname = current_schema()
          AND relation.relname = 'catalog_category_closure'
          AND trigger.tgname = 'trg_catalog_category_closure_validate_tree'
          AND NOT trigger.tgisinternal
    ) THEN
        RAISE EXCEPTION 'CAT-33 closure compatibility trigger must exist before CAT-34 storage retirement';
    END IF;
END;
$$;

DROP TRIGGER trg_catalog_category_closure_validate_tree ON catalog_category_closure;
DROP TABLE catalog_category_closure;

-- The retained category trigger/function must remain valid after the closure table is gone.
SELECT rustok_product_assert_category_tree();
"#,
        )
        .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        let txn = manager.get_connection().begin().await?;
        txn.execute_unprepared(
            r#"
SELECT rustok_product_assert_category_tree();

CREATE TABLE catalog_category_closure (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    ancestor_id UUID NOT NULL REFERENCES catalog_categories(id) ON DELETE CASCADE,
    descendant_id UUID NOT NULL REFERENCES catalog_categories(id) ON DELETE CASCADE,
    depth INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, ancestor_id, descendant_id),
    CONSTRAINT chk_catalog_category_closure_depth CHECK (depth >= 0)
);

ALTER TABLE catalog_category_closure
    ADD CONSTRAINT fk_catalog_category_closure_ancestor_tenant
    FOREIGN KEY (tenant_id, ancestor_id)
    REFERENCES catalog_categories(tenant_id, id)
    ON DELETE CASCADE;

ALTER TABLE catalog_category_closure
    ADD CONSTRAINT fk_catalog_category_closure_descendant_tenant
    FOREIGN KEY (tenant_id, descendant_id)
    REFERENCES catalog_categories(tenant_id, id)
    ON DELETE CASCADE;

CREATE INDEX idx_catalog_category_closure_descendant
    ON catalog_category_closure (tenant_id, descendant_id, depth);

WITH RECURSIVE category_walk AS (
    SELECT
        tenant_id,
        id AS descendant_id,
        id AS ancestor_id,
        parent_id,
        0 AS depth,
        ARRAY[id]::UUID[] AS visited_ids
    FROM catalog_categories

    UNION ALL

    SELECT
        walk.tenant_id,
        walk.descendant_id,
        parent.id AS ancestor_id,
        parent.parent_id,
        walk.depth + 1,
        walk.visited_ids || parent.id
    FROM category_walk walk
    JOIN catalog_categories parent
      ON parent.tenant_id = walk.tenant_id
     AND parent.id = walk.parent_id
    WHERE walk.parent_id IS NOT NULL
      AND NOT parent.id = ANY(walk.visited_ids)
)
INSERT INTO catalog_category_closure (tenant_id, ancestor_id, descendant_id, depth)
SELECT tenant_id, ancestor_id, descendant_id, depth
FROM category_walk;

DO $$
BEGIN
    IF EXISTS (
        WITH RECURSIVE category_walk AS (
            SELECT
                tenant_id,
                id AS descendant_id,
                id AS ancestor_id,
                parent_id,
                0 AS depth,
                ARRAY[id]::UUID[] AS visited_ids
            FROM catalog_categories

            UNION ALL

            SELECT
                walk.tenant_id,
                walk.descendant_id,
                parent.id AS ancestor_id,
                parent.parent_id,
                walk.depth + 1,
                walk.visited_ids || parent.id
            FROM category_walk walk
            JOIN catalog_categories parent
              ON parent.tenant_id = walk.tenant_id
             AND parent.id = walk.parent_id
            WHERE walk.parent_id IS NOT NULL
              AND NOT parent.id = ANY(walk.visited_ids)
        ),
        expected_closure AS (
            SELECT tenant_id, ancestor_id, descendant_id, depth
            FROM category_walk
        )
        SELECT 1
        FROM expected_closure expected
        FULL OUTER JOIN catalog_category_closure actual
          ON actual.tenant_id = expected.tenant_id
         AND actual.ancestor_id = expected.ancestor_id
         AND actual.descendant_id = expected.descendant_id
        WHERE expected.ancestor_id IS NULL
           OR actual.ancestor_id IS NULL
           OR actual.depth <> expected.depth
    ) THEN
        RAISE EXCEPTION 'CAT-34 closure reconstruction is not the Product parent-tree projection';
    END IF;
END;
$$;

CREATE CONSTRAINT TRIGGER trg_catalog_category_closure_validate_tree
AFTER INSERT OR UPDATE OR DELETE ON catalog_category_closure
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION rustok_product_validate_category_tree_trigger();

SELECT rustok_product_assert_category_tree();
"#,
        )
        .await?;
        txn.commit().await?;
        Ok(())
    }
}
