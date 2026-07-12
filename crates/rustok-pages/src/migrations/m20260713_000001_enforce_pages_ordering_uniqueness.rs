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
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_page_blocks_page_position_unique")
                    .table(PageBlocks::Table)
                    .col(PageBlocks::PageId)
                    .col(PageBlocks::Position)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_page_blocks_page_position_unique")
                    .table(PageBlocks::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_menus_tenant_location_unique")
                    .table(Menus::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Menus {
    Table,
    TenantId,
    Location,
}

#[derive(DeriveIden)]
enum PageBlocks {
    Table,
    PageId,
    Position,
}
