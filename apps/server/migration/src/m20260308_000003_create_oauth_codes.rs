use sea_orm_migration::prelude::*;

use super::m20250101_000001_create_tenants::Tenants;
use super::m20250101_000002_create_users::Users;
use super::m20260308_000001_create_oauth_apps::OAuthApps;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OAuthAuthorizationCodes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OAuthAuthorizationCodes::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(OAuthAuthorizationCodes::AppId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthAuthorizationCodes::UserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthAuthorizationCodes::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthAuthorizationCodes::CodeHash)
                            .string_len(255)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(OAuthAuthorizationCodes::RedirectUri)
                            .string_len(500)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthAuthorizationCodes::Scopes)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthAuthorizationCodes::CodeChallenge)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthAuthorizationCodes::CodeChallengeMethod)
                            .string_len(10)
                            .not_null()
                            .default("S256"),
                    )
                    .col(
                        ColumnDef::new(OAuthAuthorizationCodes::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OAuthAuthorizationCodes::UsedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(OAuthAuthorizationCodes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_codes_app_id")
                            .from(
                                OAuthAuthorizationCodes::Table,
                                OAuthAuthorizationCodes::AppId,
                            )
                            .to(OAuthApps::Table, OAuthApps::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_codes_user_id")
                            .from(
                                OAuthAuthorizationCodes::Table,
                                OAuthAuthorizationCodes::UserId,
                            )
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_codes_tenant_id")
                            .from(
                                OAuthAuthorizationCodes::Table,
                                OAuthAuthorizationCodes::TenantId,
                            )
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for looking up the code during exchange
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_codes_hash")
                    .table(OAuthAuthorizationCodes::Table)
                    .col(OAuthAuthorizationCodes::CodeHash)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(OAuthAuthorizationCodes::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum OAuthAuthorizationCodes {
    Table,
    Id,
    AppId,
    UserId,
    TenantId,
    CodeHash,
    RedirectUri,
    Scopes,
    CodeChallenge,
    CodeChallengeMethod,
    ExpiresAt,
    UsedAt,
    CreatedAt,
}
