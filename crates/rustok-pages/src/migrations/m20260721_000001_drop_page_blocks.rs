use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PageBlocks::Table)
                    .if_exists()
                    .cascade()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PageBlocks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PageBlocks::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PageBlocks::PageId).uuid().not_null())
                    .col(ColumnDef::new(PageBlocks::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(PageBlocks::BlockType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PageBlocks::Position).integer().not_null())
                    .col(ColumnDef::new(PageBlocks::Data).json_binary().not_null())
                    .col(ColumnDef::new(PageBlocks::Translations).json_binary())
                    .col(
                        ColumnDef::new(PageBlocks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PageBlocks::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_page_blocks_page")
                            .from(PageBlocks::Table, PageBlocks::PageId)
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
                    .name("idx_page_blocks_page_position")
                    .table(PageBlocks::Table)
                    .col(PageBlocks::PageId)
                    .col(PageBlocks::Position)
                    .unique()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Pages {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum PageBlocks {
    Table,
    Id,
    PageId,
    TenantId,
    BlockType,
    Position,
    Data,
    Translations,
    CreatedAt,
    UpdatedAt,
}
