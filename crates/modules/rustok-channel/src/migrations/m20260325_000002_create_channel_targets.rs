use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ChannelTargets::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ChannelTargets::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ChannelTargets::ChannelId).uuid().not_null())
                    .col(
                        ColumnDef::new(ChannelTargets::TargetType)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ChannelTargets::Value)
                            .string_len(500)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ChannelTargets::IsPrimary)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(ChannelTargets::Settings)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(ChannelTargets::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ChannelTargets::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_channel_targets_channel_id")
                            .from(ChannelTargets::Table, ChannelTargets::ChannelId)
                            .to(Channels::Table, Channels::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_channel_targets_channel")
                    .table(ChannelTargets::Table)
                    .col(ChannelTargets::ChannelId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ChannelTargets::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Channels {
    Table,
    Id,
}

#[derive(Iden)]
enum ChannelTargets {
    Table,
    Id,
    ChannelId,
    TargetType,
    Value,
    IsPrimary,
    Settings,
    CreatedAt,
    UpdatedAt,
}
