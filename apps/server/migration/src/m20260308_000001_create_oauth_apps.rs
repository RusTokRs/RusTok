use sea_orm_migration::prelude::*;

use super::m20250101_000001_create_tenants::Tenants;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OAuthApps::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OAuthApps::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OAuthApps::TenantId).uuid().not_null())
                    // Identification
                    .col(ColumnDef::new(OAuthApps::Name).string_len(255).not_null())
                    .col(ColumnDef::new(OAuthApps::Slug).string_len(100).not_null())
                    .col(ColumnDef::new(OAuthApps::Description).text())
                    .col(ColumnDef::new(OAuthApps::AppType).string_len(50).not_null())
                    .col(ColumnDef::new(OAuthApps::IconUrl).string_len(500))
                    // Credentials
                    .col(
                        ColumnDef::new(OAuthApps::ClientId)
                            .uuid()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(OAuthApps::ClientSecretHash).string_len(255))
                    // OAuth2 config
                    .col(
                        ColumnDef::new(OAuthApps::RedirectUris)
                            .json_binary()
                            .not_null()
                            .default("[]"),
                    )
                    .col(
                        ColumnDef::new(OAuthApps::Scopes)
                            .json_binary()
                            .not_null()
                            .default("[]"),
                    )
                    .col(
                        ColumnDef::new(OAuthApps::GrantTypes)
                            .json_binary()
                            .not_null()
                            .default("[]"),
                    )
                    // Manifest link
                    .col(ColumnDef::new(OAuthApps::ManifestRef).string_len(100))
                    .col(
                        ColumnDef::new(OAuthApps::AutoCreated)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    // Status
                    .col(
                        ColumnDef::new(OAuthApps::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(ColumnDef::new(OAuthApps::RevokedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(OAuthApps::LastUsedAt).timestamp_with_time_zone())
                    // Metadata
                    .col(
                        ColumnDef::new(OAuthApps::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(OAuthApps::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(OAuthApps::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_apps_tenant_id")
                            .from(OAuthApps::Table, OAuthApps::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Index on client_id for token lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_apps_client_id")
                    .table(OAuthApps::Table)
                    .col(OAuthApps::ClientId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index on tenant + active for listing
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_apps_tenant_active")
                    .table(OAuthApps::Table)
                    .col(OAuthApps::TenantId)
                    .col(OAuthApps::IsActive)
                    .to_owned(),
            )
            .await?;

        // Unique constraint: (tenant_id, slug)
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_apps_tenant_slug")
                    .table(OAuthApps::Table)
                    .col(OAuthApps::TenantId)
                    .col(OAuthApps::Slug)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OAuthApps::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum OAuthApps {
    Table,
    Id,
    TenantId,
    Name,
    Slug,
    Description,
    AppType,
    IconUrl,
    ClientId,
    ClientSecretHash,
    RedirectUris,
    Scopes,
    GrantTypes,
    ManifestRef,
    AutoCreated,
    IsActive,
    RevokedAt,
    LastUsedAt,
    Metadata,
    CreatedAt,
    UpdatedAt,
}
