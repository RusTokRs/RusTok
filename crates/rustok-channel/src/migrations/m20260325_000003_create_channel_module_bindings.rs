use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ChannelModuleBindings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ChannelModuleBindings::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ChannelModuleBindings::ChannelId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ChannelModuleBindings::ModuleSlug)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ChannelModuleBindings::IsEnabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(ChannelModuleBindings::Settings)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(ChannelModuleBindings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ChannelModuleBindings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_channel_module_bindings_channel_id")
                            .from(
                                ChannelModuleBindings::Table,
                                ChannelModuleBindings::ChannelId,
                            )
                            .to(Channels::Table, Channels::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_channel_module_bindings_unique")
                    .table(ChannelModuleBindings::Table)
                    .col(ChannelModuleBindings::ChannelId)
                    .col(ChannelModuleBindings::ModuleSlug)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ChannelModuleBindings::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Channels {
    Table,
    Id,
}

#[derive(Iden)]
enum ChannelModuleBindings {
    Table,
    Id,
    ChannelId,
    ModuleSlug,
    IsEnabled,
    Settings,
    CreatedAt,
    UpdatedAt,
}
