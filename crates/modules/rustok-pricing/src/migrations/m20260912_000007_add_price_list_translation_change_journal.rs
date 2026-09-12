use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut table = Table::create();
        table
            .table(PriceListTranslationChangeJournal::Table)
            .if_not_exists();
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            table.col(
                ColumnDef::new(PriceListTranslationChangeJournal::ChangeSeq)
                    .integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            );
        } else {
            table.col(
                ColumnDef::new(PriceListTranslationChangeJournal::ChangeSeq)
                    .big_integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            );
        }
        table
            .col(
                ColumnDef::new(PriceListTranslationChangeJournal::OperationId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(PriceListTranslationChangeJournal::TenantId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(PriceListTranslationChangeJournal::PriceListId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(PriceListTranslationChangeJournal::ResourceRevision)
                    .string_len(96)
                    .not_null(),
            )
            .col(
                ColumnDef::new(PriceListTranslationChangeJournal::Lifecycle)
                    .string_len(16)
                    .not_null(),
            )
            .col(
                ColumnDef::new(PriceListTranslationChangeJournal::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null()
                    .default(Expr::current_timestamp()),
            );
        manager.create_table(table.to_owned()).await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_price_list_translation_change_operation_target")
                    .table(PriceListTranslationChangeJournal::Table)
                    .col(PriceListTranslationChangeJournal::OperationId)
                    .col(PriceListTranslationChangeJournal::PriceListId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_price_list_translation_change_tenant_seq")
                    .table(PriceListTranslationChangeJournal::Table)
                    .col(PriceListTranslationChangeJournal::TenantId)
                    .col(PriceListTranslationChangeJournal::ChangeSeq)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_price_list_translation_change_target_seq")
                    .table(PriceListTranslationChangeJournal::Table)
                    .col(PriceListTranslationChangeJournal::TenantId)
                    .col(PriceListTranslationChangeJournal::PriceListId)
                    .col(PriceListTranslationChangeJournal::ChangeSeq)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PriceListTranslationChangeJournal::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PriceListTranslationChangeJournal {
    Table,
    ChangeSeq,
    OperationId,
    TenantId,
    PriceListId,
    ResourceRevision,
    Lifecycle,
    CreatedAt,
}
