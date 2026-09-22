use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PageRouteAliases::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PageRouteAliases::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PageRouteAliases::TenantId).uuid().not_null())
                    .col(ColumnDef::new(PageRouteAliases::PageId).uuid().not_null())
                    .col(
                        ColumnDef::new(PageRouteAliases::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRouteAliases::Slug)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRouteAliases::Disposition)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PageRouteAliases::TargetPageId).uuid())
                    .col(ColumnDef::new(PageRouteAliases::TargetLocale).string_len(32))
                    .col(
                        ColumnDef::new(PageRouteAliases::Reason)
                            .string_len(500)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRouteAliases::CreatedAt)
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
                    .name("idx_page_route_aliases_claim")
                    .table(PageRouteAliases::Table)
                    .col(PageRouteAliases::TenantId)
                    .col(PageRouteAliases::Locale)
                    .col(PageRouteAliases::Slug)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_page_route_aliases_target")
                    .table(PageRouteAliases::Table)
                    .col(PageRouteAliases::TenantId)
                    .col(PageRouteAliases::TargetPageId)
                    .col(PageRouteAliases::TargetLocale)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only by design: dropping immutable public route history can make
        // old URLs claimable by another page and silently break redirect safety.
        Ok(())
    }
}

#[derive(DeriveIden)]
enum PageRouteAliases {
    Table,
    Id,
    TenantId,
    PageId,
    Locale,
    Slug,
    Disposition,
    TargetPageId,
    TargetLocale,
    Reason,
    CreatedAt,
}
