use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ModerationCases::Table)
                    .add_column(
                        ColumnDef::new(ModerationCases::DeduplicationKey)
                            .string_len(64)
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ModerationCases::Table)
                    .add_column(
                        ColumnDef::new(ModerationCases::ActiveDeduplicationKey).string_len(64),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_moderation_cases_active_deduplication")
                    .table(ModerationCases::Table)
                    .col(ModerationCases::TenantId)
                    .col(ModerationCases::ActiveDeduplicationKey)
                    .unique()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_moderation_cases_active_deduplication")
                    .table(ModerationCases::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ModerationCases::Table)
                    .drop_column(ModerationCases::ActiveDeduplicationKey)
                    .drop_column(ModerationCases::DeduplicationKey)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ModerationCases {
    Table,
    TenantId,
    DeduplicationKey,
    ActiveDeduplicationKey,
}
