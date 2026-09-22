use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PageChannelVisibility::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PageChannelVisibility::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PageChannelVisibility::PageId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageChannelVisibility::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageChannelVisibility::ChannelSlug)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageChannelVisibility::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_page_channel_visibility_page")
                            .from(PageChannelVisibility::Table, PageChannelVisibility::PageId)
                            .to(Pages::Table, Pages::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_page_channel_visibility_page_slug")
                    .table(PageChannelVisibility::Table)
                    .col(PageChannelVisibility::PageId)
                    .col(PageChannelVisibility::ChannelSlug)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_page_channel_visibility_tenant_slug")
                    .table(PageChannelVisibility::Table)
                    .col(PageChannelVisibility::TenantId)
                    .col(PageChannelVisibility::ChannelSlug)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PageChannelVisibility::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum PageChannelVisibility {
    Table,
    Id,
    PageId,
    TenantId,
    ChannelSlug,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Pages {
    Table,
    Id,
}
