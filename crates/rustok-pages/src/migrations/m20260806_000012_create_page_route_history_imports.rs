use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PageRouteHistoryImports::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PageRouteHistoryImports::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PageRouteHistoryImports::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRouteHistoryImports::Source)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRouteHistoryImports::SourceRecordId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRouteHistoryImports::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRouteHistoryImports::PageId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRouteHistoryImports::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRouteHistoryImports::Slug)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRouteHistoryImports::PageWasMissing)
                            .boolean()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PageRouteHistoryImports::ImportedBy).uuid())
                    .col(
                        ColumnDef::new(PageRouteHistoryImports::ImportedAt)
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
                    .name("idx_page_route_history_imports_source")
                    .table(PageRouteHistoryImports::Table)
                    .col(PageRouteHistoryImports::TenantId)
                    .col(PageRouteHistoryImports::Source)
                    .col(PageRouteHistoryImports::SourceRecordId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_page_route_history_imports_route")
                    .table(PageRouteHistoryImports::Table)
                    .col(PageRouteHistoryImports::TenantId)
                    .col(PageRouteHistoryImports::PageId)
                    .col(PageRouteHistoryImports::Locale)
                    .col(PageRouteHistoryImports::Slug)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_page_route_history_imports_audit")
                    .table(PageRouteHistoryImports::Table)
                    .col(PageRouteHistoryImports::TenantId)
                    .col(PageRouteHistoryImports::ImportedAt)
                    .col(PageRouteHistoryImports::Id)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only by design: provenance receipts protect idempotent replay
        // and explain why an externally recovered public route remains reserved.
        Ok(())
    }
}

#[derive(DeriveIden)]
enum PageRouteHistoryImports {
    Table,
    Id,
    TenantId,
    Source,
    SourceRecordId,
    RequestHash,
    PageId,
    Locale,
    Slug,
    PageWasMissing,
    ImportedBy,
    ImportedAt,
}
