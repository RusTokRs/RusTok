use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RegistryValidationJobs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RegistryValidationJobs::Id)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationJobs::RequestId)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationJobs::Slug)
                            .string_len(96)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationJobs::Version)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationJobs::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationJobs::TriggeredBy)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationJobs::QueueReason)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationJobs::AttemptNumber)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationJobs::StartedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationJobs::FinishedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationJobs::LastError)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationJobs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationJobs::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_registry_validation_jobs_request_id")
                            .from(
                                RegistryValidationJobs::Table,
                                RegistryValidationJobs::RequestId,
                            )
                            .to(RegistryPublishRequests::Table, RegistryPublishRequests::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_registry_validation_jobs_request_created")
                    .table(RegistryValidationJobs::Table)
                    .col(RegistryValidationJobs::RequestId)
                    .col(RegistryValidationJobs::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_registry_validation_jobs_slug_created")
                    .table(RegistryValidationJobs::Table)
                    .col(RegistryValidationJobs::Slug)
                    .col(RegistryValidationJobs::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_registry_validation_jobs_status_created")
                    .table(RegistryValidationJobs::Table)
                    .col(RegistryValidationJobs::Status)
                    .col(RegistryValidationJobs::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RegistryValidationJobs::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum RegistryValidationJobs {
    Table,
    Id,
    RequestId,
    Slug,
    Version,
    Status,
    TriggeredBy,
    QueueReason,
    AttemptNumber,
    StartedAt,
    FinishedAt,
    LastError,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum RegistryPublishRequests {
    Table,
    Id,
}
