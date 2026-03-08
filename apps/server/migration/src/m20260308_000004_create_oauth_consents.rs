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
                    .table(OAuthConsents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OAuthConsents::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OAuthConsents::AppId).uuid().not_null())
                    .col(ColumnDef::new(OAuthConsents::UserId).uuid().not_null())
                    .col(ColumnDef::new(OAuthConsents::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(OAuthConsents::Scopes)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthConsents::GrantedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(OAuthConsents::RevokedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_consents_app_id")
                            .from(OAuthConsents::Table, OAuthConsents::AppId)
                            .to(OAuthApps::Table, OAuthApps::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_consents_user_id")
                            .from(OAuthConsents::Table, OAuthConsents::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_consents_tenant_id")
                            .from(OAuthConsents::Table, OAuthConsents::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_consents_unique")
                    .table(OAuthConsents::Table)
                    .col(OAuthConsents::AppId)
                    .col(OAuthConsents::UserId)
                    .col(OAuthConsents::TenantId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OAuthConsents::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum OAuthConsents {
    Table,
    Id,
    AppId,
    UserId,
    TenantId,
    Scopes,
    GrantedAt,
    RevokedAt,
}
