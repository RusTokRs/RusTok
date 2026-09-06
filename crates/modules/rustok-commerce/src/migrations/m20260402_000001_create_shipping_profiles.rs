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
                    .table(ShippingProfiles::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ShippingProfiles::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ShippingProfiles::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(ShippingProfiles::Slug)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ShippingProfiles::Name)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ShippingProfiles::Description).text())
                    .col(
                        ColumnDef::new(ShippingProfiles::Active)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(ShippingProfiles::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(ShippingProfiles::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ShippingProfiles::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ShippingProfiles::Table, ShippingProfiles::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_shipping_profiles_tenant_slug_unique")
                    .table(ShippingProfiles::Table)
                    .col(ShippingProfiles::TenantId)
                    .col(ShippingProfiles::Slug)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_shipping_profiles_tenant_active")
                    .table(ShippingProfiles::Table)
                    .col(ShippingProfiles::TenantId)
                    .col(ShippingProfiles::Active)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ShippingProfiles::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum ShippingProfiles {
    Table,
    Id,
    TenantId,
    Slug,
    Name,
    Description,
    Active,
    Metadata,
    CreatedAt,
    UpdatedAt,
}
