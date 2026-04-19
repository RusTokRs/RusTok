use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            for statement in [
                Table::alter()
                    .table(RegistryValidationStages::Table)
                    .add_column(
                        ColumnDef::new(RegistryValidationStages::ClaimId)
                            .string_len(64)
                            .null(),
                    )
                    .to_owned(),
                Table::alter()
                    .table(RegistryValidationStages::Table)
                    .add_column(
                        ColumnDef::new(RegistryValidationStages::ClaimedBy)
                            .string_len(128)
                            .null(),
                    )
                    .to_owned(),
                Table::alter()
                    .table(RegistryValidationStages::Table)
                    .add_column(
                        ColumnDef::new(RegistryValidationStages::ClaimExpiresAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
                Table::alter()
                    .table(RegistryValidationStages::Table)
                    .add_column(
                        ColumnDef::new(RegistryValidationStages::LastHeartbeatAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
                Table::alter()
                    .table(RegistryValidationStages::Table)
                    .add_column(
                        ColumnDef::new(RegistryValidationStages::RunnerKind)
                            .string_len(32)
                            .null(),
                    )
                    .to_owned(),
            ] {
                manager.alter_table(statement).await?;
            }
        } else {
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
        }

        manager
            .create_index(
                Index::create()
                    .name("idx_registry_validation_stages_claim_id")
                    .table(RegistryValidationStages::Table)
                    .col(RegistryValidationStages::ClaimId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_registry_validation_stages_remote_leases")
                    .table(RegistryValidationStages::Table)
                    .col(RegistryValidationStages::RunnerKind)
                    .col(RegistryValidationStages::ClaimExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_registry_validation_stages_remote_leases")
                    .table(RegistryValidationStages::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_registry_validation_stages_claim_id")
                    .table(RegistryValidationStages::Table)
                    .to_owned(),
            )
            .await?;
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            for statement in [
                Table::alter()
                    .table(RegistryValidationStages::Table)
                    .drop_column(RegistryValidationStages::RunnerKind)
                    .to_owned(),
                Table::alter()
                    .table(RegistryValidationStages::Table)
                    .drop_column(RegistryValidationStages::LastHeartbeatAt)
                    .to_owned(),
                Table::alter()
                    .table(RegistryValidationStages::Table)
                    .drop_column(RegistryValidationStages::ClaimExpiresAt)
                    .to_owned(),
                Table::alter()
                    .table(RegistryValidationStages::Table)
                    .drop_column(RegistryValidationStages::ClaimedBy)
                    .to_owned(),
                Table::alter()
                    .table(RegistryValidationStages::Table)
                    .drop_column(RegistryValidationStages::ClaimId)
                    .to_owned(),
            ] {
                manager.alter_table(statement).await?;
            }
            Ok(())
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(RegistryValidationStages::Table)
                        .drop_column(RegistryValidationStages::RunnerKind)
                        .drop_column(RegistryValidationStages::LastHeartbeatAt)
                        .drop_column(RegistryValidationStages::ClaimExpiresAt)
                        .drop_column(RegistryValidationStages::ClaimedBy)
                        .drop_column(RegistryValidationStages::ClaimId)
                        .to_owned(),
                )
                .await
        }
    }
}

#[derive(DeriveIden)]
enum RegistryValidationStages {
    Table,
    ClaimId,
    ClaimedBy,
    ClaimExpiresAt,
    LastHeartbeatAt,
    RunnerKind,
}
