use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let present = [Menus::Table, MenuTranslations::Table, MenuItems::Table, MenuItemTranslations::Table, MenuBindings::Table];
        let mut count = 0usize;
        for table in &present {
            if manager.has_table(table.to_string()).await? { count += 1; }
        }
        if count == present.len() { return Ok(()); }
        if count != 0 { return Err(DbErr::Custom("navigation migration found a partial menu table set".to_string())); }

        manager.create_table(Table::create().table(Menus::Table)
            .col(ColumnDef::new(Menus::Id).uuid().not_null().primary_key())
            .col(ColumnDef::new(Menus::TenantId).uuid().not_null())
            .col(ColumnDef::new(Menus::Location).string_len(32).not_null())
            .col(ColumnDef::new(Menus::CreatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
            .col(ColumnDef::new(Menus::UpdatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp())).to_owned()).await?;
        manager.create_index(Index::create().name("uq_menus_tenant_id").table(Menus::Table).col(Menus::TenantId).col(Menus::Id).unique().to_owned()).await?;

        manager.create_table(Table::create().table(MenuTranslations::Table)
            .col(ColumnDef::new(MenuTranslations::Id).uuid().not_null().primary_key())
            .col(ColumnDef::new(MenuTranslations::TenantId).uuid().not_null())
            .col(ColumnDef::new(MenuTranslations::MenuId).uuid().not_null())
            .col(ColumnDef::new(MenuTranslations::Locale).string_len(32).not_null())
            .col(ColumnDef::new(MenuTranslations::Name).string_len(255).not_null())
            .foreign_key(ForeignKey::create().name("fk_menu_translations_tenant_menu")
                .from(MenuTranslations::Table, MenuTranslations::TenantId).from_col(MenuTranslations::MenuId)
                .to(Menus::Table, Menus::TenantId).to_col(Menus::Id).on_update(ForeignKeyAction::Cascade).on_delete(ForeignKeyAction::Cascade)).to_owned()).await?;
        manager.create_index(Index::create().name("uq_menu_translations_tenant_menu_locale").table(MenuTranslations::Table)
            .col(MenuTranslations::TenantId).col(MenuTranslations::MenuId).col(MenuTranslations::Locale).unique().to_owned()).await?;

        manager.create_table(Table::create().table(MenuItems::Table)
            .col(ColumnDef::new(MenuItems::Id).uuid().not_null().primary_key())
            .col(ColumnDef::new(MenuItems::MenuId).uuid().not_null())
            .col(ColumnDef::new(MenuItems::TenantId).uuid().not_null())
            .col(ColumnDef::new(MenuItems::ParentItemId).uuid())
            .col(ColumnDef::new(MenuItems::PageId).uuid())
            .col(ColumnDef::new(MenuItems::Position).integer().not_null())
            .col(ColumnDef::new(MenuItems::Url).string_len(2048).not_null())
            .col(ColumnDef::new(MenuItems::Icon).string_len(255))
            .col(ColumnDef::new(MenuItems::CreatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
            .col(ColumnDef::new(MenuItems::UpdatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
            .foreign_key(ForeignKey::create().name("fk_menu_items_tenant_menu")
                .from(MenuItems::Table, MenuItems::TenantId).from_col(MenuItems::MenuId)
                .to(Menus::Table, Menus::TenantId).to_col(Menus::Id).on_update(ForeignKeyAction::Cascade).on_delete(ForeignKeyAction::Cascade)).to_owned()).await?;
        manager.create_index(Index::create().name("uq_menu_items_tenant_menu_id").table(MenuItems::Table)
            .col(MenuItems::TenantId).col(MenuItems::MenuId).col(MenuItems::Id).unique().to_owned()).await?;
        manager.create_index(Index::create().name("idx_menu_items_menu_parent_position").table(MenuItems::Table)
            .col(MenuItems::MenuId).col(MenuItems::ParentItemId).col(MenuItems::Position).to_owned()).await?;

        manager.create_table(Table::create().table(MenuItemTranslations::Table)
            .col(ColumnDef::new(MenuItemTranslations::Id).uuid().not_null().primary_key())
            .col(ColumnDef::new(MenuItemTranslations::TenantId).uuid().not_null())
            .col(ColumnDef::new(MenuItemTranslations::MenuId).uuid().not_null())
            .col(ColumnDef::new(MenuItemTranslations::MenuItemId).uuid().not_null())
            .col(ColumnDef::new(MenuItemTranslations::Locale).string_len(32).not_null())
            .col(ColumnDef::new(MenuItemTranslations::Title).string_len(255).not_null())
            .foreign_key(ForeignKey::create().name("fk_menu_item_translations_tenant_item")
                .from(MenuItemTranslations::Table, MenuItemTranslations::TenantId).from_col(MenuItemTranslations::MenuId).from_col(MenuItemTranslations::MenuItemId)
                .to(MenuItems::Table, MenuItems::TenantId).to_col(MenuItems::MenuId).to_col(MenuItems::Id)
                .on_update(ForeignKeyAction::Cascade).on_delete(ForeignKeyAction::Cascade)).to_owned()).await?;
        manager.create_index(Index::create().name("uq_menu_item_translations_tenant_menu_item_locale").table(MenuItemTranslations::Table)
            .col(MenuItemTranslations::TenantId).col(MenuItemTranslations::MenuId).col(MenuItemTranslations::MenuItemId).col(MenuItemTranslations::Locale).unique().to_owned()).await?;

        manager.create_table(Table::create().table(MenuBindings::Table)
            .col(ColumnDef::new(MenuBindings::Id).uuid().not_null().primary_key())
            .col(ColumnDef::new(MenuBindings::TenantId).uuid().not_null())
            .col(ColumnDef::new(MenuBindings::ChannelId).uuid().not_null())
            .col(ColumnDef::new(MenuBindings::Location).string_len(32).not_null())
            .col(ColumnDef::new(MenuBindings::MenuId).uuid().not_null())
            .col(ColumnDef::new(MenuBindings::CreatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
            .col(ColumnDef::new(MenuBindings::UpdatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
            .foreign_key(ForeignKey::create().name("fk_menu_bindings_tenant_menu")
                .from(MenuBindings::Table, MenuBindings::TenantId).from_col(MenuBindings::MenuId)
                .to(Menus::Table, Menus::TenantId).to_col(Menus::Id).on_update(ForeignKeyAction::Cascade).on_delete(ForeignKeyAction::Cascade))
            .foreign_key(ForeignKey::create().name("fk_menu_bindings_channel").from(MenuBindings::Table, MenuBindings::ChannelId)
                .to(Channels::Table, Channels::Id).on_update(ForeignKeyAction::Cascade).on_delete(ForeignKeyAction::Cascade)).to_owned()).await?;
        manager.create_index(Index::create().name("uq_menu_bindings_tenant_channel_location").table(MenuBindings::Table)
            .col(MenuBindings::TenantId).col(MenuBindings::ChannelId).col(MenuBindings::Location).unique().to_owned()).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [MenuBindings::Table, MenuItemTranslations::Table, MenuItems::Table, MenuTranslations::Table, Menus::Table] {
            if manager.has_table(table.to_string()).await? { manager.drop_table(Table::drop().table(table).to_owned()).await?; }
        }
        Ok(())
    }
}

#[derive(DeriveIden)] enum Menus { Table, Id, TenantId, Location, CreatedAt, UpdatedAt }
#[derive(DeriveIden)] enum MenuTranslations { Table, Id, TenantId, MenuId, Locale, Name }
#[derive(DeriveIden)] enum MenuItems { Table, Id, MenuId, TenantId, ParentItemId, PageId, Position, Url, Icon, CreatedAt, UpdatedAt }
#[derive(DeriveIden)] enum MenuItemTranslations { Table, Id, TenantId, MenuId, MenuItemId, Locale, Title }
#[derive(DeriveIden)] enum MenuBindings { Table, Id, TenantId, ChannelId, Location, MenuId, CreatedAt, UpdatedAt }
#[derive(DeriveIden)] enum Channels { Table, Id }
