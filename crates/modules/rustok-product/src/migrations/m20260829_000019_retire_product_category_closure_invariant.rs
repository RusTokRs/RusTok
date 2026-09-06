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

CREATE OR REPLACE FUNCTION rustok_product_assert_category_tree()
RETURNS VOID AS $$
BEGIN
    IF EXISTS (
        WITH RECURSIVE category_walk AS (
            SELECT
                tenant_id,
                id AS descendant_id,
                id AS ancestor_id,
                parent_id,
                ARRAY[id]::UUID[] AS visited_ids,
                FALSE AS has_cycle
            FROM catalog_categories

            UNION ALL

            SELECT
                walk.tenant_id,
                walk.descendant_id,
                parent.id AS ancestor_id,
                parent.parent_id,
                walk.visited_ids || parent.id,
                parent.id = ANY(walk.visited_ids)
            FROM category_walk walk
            JOIN catalog_categories parent
              ON parent.tenant_id = walk.tenant_id
             AND parent.id = walk.parent_id
            WHERE walk.parent_id IS NOT NULL
              AND NOT walk.has_cycle
        )
        SELECT 1
        FROM category_walk
        WHERE has_cycle
    ) THEN
        RAISE EXCEPTION 'catalog category tree contains a cycle';
    END IF;
END;
$$ LANGUAGE plpgsql;

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

DELETE FROM catalog_category_closure;

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

CREATE OR REPLACE FUNCTION rustok_product_assert_category_tree()
RETURNS VOID AS $$
BEGIN
    IF EXISTS (
        WITH RECURSIVE category_walk AS (
            SELECT
                tenant_id,
                id AS descendant_id,
                id AS ancestor_id,
                parent_id,
                0 AS depth,
                ARRAY[id]::UUID[] AS visited_ids,
                FALSE AS has_cycle
            FROM catalog_categories

            UNION ALL

            SELECT
                walk.tenant_id,
                walk.descendant_id,
                parent.id AS ancestor_id,
                parent.parent_id,
                walk.depth + 1,
                walk.visited_ids || parent.id,
                parent.id = ANY(walk.visited_ids)
            FROM category_walk walk
            JOIN catalog_categories parent
              ON parent.tenant_id = walk.tenant_id
             AND parent.id = walk.parent_id
            WHERE walk.parent_id IS NOT NULL
              AND NOT walk.has_cycle
        )
        SELECT 1
        FROM category_walk
        WHERE has_cycle
    ) THEN
        RAISE EXCEPTION 'catalog category tree contains a cycle';
    END IF;

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
        RAISE EXCEPTION 'catalog category closure is not the canonical parent-tree projection';
    END IF;
END;
$$ LANGUAGE plpgsql;

SELECT rustok_product_assert_category_tree();
"#,
        )
        .await?;
        txn.commit().await?;
        Ok(())
    }
}
