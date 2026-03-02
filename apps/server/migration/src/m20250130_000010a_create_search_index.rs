use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SearchIndex::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(SearchIndex::Id).uuid().not_null())
                    .col(ColumnDef::new(SearchIndex::TenantId).uuid().not_null())
                    .col(ColumnDef::new(SearchIndex::Locale).string().not_null())
                    .col(ColumnDef::new(SearchIndex::DocType).string().not_null())
                    .col(ColumnDef::new(SearchIndex::Title).string().not_null())
                    .col(ColumnDef::new(SearchIndex::Content).text())
                    .col(ColumnDef::new(SearchIndex::Payload).json_binary().not_null())
                    .col(
                        ColumnDef::new(SearchIndex::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(SearchIndex::Id)
                            .col(SearchIndex::Locale),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SearchIndex::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum SearchIndex {
    Table,
    Id,
    TenantId,
    Locale,
    DocType,
    Title,
    Content,
    Payload,
    UpdatedAt,
}
