use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Tenants::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Tenants::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Tenants::Name).string_len(255).not_null())
                    .col(
                        ColumnDef::new(Tenants::Slug)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Tenants::Domain)
                            .string_len(255)
                            .null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Tenants::Settings)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Tenants::DefaultLocale)
                            .string_len(5)
                            .not_null()
                            .default("en"),
                    )
                    .col(
                        ColumnDef::new(Tenants::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Tenants::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Tenants::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tenants_slug")
                    .table(Tenants::Table)
                    .col(Tenants::Slug)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tenants_domain")
                    .table(Tenants::Table)
                    .col(Tenants::Domain)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Tenants::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Tenants {
    Table,
    Id,
    Name,
    Slug,
    Domain,
    Settings,
    DefaultLocale,
    Metadata,
    IsActive,
    CreatedAt,
    UpdatedAt,
}
