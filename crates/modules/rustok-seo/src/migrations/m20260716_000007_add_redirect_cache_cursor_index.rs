use sea_orm_migration::prelude::*;

const INDEX_NAME: &str = "idx_seo_event_deliveries_redirect_cursor";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name(INDEX_NAME)
                    .table(SeoEventDeliveries::Table)
                    .col(SeoEventDeliveries::SourceKind)
                    .col(SeoEventDeliveries::CreatedAt)
                    .col(SeoEventDeliveries::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(INDEX_NAME)
                    .table(SeoEventDeliveries::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum SeoEventDeliveries {
    Table,
    Id,
    SourceKind,
    CreatedAt,
}
