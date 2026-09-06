use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLE: &str = "product_field_definitions";
const TRIGGER: &str = "flex_product_fd_cache_generation";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        flex::cache_generation::create_field_definition_cache_generation_trigger(
            manager, TABLE, TRIGGER,
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        flex::cache_generation::drop_field_definition_cache_generation_trigger(
            manager, TABLE, TRIGGER,
        )
        .await
    }
}
