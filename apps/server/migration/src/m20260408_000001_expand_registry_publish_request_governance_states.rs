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
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
