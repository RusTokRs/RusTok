use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(RegistryPublishRequests::Table)
                    .add_column(
                        ColumnDef::new(RegistryPublishRequests::ChangesRequestedBy)
                            .string_len(128)
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(RegistryPublishRequests::ChangesRequestedReason)
                            .text()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(RegistryPublishRequests::ChangesRequestedReasonCode)
                            .string_len(64)
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(RegistryPublishRequests::ChangesRequestedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(RegistryPublishRequests::HeldBy)
                            .string_len(128)
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(RegistryPublishRequests::HeldReason)
                            .text()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(RegistryPublishRequests::HeldReasonCode)
                            .string_len(64)
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(RegistryPublishRequests::HeldAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(RegistryPublishRequests::HeldFromStatus)
                            .string_len(32)
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(RegistryValidationStages::Table)
                    .add_column(
                        ColumnDef::new(RegistryValidationStages::ClaimId)
                            .string_len(64)
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(RegistryValidationStages::ClaimedBy)
                            .string_len(128)
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(RegistryValidationStages::ClaimExpiresAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(RegistryValidationStages::LastHeartbeatAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(RegistryValidationStages::RunnerKind)
                            .string_len(32)
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_registry_validation_stages_status_claim_expiry")
                    .table(RegistryValidationStages::Table)
                    .col(RegistryValidationStages::Status)
                    .col(RegistryValidationStages::ClaimExpiresAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_registry_validation_stages_claim_id")
                    .table(RegistryValidationStages::Table)
                    .col(RegistryValidationStages::ClaimId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_registry_validation_stages_claim_id")
                    .table(RegistryValidationStages::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_registry_validation_stages_status_claim_expiry")
                    .table(RegistryValidationStages::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(RegistryValidationStages::Table)
                    .drop_column(RegistryValidationStages::ClaimId)
                    .drop_column(RegistryValidationStages::ClaimedBy)
                    .drop_column(RegistryValidationStages::ClaimExpiresAt)
                    .drop_column(RegistryValidationStages::LastHeartbeatAt)
                    .drop_column(RegistryValidationStages::RunnerKind)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(RegistryPublishRequests::Table)
                    .drop_column(RegistryPublishRequests::ChangesRequestedBy)
                    .drop_column(RegistryPublishRequests::ChangesRequestedReason)
                    .drop_column(RegistryPublishRequests::ChangesRequestedReasonCode)
                    .drop_column(RegistryPublishRequests::ChangesRequestedAt)
                    .drop_column(RegistryPublishRequests::HeldBy)
                    .drop_column(RegistryPublishRequests::HeldReason)
                    .drop_column(RegistryPublishRequests::HeldReasonCode)
                    .drop_column(RegistryPublishRequests::HeldAt)
                    .drop_column(RegistryPublishRequests::HeldFromStatus)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum RegistryPublishRequests {
    Table,
    ChangesRequestedBy,
    ChangesRequestedReason,
    ChangesRequestedReasonCode,
    ChangesRequestedAt,
    HeldBy,
    HeldReason,
    HeldReasonCode,
    HeldAt,
    HeldFromStatus,
}

#[derive(DeriveIden)]
enum RegistryValidationStages {
    Table,
    Status,
    ClaimId,
    ClaimedBy,
    ClaimExpiresAt,
    LastHeartbeatAt,
    RunnerKind,
}
