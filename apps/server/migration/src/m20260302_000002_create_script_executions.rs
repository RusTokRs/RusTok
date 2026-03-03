use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        alloy_scripting::ScriptExecutionsMigration.up(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        alloy_scripting::ScriptExecutionsMigration
            .down(manager)
            .await
    }
}
