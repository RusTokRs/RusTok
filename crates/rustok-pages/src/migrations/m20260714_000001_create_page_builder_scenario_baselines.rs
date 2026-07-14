use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PageBuilderScenarioBaselines::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselines::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselines::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselines::PageId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselines::BaselineId)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselines::BaselineHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselines::SourceProjectHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselines::Baseline)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselines::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselines::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_page_builder_scenario_baselines_page")
                            .from(
                                PageBuilderScenarioBaselines::Table,
                                PageBuilderScenarioBaselines::PageId,
                            )
                            .to(Pages::Table, Pages::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_page_builder_scenario_baselines_tenant_page")
                    .table(PageBuilderScenarioBaselines::Table)
                    .col(PageBuilderScenarioBaselines::TenantId)
                    .col(PageBuilderScenarioBaselines::PageId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_page_builder_scenario_baselines_hash")
                    .table(PageBuilderScenarioBaselines::Table)
                    .col(PageBuilderScenarioBaselines::TenantId)
                    .col(PageBuilderScenarioBaselines::BaselineHash)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PageBuilderScenarioBaselines::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PageBuilderScenarioBaselines {
    Table,
    Id,
    TenantId,
    PageId,
    BaselineId,
    BaselineHash,
    SourceProjectHash,
    Baseline,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Pages {
    Table,
    Id,
}
