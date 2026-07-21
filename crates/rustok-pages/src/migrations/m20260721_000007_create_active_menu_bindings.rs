use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MenuBindings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MenuBindings::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MenuBindings::TenantId).uuid().not_null())
                    .col(ColumnDef::new(MenuBindings::ChannelId).uuid().not_null())
                    .col(
                        ColumnDef::new(MenuBindings::Location)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MenuBindings::MenuId).uuid().not_null())
                    .col(
                        ColumnDef::new(MenuBindings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MenuBindings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_menu_bindings_menu")
                            .from(MenuBindings::Table, MenuBindings::MenuId)
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
                    .name("uq_menu_bindings_tenant_channel_location")
                    .table(MenuBindings::Table)
                    .col(MenuBindings::TenantId)
                    .col(MenuBindings::ChannelId)
                    .col(MenuBindings::Location)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_menu_bindings_tenant_menu")
                    .table(MenuBindings::Table)
                    .col(MenuBindings::TenantId)
                    .col(MenuBindings::MenuId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MenuBindings::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum MenuBindings {
    Table,
    Id,
    TenantId,
    ChannelId,
    Location,
    MenuId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Menus {
    Table,
    Id,
}
