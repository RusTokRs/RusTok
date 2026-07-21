use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx_menus_tenant_location_unique")
                    .table(Menus::Table)
                    .col(Menus::TenantId)
                    .col(Menus::Location)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_menus_tenant_location_unique")
                    .table(Menus::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Menus {
    Table,
    TenantId,
    Location,
}
