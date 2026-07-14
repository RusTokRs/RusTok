use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PageBuilderScenarioBaselineRevisions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselineRevisions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselineRevisions::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselineRevisions::PageId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselineRevisions::Operation)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselineRevisions::BaselineId)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselineRevisions::BaselineHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselineRevisions::SourceProjectHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            PageBuilderScenarioBaselineRevisions::PreviousBaselineHash,
                        )
                        .string_len(128)
                        .null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselineRevisions::Baseline)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselineRevisions::ActorId)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselineRevisions::Note)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(PageBuilderScenarioBaselineRevisions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_page_builder_scenario_baseline_revisions_page")
                            .from(
                                PageBuilderScenarioBaselineRevisions::Table,
                                PageBuilderScenarioBaselineRevisions::PageId,
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
                    .name("idx_page_builder_scenario_baseline_revisions_page_time")
                    .table(PageBuilderScenarioBaselineRevisions::Table)
                    .col(PageBuilderScenarioBaselineRevisions::TenantId)
                    .col(PageBuilderScenarioBaselineRevisions::PageId)
                    .col(PageBuilderScenarioBaselineRevisions::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_page_builder_scenario_baseline_revisions_hash")
                    .table(PageBuilderScenarioBaselineRevisions::Table)
                    .col(PageBuilderScenarioBaselineRevisions::TenantId)
                    .col(PageBuilderScenarioBaselineRevisions::BaselineHash)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PageBuilderScenarioBaselineRevisions::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PageBuilderScenarioBaselineRevisions {
    Table,
    Id,
    TenantId,
    PageId,
    Operation,
    BaselineId,
    BaselineHash,
    SourceProjectHash,
    PreviousBaselineHash,
    Baseline,
    ActorId,
    Note,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Pages {
    Table,
    Id,
}
