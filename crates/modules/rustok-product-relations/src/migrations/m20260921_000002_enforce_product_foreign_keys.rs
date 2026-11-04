use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == DatabaseBackend::Postgres;

        if is_postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
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
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == DatabaseBackend::Postgres;

        if is_postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
ALTER TABLE product_relations
    DROP CONSTRAINT IF EXISTS fk_product_relations_related_product;

ALTER TABLE product_relations
    DROP CONSTRAINT IF EXISTS fk_product_relations_product;
"#,
                )
                .await?;
        }

        Ok(())
    }
}
