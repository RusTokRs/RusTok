use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product-relations migrations require PostgreSQL".to_owned(),
            ));
        }

        // Historical rows may have duplicate or gapped positions. Compact each ordered relation
        // group before adding the durable invariant used by create, move, delete, and reorder.
        manager
            .get_connection()
            .execute_unprepared(
                r#"
WITH normalized AS (
    SELECT
        id,
        ROW_NUMBER() OVER (
            PARTITION BY tenant_id, product_id, relation_type
            ORDER BY position ASC, created_at ASC, id ASC
        ) - 1 AS normalized_position
    FROM product_relations
)
UPDATE product_relations AS relation
SET position = normalized.normalized_position::INT
FROM normalized
WHERE relation.id = normalized.id;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_product_relations_position_non_negative'
          AND conrelid = 'product_relations'::regclass
    ) THEN
        ALTER TABLE product_relations
            ADD CONSTRAINT chk_product_relations_position_non_negative
            CHECK (position >= 0);
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'uq_product_relations_tenant_product_type_position'
          AND conrelid = 'product_relations'::regclass
    ) THEN
        ALTER TABLE product_relations
            ADD CONSTRAINT uq_product_relations_tenant_product_type_position
            UNIQUE (tenant_id, product_id, relation_type, position);
    END IF;
END $$;
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product-relations migrations require PostgreSQL".to_owned(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE product_relations
    DROP CONSTRAINT IF EXISTS uq_product_relations_tenant_product_type_position;
ALTER TABLE product_relations
    DROP CONSTRAINT IF EXISTS chk_product_relations_position_non_negative;
"#,
            )
            .await?;
        Ok(())
    }
}
