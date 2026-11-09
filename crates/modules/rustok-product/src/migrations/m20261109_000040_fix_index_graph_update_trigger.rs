use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product migrations require PostgreSQL".to_owned(),
            ));
        }

        // The BEFORE trigger changes the row even when index_revision is absent from SET.
        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE OR REPLACE TRIGGER trg_products_index_graph_projection_update
AFTER UPDATE ON products
FOR EACH ROW
WHEN (OLD.index_revision IS DISTINCT FROM NEW.index_revision)
EXECUTE FUNCTION rustok_product_capture_index_graph_projection_from_product();
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
CREATE OR REPLACE TRIGGER trg_products_index_graph_projection_update
AFTER UPDATE OF index_revision ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_capture_index_graph_projection_from_product();
"#,
            )
            .await?;
        Ok(())
    }
}
