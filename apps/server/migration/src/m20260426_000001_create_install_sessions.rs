use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(InstallSessions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InstallSessions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(InstallSessions::TenantId).uuid().null())
                    .col(
                        ColumnDef::new(InstallSessions::Status)
                            .string_len(32)
                            .not_null()
                            .default("draft"),
                    )
                    .col(
                        ColumnDef::new(InstallSessions::Profile)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InstallSessions::Environment)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InstallSessions::DatabaseEngine)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InstallSessions::SeedProfile)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InstallSessions::PlanSnapshot)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(ColumnDef::new(InstallSessions::LockOwner).string_len(128))
                    .col(ColumnDef::new(InstallSessions::LockExpiresAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(InstallSessions::ErrorMessage).text())
                    .col(ColumnDef::new(InstallSessions::CreatedBy).uuid().null())
                    .col(
                        ColumnDef::new(InstallSessions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(InstallSessions::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(InstallSessions::CompletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(InstallSessions::Table, InstallSessions::TenantId)
                            .to(Alias::new("tenants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_install_sessions_status")
                    .table(InstallSessions::Table)
                    .col(InstallSessions::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_install_sessions_tenant_status")
                    .table(InstallSessions::Table)
                    .col(InstallSessions::TenantId)
                    .col(InstallSessions::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(InstallStepReceipts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InstallStepReceipts::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(InstallStepReceipts::SessionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InstallStepReceipts::Step)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InstallStepReceipts::Outcome)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InstallStepReceipts::InputChecksum)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InstallStepReceipts::Diagnostics)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(InstallStepReceipts::InstallerVersion)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InstallStepReceipts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(InstallStepReceipts::Table, InstallStepReceipts::SessionId)
                            .to(InstallSessions::Table, InstallSessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_install_step_receipts_session_step")
                    .table(InstallStepReceipts::Table)
                    .col(InstallStepReceipts::SessionId)
                    .col(InstallStepReceipts::Step)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_install_step_receipts_idempotency")
                    .table(InstallStepReceipts::Table)
                    .col(InstallStepReceipts::SessionId)
                    .col(InstallStepReceipts::Step)
                    .col(InstallStepReceipts::InputChecksum)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(InstallStepReceipts::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(InstallSessions::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum InstallSessions {
    Table,
    Id,
    TenantId,
    Status,
    Profile,
    Environment,
    DatabaseEngine,
    SeedProfile,
    PlanSnapshot,
    LockOwner,
    LockExpiresAt,
    ErrorMessage,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(Iden)]
enum InstallStepReceipts {
    Table,
    Id,
    SessionId,
    Step,
    Outcome,
    InputChecksum,
    Diagnostics,
    InstallerVersion,
    CreatedAt,
}
