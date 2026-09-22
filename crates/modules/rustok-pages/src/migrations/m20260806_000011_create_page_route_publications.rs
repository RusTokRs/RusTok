use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PageRoutePublications::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PageRoutePublications::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PageRoutePublications::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRoutePublications::PageId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRoutePublications::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRoutePublications::Slug)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PageRoutePublications::RecordedAt)
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
                    .name("idx_page_route_publications_claim")
                    .table(PageRoutePublications::Table)
                    .col(PageRoutePublications::TenantId)
                    .col(PageRoutePublications::Locale)
                    .col(PageRoutePublications::Slug)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_page_route_publications_page")
                    .table(PageRoutePublications::Table)
                    .col(PageRoutePublications::TenantId)
                    .col(PageRoutePublications::PageId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only by design: these snapshots retain which localized routes
        // were actually public before a later lifecycle transition and delete.
        Ok(())
    }
}

#[derive(DeriveIden)]
enum PageRoutePublications {
    Table,
    Id,
    TenantId,
    PageId,
    Locale,
    Slug,
    RecordedAt,
}
