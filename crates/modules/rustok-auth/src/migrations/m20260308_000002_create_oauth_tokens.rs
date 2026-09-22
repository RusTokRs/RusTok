use super::shared::*;
use sea_orm_migration::prelude::*;

use super::m20260308_000001_create_oauth_apps::OAuthApps;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OAuthTokens::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OAuthTokens::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OAuthTokens::AppId).uuid().not_null())
                    .col(ColumnDef::new(OAuthTokens::UserId).uuid())
                    .col(ColumnDef::new(OAuthTokens::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(OAuthTokens::TokenHash)
                            .string_len(255)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(OAuthTokens::GrantType)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(ColumnDef::new(OAuthTokens::Scopes).json_binary().not_null())
                    .col(
                        ColumnDef::new(OAuthTokens::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OAuthTokens::RevokedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(OAuthTokens::LastUsedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(OAuthTokens::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(OAuthTokens::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_tokens_app_id")
                            .from(OAuthTokens::Table, OAuthTokens::AppId)
                            .to(OAuthApps::Table, OAuthApps::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_tokens_tenant_id")
                            .from(OAuthTokens::Table, OAuthTokens::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Partial index on token_hash for active (non-revoked) tokens
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_tokens_hash")
                    .table(OAuthTokens::Table)
                    .col(OAuthTokens::TokenHash)
                    .unique()
                    .and_where(Expr::col((OAuthTokens::Table, OAuthTokens::RevokedAt)).is_null())
                    .to_owned(),
            )
            .await?;

        // Partial index on (app_id, tenant_id) for active tokens
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_tokens_app_tenant")
                    .table(OAuthTokens::Table)
                    .col(OAuthTokens::AppId)
                    .col(OAuthTokens::TenantId)
                    .and_where(Expr::col((OAuthTokens::Table, OAuthTokens::RevokedAt)).is_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OAuthTokens::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum OAuthTokens {
    #[iden = "oauth_tokens"]
    Table,
    Id,
    AppId,
    UserId,
    TenantId,
    TokenHash,
    GrantType,
    Scopes,
    ExpiresAt,
    RevokedAt,
    LastUsedAt,
    CreatedAt,
    UpdatedAt,
}
