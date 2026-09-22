use rustok_core::field_schema::{create_field_definitions_table, drop_field_definitions_table};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_field_definitions_table(manager, "node", "nodes").await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_field_definitions_table(manager, "node").await
    }
}
