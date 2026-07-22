use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(GroupAuditEntries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GroupAuditEntries::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(GroupAuditEntries::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(GroupAuditEntries::GroupId).uuid().not_null())
                    .col(ColumnDef::new(GroupAuditEntries::ActorUserId).uuid())
                    .col(
                        ColumnDef::new(GroupAuditEntries::Action)
                            .string_len(80)
                            .not_null(),
                    )
                    .col(ColumnDef::new(GroupAuditEntries::TargetUserId).uuid())
                    .col(
                        ColumnDef::new(GroupAuditEntries::Details)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(GroupAuditEntries::CorrelationId)
                            .string_len(160)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupAuditEntries::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_group_audit_entries_tenant_group")
                            .from(GroupAuditEntries::Table, GroupAuditEntries::TenantId)
                            .from_col(GroupAuditEntries::GroupId)
                            .to(Groups::Table, Groups::TenantId)
                            .to_col(Groups::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("idx_group_audit_entries_tenant_group_created")
                .table(GroupAuditEntries::Table)
                .col(GroupAuditEntries::TenantId)
                .col(GroupAuditEntries::GroupId)
                .col(GroupAuditEntries::CreatedAt)
                .to_owned(),
            Index::create()
                .name("idx_group_audit_entries_tenant_action")
                .table(GroupAuditEntries::Table)
                .col(GroupAuditEntries::TenantId)
                .col(GroupAuditEntries::Action)
                .col(GroupAuditEntries::CreatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(GroupCommandReceipts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GroupCommandReceipts::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(GroupCommandReceipts::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupCommandReceipts::GroupId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupCommandReceipts::ActorUserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupCommandReceipts::IdempotencyKey)
                            .string_len(160)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupCommandReceipts::CommandType)
                            .string_len(80)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupCommandReceipts::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupCommandReceipts::Response)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupCommandReceipts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_group_command_receipts_tenant_group")
                            .from(GroupCommandReceipts::Table, GroupCommandReceipts::TenantId)
                            .from_col(GroupCommandReceipts::GroupId)
                            .to(Groups::Table, Groups::TenantId)
                            .to_col(Groups::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("ux_group_command_receipts_tenant_key")
                .table(GroupCommandReceipts::Table)
                .col(GroupCommandReceipts::TenantId)
                .col(GroupCommandReceipts::IdempotencyKey)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_group_command_receipts_tenant_group_created")
                .table(GroupCommandReceipts::Table)
                .col(GroupCommandReceipts::TenantId)
                .col(GroupCommandReceipts::GroupId)
                .col(GroupCommandReceipts::CreatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(GroupCommandReceipts::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(GroupAuditEntries::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Groups {
    Table,
    Id,
    TenantId,
}

#[derive(DeriveIden)]
enum GroupAuditEntries {
    Table,
    Id,
    TenantId,
    GroupId,
    ActorUserId,
    Action,
    TargetUserId,
    Details,
    CorrelationId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum GroupCommandReceipts {
    Table,
    Id,
    TenantId,
    GroupId,
    ActorUserId,
    IdempotencyKey,
    CommandType,
    RequestHash,
    Response,
    CreatedAt,
}
