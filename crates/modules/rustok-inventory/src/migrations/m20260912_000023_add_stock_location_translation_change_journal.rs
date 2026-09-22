use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut table = Table::create();
        table
            .table(StockLocationTranslationChangeJournal::Table)
            .if_not_exists();
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            table.col(
                ColumnDef::new(StockLocationTranslationChangeJournal::ChangeSeq)
                    .integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            );
        } else {
            table.col(
                ColumnDef::new(StockLocationTranslationChangeJournal::ChangeSeq)
                    .big_integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            );
        }
        table
            .col(
                ColumnDef::new(StockLocationTranslationChangeJournal::OperationId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(StockLocationTranslationChangeJournal::TenantId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(StockLocationTranslationChangeJournal::StockLocationId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(StockLocationTranslationChangeJournal::ResourceRevision)
                    .string_len(96)
                    .not_null(),
            )
            .col(
                ColumnDef::new(StockLocationTranslationChangeJournal::Lifecycle)
                    .string_len(16)
                    .not_null(),
            )
            .col(
                ColumnDef::new(StockLocationTranslationChangeJournal::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null()
                    .default(Expr::current_timestamp()),
            );
        manager.create_table(table.to_owned()).await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_stock_location_translation_change_operation_target")
                    .table(StockLocationTranslationChangeJournal::Table)
                    .col(StockLocationTranslationChangeJournal::OperationId)
                    .col(StockLocationTranslationChangeJournal::StockLocationId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_stock_location_translation_change_tenant_seq")
                    .table(StockLocationTranslationChangeJournal::Table)
                    .col(StockLocationTranslationChangeJournal::TenantId)
                    .col(StockLocationTranslationChangeJournal::ChangeSeq)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_stock_location_translation_change_target_seq")
                    .table(StockLocationTranslationChangeJournal::Table)
                    .col(StockLocationTranslationChangeJournal::TenantId)
                    .col(StockLocationTranslationChangeJournal::StockLocationId)
                    .col(StockLocationTranslationChangeJournal::ChangeSeq)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(StockLocationTranslationChangeJournal::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum StockLocationTranslationChangeJournal {
    Table,
    ChangeSeq,
    OperationId,
    TenantId,
    StockLocationId,
    ResourceRevision,
    Lifecycle,
    CreatedAt,
}
