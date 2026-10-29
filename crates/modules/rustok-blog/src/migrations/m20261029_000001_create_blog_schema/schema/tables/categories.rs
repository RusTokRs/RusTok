use super::super::*;
use sea_orm_migration::prelude::*;

pub(super) async fn create(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
// 4. Create blog_categories table
        manager
            .create_table(
                Table::create()
                    .table(BlogCategories::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlogCategories::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(BlogCategories::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(BlogCategories::Settings)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(BlogCategories::Revision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(BlogCategories::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(BlogCategories::UpdatedAt)
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
                    .name("uq_blog_categories_tenant_id")
                    .table(BlogCategories::Table)
                    .col(BlogCategories::TenantId)
                    .col(BlogCategories::Id)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
    Ok(())
}
