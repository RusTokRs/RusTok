use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                    .col(ColumnDef::new(BlogCategories::ParentId).uuid())
                    .col(
                        ColumnDef::new(BlogCategories::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(BlogCategories::Depth)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(BlogCategories::PostCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(BlogCategories::Settings)
                            .json_binary()
                            .not_null()
                            .default("{}"),
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
            .create_table(
                Table::create()
                    .table(BlogCategoryTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlogCategoryTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BlogCategoryTranslations::CategoryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCategoryTranslations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCategoryTranslations::Locale)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCategoryTranslations::Name)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCategoryTranslations::Slug)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(BlogCategoryTranslations::Description).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blog_category_translations_category")
                            .from(
                                BlogCategoryTranslations::Table,
                                BlogCategoryTranslations::CategoryId,
                            )
                            .to(BlogCategories::Table, BlogCategories::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_category_translations_category_locale")
                    .table(BlogCategoryTranslations::Table)
                    .col(BlogCategoryTranslations::CategoryId)
                    .col(BlogCategoryTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_category_translations_tenant_locale_slug")
                    .table(BlogCategoryTranslations::Table)
                    .col(BlogCategoryTranslations::TenantId)
                    .col(BlogCategoryTranslations::Locale)
                    .col(BlogCategoryTranslations::Slug)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(BlogPostTags::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(BlogPostTags::PostId).uuid().not_null())
                    .col(ColumnDef::new(BlogPostTags::TagId).uuid().not_null())
                    .col(
                        ColumnDef::new(BlogPostTags::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(BlogPostTags::PostId)
                            .col(BlogPostTags::TagId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blog_post_tags_post")
                            .from(BlogPostTags::Table, BlogPostTags::PostId)
                            .to(BlogPosts::Table, BlogPosts::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blog_post_tags_tag")
                            .from(BlogPostTags::Table, BlogPostTags::TagId)
                            .to(TaxonomyTerms::Table, TaxonomyTerms::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_post_tags_tag_id")
                    .table(BlogPostTags::Table)
                    .col(BlogPostTags::TagId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BlogPostTags::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(BlogCategoryTranslations::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(BlogCategories::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum BlogPosts {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum BlogCategories {
    Table,
    Id,
    TenantId,
    ParentId,
    Position,
    Depth,
    PostCount,
    Settings,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum BlogCategoryTranslations {
    Table,
    Id,
    CategoryId,
    TenantId,
    Locale,
    Name,
    Slug,
    Description,
}

#[derive(DeriveIden)]
enum BlogPostTags {
    Table,
    PostId,
    TagId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum TaxonomyTerms {
    Table,
    Id,
}
