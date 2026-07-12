use sea_orm_migration::prelude::*;

use super::m20250101_000001_create_tenants::Tenants;
use super::m20250101_000002_create_users::Users;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Sessions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Sessions::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Sessions::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Sessions::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(Sessions::TokenHash)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Sessions::IpAddress).string_len(64))
                    .col(ColumnDef::new(Sessions::UserAgent).string_len(255))
                    .col(ColumnDef::new(Sessions::LastUsedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Sessions::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Sessions::RevokedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Sessions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Sessions::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sessions_tenant_id")
                            .from(Sessions::Table, Sessions::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sessions_user_id")
                            .from(Sessions::Table, Sessions::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sessions_tenant_token")
                    .table(Sessions::Table)
                    .col(Sessions::TenantId)
                    .col(Sessions::TokenHash)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sessions_user")
                    .table(Sessions::Table)
                    .col(Sessions::UserId)
                    .col(Sessions::RevokedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Sessions::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Sessions {
    Table,
    Id,
    TenantId,
    UserId,
    TokenHash,
    IpAddress,
    UserAgent,
    LastUsedAt,
    ExpiresAt,
    RevokedAt,
    CreatedAt,
    UpdatedAt,
}
