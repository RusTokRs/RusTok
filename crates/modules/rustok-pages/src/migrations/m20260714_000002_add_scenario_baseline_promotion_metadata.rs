use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PageBuilderScenarioBaselines::Table)
                    .add_column(
                        ColumnDef::new(PageBuilderScenarioBaselines::PreviousBaselineHash)
                            .string_len(128)
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(PageBuilderScenarioBaselines::Table)
                    .add_column(
                        ColumnDef::new(PageBuilderScenarioBaselines::PromotedBy)
                            .uuid()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(PageBuilderScenarioBaselines::Table)
                    .add_column(
                        ColumnDef::new(PageBuilderScenarioBaselines::PromotionNote)
                            .text()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(PageBuilderScenarioBaselines::Table)
                    .add_column(
                        ColumnDef::new(PageBuilderScenarioBaselines::PromotedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PageBuilderScenarioBaselines::Table)
                    .drop_column(PageBuilderScenarioBaselines::PromotedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(PageBuilderScenarioBaselines::Table)
                    .drop_column(PageBuilderScenarioBaselines::PromotionNote)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(PageBuilderScenarioBaselines::Table)
                    .drop_column(PageBuilderScenarioBaselines::PromotedBy)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(PageBuilderScenarioBaselines::Table)
                    .drop_column(PageBuilderScenarioBaselines::PreviousBaselineHash)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PageBuilderScenarioBaselines {
    Table,
    PreviousBaselineHash,
    PromotedBy,
    PromotionNote,
    PromotedAt,
}
