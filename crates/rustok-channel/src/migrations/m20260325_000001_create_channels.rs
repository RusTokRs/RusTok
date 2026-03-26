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
                    .table(Channels::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Channels::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Channels::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Channels::Slug).string_len(100).not_null())
                    .col(ColumnDef::new(Channels::Name).string_len(255).not_null())
                    .col(
                        ColumnDef::new(Channels::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Channels::Status)
                            .string_len(50)
                            .not_null()
                            .default("experimental"),
                    )
                    .col(
                        ColumnDef::new(Channels::Settings)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Channels::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Channels::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_channels_tenant_id")
                            .from(Channels::Table, Channels::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_channels_tenant_slug")
                    .table(Channels::Table)
                    .col(Channels::TenantId)
                    .col(Channels::Slug)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Channels::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Channels {
    Table,
    Id,
    TenantId,
    Slug,
    Name,
    IsActive,
    Status,
    Settings,
    CreatedAt,
    UpdatedAt,
}
