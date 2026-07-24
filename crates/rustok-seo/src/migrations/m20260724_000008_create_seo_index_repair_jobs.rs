use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SeoIndexRepairJobs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SeoIndexRepairJobs::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SeoIndexRepairJobs::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(SeoIndexRepairJobs::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(SeoIndexRepairJobs::TargetType).string_len(64))
                    .col(
                        ColumnDef::new(SeoIndexRepairJobs::Limit)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeoIndexRepairJobs::ReplayHistorical)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SeoIndexRepairJobs::RepairedCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(SeoIndexRepairJobs::ReplayedCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(SeoIndexRepairJobs::HistoricalEventsScanned)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(SeoIndexRepairJobs::ReplayRunId).uuid())
                    .col(ColumnDef::new(SeoIndexRepairJobs::LastError).string_len(2048))
                    .col(ColumnDef::new(SeoIndexRepairJobs::StartedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(SeoIndexRepairJobs::CompletedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(SeoIndexRepairJobs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeoIndexRepairJobs::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_seo_index_repair_jobs_status_created")
                    .table(SeoIndexRepairJobs::Table)
                    .col(SeoIndexRepairJobs::Status)
                    .col(SeoIndexRepairJobs::CreatedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_seo_index_repair_jobs_tenant_created")
                    .table(SeoIndexRepairJobs::Table)
                    .col(SeoIndexRepairJobs::TenantId)
                    .col(SeoIndexRepairJobs::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SeoIndexRepairJobs::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum SeoIndexRepairJobs {
    Table,
    Id,
    TenantId,
    Status,
    TargetType,
    Limit,
    ReplayHistorical,
    RepairedCount,
    ReplayedCount,
    HistoricalEventsScanned,
    ReplayRunId,
    LastError,
    StartedAt,
    CompletedAt,
    CreatedAt,
    UpdatedAt,
}
