use super::shared::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuthInviteConsumptions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuthInviteConsumptions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AuthInviteConsumptions::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuthInviteConsumptions::TokenHash)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(AuthInviteConsumptions::Email)
                            .string_len(320)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuthInviteConsumptions::Role)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(AuthInviteConsumptions::UserId).uuid())
                    .col(
                        ColumnDef::new(AuthInviteConsumptions::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuthInviteConsumptions::ConsumedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_auth_invite_consumptions_tenant")
                            .from(
                                AuthInviteConsumptions::Table,
                                AuthInviteConsumptions::TenantId,
                            )
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_auth_invite_consumptions_user")
                            .from(
                                AuthInviteConsumptions::Table,
                                AuthInviteConsumptions::UserId,
                            )
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_auth_invite_consumptions_tenant_consumed")
                    .table(AuthInviteConsumptions::Table)
                    .col(AuthInviteConsumptions::TenantId)
                    .col(AuthInviteConsumptions::ConsumedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(AuthInviteConsumptions::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum AuthInviteConsumptions {
    #[iden = "auth_invite_consumptions"]
    Table,
    Id,
    TenantId,
    TokenHash,
    Email,
    Role,
    UserId,
    ExpiresAt,
    ConsumedAt,
}
