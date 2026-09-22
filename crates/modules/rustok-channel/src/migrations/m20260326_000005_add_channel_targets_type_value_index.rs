use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx_channel_targets_type_value")
                    .table(ChannelTargets::Table)
                    .col(ChannelTargets::TargetType)
                    .col(ChannelTargets::Value)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_channel_targets_type_value")
                    .table(ChannelTargets::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum ChannelTargets {
    Table,
    TargetType,
    Value,
}
