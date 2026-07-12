use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_index_content_unique")
                    .table(IndexContent::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_index_content_unique")
                    .table(IndexContent::Table)
                    .col(IndexContent::TenantId)
                    .col(IndexContent::NodeId)
                    .col(IndexContent::Locale)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_index_content_unique")
                    .table(IndexContent::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_index_content_unique")
                    .table(IndexContent::Table)
                    .col(IndexContent::NodeId)
                    .col(IndexContent::Locale)
                    .unique()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum IndexContent {
    Table,
    TenantId,
    NodeId,
    Locale,
}
