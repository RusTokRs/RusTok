use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Pages::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Pages::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Pages::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Pages::AuthorId).uuid())
                    .col(
                        ColumnDef::new(Pages::Status)
                            .string_len(32)
                            .not_null()
                            .default("draft"),
                    )
                    .col(ColumnDef::new(Pages::Template).string_len(128).not_null())
                    .col(
                        ColumnDef::new(Pages::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Pages::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Pages::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Pages::PublishedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Pages::ArchivedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Pages::Version)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_pages_tenant_status_template")
                    .table(Pages::Table)
                    .col(Pages::TenantId)
                    .col(Pages::Status)
                    .col(Pages::Template)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PageTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PageTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PageTranslations::PageId).uuid().not_null())
                    .col(ColumnDef::new(PageTranslations::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(PageTranslations::Locale)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PageTranslations::Title).text().not_null())
                    .col(
                        ColumnDef::new(PageTranslations::Slug)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PageTranslations::MetaTitle).text())
                    .col(ColumnDef::new(PageTranslations::MetaDescription).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_page_translations_page")
                            .from(PageTranslations::Table, PageTranslations::PageId)
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
                    .name("idx_page_translations_page_locale")
                    .table(PageTranslations::Table)
                    .col(PageTranslations::PageId)
                    .col(PageTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_page_translations_tenant_locale_slug")
                    .table(PageTranslations::Table)
                    .col(PageTranslations::TenantId)
                    .col(PageTranslations::Locale)
                    .col(PageTranslations::Slug)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PageBodies::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PageBodies::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PageBodies::PageId).uuid().not_null())
                    .col(ColumnDef::new(PageBodies::Locale).string_len(16).not_null())
                    .col(ColumnDef::new(PageBodies::Content).text().not_null())
                    .col(ColumnDef::new(PageBodies::Format).string_len(32).not_null())
                    .col(
                        ColumnDef::new(PageBodies::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_page_bodies_page")
                            .from(PageBodies::Table, PageBodies::PageId)
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
                    .name("idx_page_bodies_page_locale")
                    .table(PageBodies::Table)
                    .col(PageBodies::PageId)
                    .col(PageBodies::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Menus::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Menus::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Menus::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Menus::Location).string_len(32).not_null())
                    .col(
                        ColumnDef::new(Menus::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Menus::UpdatedAt)
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
                    .name("idx_menus_tenant_location")
                    .table(Menus::Table)
                    .col(Menus::TenantId)
                    .col(Menus::Location)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MenuTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MenuTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MenuTranslations::MenuId).uuid().not_null())
                    .col(
                        ColumnDef::new(MenuTranslations::Locale)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MenuTranslations::Name).text().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_menu_translations_menu")
                            .from(MenuTranslations::Table, MenuTranslations::MenuId)
                            .to(Menus::Table, Menus::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_menu_translations_menu_locale")
                    .table(MenuTranslations::Table)
                    .col(MenuTranslations::MenuId)
                    .col(MenuTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MenuItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MenuItems::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MenuItems::MenuId).uuid().not_null())
                    .col(ColumnDef::new(MenuItems::TenantId).uuid().not_null())
                    .col(ColumnDef::new(MenuItems::ParentItemId).uuid())
                    .col(ColumnDef::new(MenuItems::PageId).uuid())
                    .col(ColumnDef::new(MenuItems::Position).integer().not_null())
                    .col(ColumnDef::new(MenuItems::Url).string_len(2048).not_null())
                    .col(ColumnDef::new(MenuItems::Icon).string_len(255))
                    .col(
                        ColumnDef::new(MenuItems::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MenuItems::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_menu_items_menu")
                            .from(MenuItems::Table, MenuItems::MenuId)
                            .to(Menus::Table, Menus::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_menu_items_parent")
                            .from(MenuItems::Table, MenuItems::ParentItemId)
                            .to(MenuItems::Table, MenuItems::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_menu_items_menu_parent_position")
                    .table(MenuItems::Table)
                    .col(MenuItems::MenuId)
                    .col(MenuItems::ParentItemId)
                    .col(MenuItems::Position)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MenuItemTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MenuItemTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MenuItemTranslations::MenuItemId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MenuItemTranslations::Locale)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MenuItemTranslations::Title)
                            .text()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_menu_item_translations_menu_item")
                            .from(
                                MenuItemTranslations::Table,
                                MenuItemTranslations::MenuItemId,
                            )
                            .to(MenuItems::Table, MenuItems::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_menu_item_translations_item_locale")
                    .table(MenuItemTranslations::Table)
                    .col(MenuItemTranslations::MenuItemId)
                    .col(MenuItemTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MenuItemTranslations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(MenuItems::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(MenuTranslations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Menus::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PageBodies::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PageTranslations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Pages::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Pages {
    Table,
    Id,
    TenantId,
    AuthorId,
    Status,
    Template,
    Metadata,
    CreatedAt,
    UpdatedAt,
    PublishedAt,
    ArchivedAt,
    Version,
}

#[derive(DeriveIden)]
enum PageTranslations {
    Table,
    Id,
    PageId,
    TenantId,
    Locale,
    Title,
    Slug,
    MetaTitle,
    MetaDescription,
}

#[derive(DeriveIden)]
enum PageBodies {
    Table,
    Id,
    PageId,
    Locale,
    Content,
    Format,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Menus {
    Table,
    Id,
    TenantId,
    Location,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum MenuTranslations {
    Table,
    Id,
    MenuId,
    Locale,
    Name,
}

#[derive(DeriveIden)]
enum MenuItems {
    Table,
    Id,
    MenuId,
    TenantId,
    ParentItemId,
    PageId,
    Position,
    Url,
    Icon,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum MenuItemTranslations {
    Table,
    Id,
    MenuItemId,
    Locale,
    Title,
}
