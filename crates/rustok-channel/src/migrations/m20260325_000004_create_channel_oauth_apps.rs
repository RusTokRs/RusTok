use super::shared::OAuthApps;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ChannelOauthApps::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ChannelOauthApps::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ChannelOauthApps::ChannelId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ChannelOauthApps::OauthAppId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ChannelOauthApps::Role).string_len(100))
                    .col(
                        ColumnDef::new(ChannelOauthApps::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_channel_oauth_apps_channel_id")
                            .from(ChannelOauthApps::Table, ChannelOauthApps::ChannelId)
                            .to(Channels::Table, Channels::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_channel_oauth_apps_oauth_app_id")
                            .from(ChannelOauthApps::Table, ChannelOauthApps::OauthAppId)
                            .to(OAuthApps::Table, OAuthApps::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_channel_oauth_apps_unique")
                    .table(ChannelOauthApps::Table)
                    .col(ChannelOauthApps::ChannelId)
                    .col(ChannelOauthApps::OauthAppId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ChannelOauthApps::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Channels {
    Table,
    Id,
}

#[derive(Iden)]
enum ChannelOauthApps {
    Table,
    Id,
    ChannelId,
    OauthAppId,
    Role,
    CreatedAt,
}
