use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SearchQueryClicks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SearchQueryClicks::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SearchQueryClicks::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SearchQueryClicks::QueryLogId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SearchQueryClicks::DocumentId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SearchQueryClicks::Position).integer())
                    .col(ColumnDef::new(SearchQueryClicks::Href).text())
                    .col(
                        ColumnDef::new(SearchQueryClicks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_search_query_clicks_query_log")
                            .from(SearchQueryClicks::Table, SearchQueryClicks::QueryLogId)
                            .to(SearchQueryLogs::Table, SearchQueryLogs::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_search_query_clicks_tenant_created")
                    .table(SearchQueryClicks::Table)
                    .col(SearchQueryClicks::TenantId)
                    .col(SearchQueryClicks::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_search_query_clicks_query_log")
                    .table(SearchQueryClicks::Table)
                    .col(SearchQueryClicks::QueryLogId)
                    .col(SearchQueryClicks::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SearchQueryClicks::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum SearchQueryClicks {
    Table,
    Id,
    TenantId,
    QueryLogId,
    DocumentId,
    Position,
    Href,
    CreatedAt,
}

#[derive(Iden)]
enum SearchQueryLogs {
    Table,
    Id,
}
