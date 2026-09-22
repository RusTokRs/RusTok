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

        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE products
    ADD CONSTRAINT uq_products_tenant_id_for_product_relations
    UNIQUE (tenant_id, id);

ALTER TABLE product_relations
    ADD CONSTRAINT fk_product_relations_product
    FOREIGN KEY (tenant_id, product_id)
    REFERENCES products (tenant_id, id)
    ON DELETE CASCADE;

ALTER TABLE product_relations
    ADD CONSTRAINT fk_product_relations_related_product
    FOREIGN KEY (tenant_id, related_product_id)
    REFERENCES products (tenant_id, id)
    ON DELETE CASCADE;
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
    DROP CONSTRAINT IF EXISTS fk_product_relations_related_product;

ALTER TABLE product_relations
    DROP CONSTRAINT IF EXISTS fk_product_relations_product;

ALTER TABLE products
    DROP CONSTRAINT IF EXISTS uq_products_tenant_id_for_product_relations;
"#,
            )
            .await?;

        Ok(())
    }
}
