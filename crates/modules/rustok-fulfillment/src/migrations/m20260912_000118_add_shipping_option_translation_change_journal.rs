use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut table = Table::create();
        table
            .table(ShippingOptionTranslationChangeJournal::Table)
            .if_not_exists();
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            table.col(
                ColumnDef::new(ShippingOptionTranslationChangeJournal::ChangeSeq)
                    .integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            );
        } else {
            table.col(
                ColumnDef::new(ShippingOptionTranslationChangeJournal::ChangeSeq)
                    .big_integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            );
        }
        table
            .col(
                ColumnDef::new(ShippingOptionTranslationChangeJournal::OperationId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(ShippingOptionTranslationChangeJournal::TenantId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(ShippingOptionTranslationChangeJournal::ShippingOptionId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(ShippingOptionTranslationChangeJournal::ResourceRevision)
                    .string_len(96)
                    .not_null(),
            )
            .col(
                ColumnDef::new(ShippingOptionTranslationChangeJournal::Lifecycle)
                    .string_len(16)
                    .not_null(),
            )
            .col(
                ColumnDef::new(ShippingOptionTranslationChangeJournal::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null()
                    .default(Expr::current_timestamp()),
            );
        manager.create_table(table.to_owned()).await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_shipping_option_translation_change_operation_target")
                    .table(ShippingOptionTranslationChangeJournal::Table)
                    .col(ShippingOptionTranslationChangeJournal::OperationId)
                    .col(ShippingOptionTranslationChangeJournal::ShippingOptionId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_shipping_option_translation_change_tenant_seq")
                    .table(ShippingOptionTranslationChangeJournal::Table)
                    .col(ShippingOptionTranslationChangeJournal::TenantId)
                    .col(ShippingOptionTranslationChangeJournal::ChangeSeq)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_shipping_option_translation_change_target_seq")
                    .table(ShippingOptionTranslationChangeJournal::Table)
                    .col(ShippingOptionTranslationChangeJournal::TenantId)
                    .col(ShippingOptionTranslationChangeJournal::ShippingOptionId)
                    .col(ShippingOptionTranslationChangeJournal::ChangeSeq)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ShippingOptionTranslationChangeJournal::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ShippingOptionTranslationChangeJournal {
    Table,
    ChangeSeq,
    OperationId,
    TenantId,
    ShippingOptionId,
    ResourceRevision,
    Lifecycle,
    CreatedAt,
}
