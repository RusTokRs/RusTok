use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RegistryValidationStages::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RegistryValidationStages::Id)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::RequestId)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::Slug)
                            .string_len(96)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::Version)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::StageKey)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::TriggeredBy)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::QueueReason)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::AttemptNumber)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::Detail)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::StartedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::FinishedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::LastError)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(RegistryValidationStages::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_registry_validation_stages_request_id")
                            .from(
                                RegistryValidationStages::Table,
                                RegistryValidationStages::RequestId,
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
                    .name("idx_registry_validation_stages_request_created")
                    .table(RegistryValidationStages::Table)
                    .col(RegistryValidationStages::RequestId)
                    .col(RegistryValidationStages::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_registry_validation_stages_request_stage_attempt")
                    .table(RegistryValidationStages::Table)
                    .col(RegistryValidationStages::RequestId)
                    .col(RegistryValidationStages::StageKey)
                    .col(RegistryValidationStages::AttemptNumber)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_registry_validation_stages_slug_created")
                    .table(RegistryValidationStages::Table)
                    .col(RegistryValidationStages::Slug)
                    .col(RegistryValidationStages::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RegistryValidationStages::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum RegistryValidationStages {
    Table,
    Id,
    RequestId,
    Slug,
    Version,
    StageKey,
    Status,
    TriggeredBy,
    QueueReason,
    AttemptNumber,
    Detail,
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
