use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const USER_FIELD_DEFINITIONS_TABLE: &str = "user_field_definitions";
const USER_TRIGGER: &str = "flex_user_fd_cache_generation";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        flex::cache_generation::create_field_definition_cache_generation_table(manager).await?;
        flex::cache_generation::create_field_definition_cache_generation_trigger(
            manager,
            USER_FIELD_DEFINITIONS_TABLE,
            USER_TRIGGER,
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        flex::cache_generation::drop_field_definition_cache_generation_trigger(
            manager,
            USER_FIELD_DEFINITIONS_TABLE,
            USER_TRIGGER,
        )
        .await
    }
}
