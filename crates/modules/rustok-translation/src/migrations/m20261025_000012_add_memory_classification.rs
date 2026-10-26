use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MemoryEntries::Table)
                    .add_column(
                        ColumnDef::new(MemoryEntries::DataClassification)
                            .string_len(32)
                            .not_null()
                            .default("tenant_private"),
                    )
                    .to_owned(),
            )
            .await?;
        // Backfill existing rows as tenant_private (most conservative) then add check
        manager
            .create_index(
                Index::create()
                    .name("idx_translation_memory_data_classification")
                    .table(MemoryEntries::Table)
                    .col(MemoryEntries::DataClassification)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[derive(Iden)]
enum MemoryEntries {
    #[iden = "translation_memory_entries"]
    Table,
    DataClassification,
}
