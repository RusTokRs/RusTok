use sea_orm_migration::prelude::*;

use super::shared::{Tenants, Users};

/// One active, one-time mailbox-change challenge per user and target address.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuthEmailChangeRequests::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AuthEmailChangeRequests::TenantId).uuid().not_null())
                    .col(ColumnDef::new(AuthEmailChangeRequests::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::CurrentEmail)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::NewEmail)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::PasswordChangedAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::TokenHash)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_auth_email_change_requests_tenant")
                            .from(AuthEmailChangeRequests::Table, AuthEmailChangeRequests::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_auth_email_change_requests_user")
                            .from(AuthEmailChangeRequests::Table, AuthEmailChangeRequests::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_auth_email_change_requests_tenant_user")
                    .table(AuthEmailChangeRequests::Table)
                    .col(AuthEmailChangeRequests::TenantId)
                    .col(AuthEmailChangeRequests::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_auth_email_change_requests_tenant_email")
                    .table(AuthEmailChangeRequests::Table)
                    .col(AuthEmailChangeRequests::TenantId)
                    .col(AuthEmailChangeRequests::NewEmail)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_auth_email_change_requests_expires_at")
                    .table(AuthEmailChangeRequests::Table)
                    .col(AuthEmailChangeRequests::ExpiresAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuthEmailChangeRequests::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum AuthEmailChangeRequests {
    #[iden = "auth_email_change_requests"]
    Table,
    Id,
    TenantId,
    UserId,
    CurrentEmail,
    NewEmail,
    PasswordChangedAt,
    TokenHash,
    ExpiresAt,
    CreatedAt,
}
