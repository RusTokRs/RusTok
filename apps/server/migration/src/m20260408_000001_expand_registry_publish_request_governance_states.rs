use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sqlite = manager.get_database_backend() == DatabaseBackend::Sqlite;
        let alter_statements = [
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::ChangesRequestedBy)
                        .string_len(128)
                        .null(),
                )
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::ChangesRequestedReason)
                        .text()
                        .null(),
                )
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::ChangesRequestedReasonCode)
                        .string_len(64)
                        .null(),
                )
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::ChangesRequestedAt)
                        .timestamp_with_time_zone()
                        .null(),
                )
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::HeldBy)
                        .string_len(128)
                        .null(),
                )
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::HeldReason)
                        .text()
                        .null(),
                )
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::HeldReasonCode)
                        .string_len(64)
                        .null(),
                )
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::HeldAt)
                        .timestamp_with_time_zone()
                        .null(),
                )
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::HeldFromStatus)
                        .string_len(32)
                        .null(),
                )
                .to_owned(),
        ];

        if sqlite {
            for statement in alter_statements {
                manager.alter_table(statement).await?;
            }
            Ok(())
        } else {
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
                .await
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sqlite = manager.get_database_backend() == DatabaseBackend::Sqlite;
        let alter_statements = [
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .drop_column(RegistryPublishRequests::HeldFromStatus)
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .drop_column(RegistryPublishRequests::HeldAt)
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .drop_column(RegistryPublishRequests::HeldReasonCode)
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .drop_column(RegistryPublishRequests::HeldReason)
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .drop_column(RegistryPublishRequests::HeldBy)
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .drop_column(RegistryPublishRequests::ChangesRequestedAt)
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .drop_column(RegistryPublishRequests::ChangesRequestedReasonCode)
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .drop_column(RegistryPublishRequests::ChangesRequestedReason)
                .to_owned(),
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .drop_column(RegistryPublishRequests::ChangesRequestedBy)
                .to_owned(),
        ];

        if sqlite {
            for statement in alter_statements {
                manager.alter_table(statement).await?;
            }
            Ok(())
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(RegistryPublishRequests::Table)
                        .drop_column(RegistryPublishRequests::HeldFromStatus)
                        .drop_column(RegistryPublishRequests::HeldAt)
                        .drop_column(RegistryPublishRequests::HeldReasonCode)
                        .drop_column(RegistryPublishRequests::HeldReason)
                        .drop_column(RegistryPublishRequests::HeldBy)
                        .drop_column(RegistryPublishRequests::ChangesRequestedAt)
                        .drop_column(RegistryPublishRequests::ChangesRequestedReasonCode)
                        .drop_column(RegistryPublishRequests::ChangesRequestedReason)
                        .drop_column(RegistryPublishRequests::ChangesRequestedBy)
                        .to_owned(),
                )
                .await
        }
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
