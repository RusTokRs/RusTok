use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RegionTranslationChangeJournal::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RegionTranslationChangeJournal::ChangeSeq)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RegionTranslationChangeJournal::OperationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegionTranslationChangeJournal::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegionTranslationChangeJournal::RegionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegionTranslationChangeJournal::ResourceRevision)
                            .string_len(96)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegionTranslationChangeJournal::Lifecycle)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegionTranslationChangeJournal::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_region_translation_change_operation_target")
                    .table(RegionTranslationChangeJournal::Table)
                    .col(RegionTranslationChangeJournal::OperationId)
                    .col(RegionTranslationChangeJournal::RegionId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_region_translation_change_tenant_seq")
                    .table(RegionTranslationChangeJournal::Table)
                    .col(RegionTranslationChangeJournal::TenantId)
                    .col(RegionTranslationChangeJournal::ChangeSeq)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_region_translation_change_target_seq")
                    .table(RegionTranslationChangeJournal::Table)
                    .col(RegionTranslationChangeJournal::TenantId)
                    .col(RegionTranslationChangeJournal::RegionId)
                    .col(RegionTranslationChangeJournal::ChangeSeq)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RegionTranslationChangeJournal::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum RegionTranslationChangeJournal {
    Table,
    ChangeSeq,
    OperationId,
    TenantId,
    RegionId,
    ResourceRevision,
    Lifecycle,
    CreatedAt,
}
