use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SearchQueryLogs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SearchQueryLogs::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SearchQueryLogs::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(SearchQueryLogs::Surface)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(SearchQueryLogs::QueryText).text().not_null())
                    .col(
                        ColumnDef::new(SearchQueryLogs::QueryNormalized)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SearchQueryLogs::Locale).string_len(16))
                    .col(
                        ColumnDef::new(SearchQueryLogs::Engine)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SearchQueryLogs::ResultCount)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(SearchQueryLogs::TookMs)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(SearchQueryLogs::Status)
                            .string_len(32)
                            .not_null()
                            .default("success"),
                    )
                    .col(
                        ColumnDef::new(SearchQueryLogs::Filters)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(SearchQueryLogs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_search_query_logs_tenant_created")
                    .table(SearchQueryLogs::Table)
                    .col(SearchQueryLogs::TenantId)
                    .col(SearchQueryLogs::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_search_query_logs_tenant_query")
                    .table(SearchQueryLogs::Table)
                    .col(SearchQueryLogs::TenantId)
                    .col(SearchQueryLogs::QueryNormalized)
                    .col(SearchQueryLogs::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_search_query_logs_tenant_status")
                    .table(SearchQueryLogs::Table)
                    .col(SearchQueryLogs::TenantId)
                    .col(SearchQueryLogs::Status)
                    .col(SearchQueryLogs::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SearchQueryLogs::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum SearchQueryLogs {
    Table,
    Id,
    TenantId,
    Surface,
    QueryText,
    QueryNormalized,
    Locale,
    Engine,
    ResultCount,
    TookMs,
    Status,
    Filters,
    CreatedAt,
}
