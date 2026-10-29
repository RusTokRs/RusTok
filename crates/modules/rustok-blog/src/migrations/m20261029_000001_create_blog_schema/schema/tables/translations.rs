use super::super::*;

pub(super) async fn create(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    // 2. Create blog_post_translations table
    manager
        .create_table(
            Table::create()
                .table(BlogPostTranslations::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(BlogPostTranslations::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(BlogPostTranslations::PostId).uuid().not_null())
                .col(
                    ColumnDef::new(BlogPostTranslations::Locale)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(BlogPostTranslations::Title)
                        .string_len(255)
                        .not_null(),
                )
                .col(ColumnDef::new(BlogPostTranslations::Excerpt).text())
                .col(ColumnDef::new(BlogPostTranslations::SeoTitle).string_len(255))
                .col(ColumnDef::new(BlogPostTranslations::SeoDescription).string_len(500))
                .col(ColumnDef::new(BlogPostTranslations::Body).text().not_null())
                .col(
                    ColumnDef::new(BlogPostTranslations::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(BlogPostTranslations::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_blog_post_translations_post")
                        .from(BlogPostTranslations::Table, BlogPostTranslations::PostId)
                        .to(BlogPosts::Table, BlogPosts::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                        .on_update(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name("idx_blog_post_translations_unique")
                .table(BlogPostTranslations::Table)
                .col(BlogPostTranslations::PostId)
                .col(BlogPostTranslations::Locale)
                .unique()
                .if_not_exists()
                .to_owned(),
        )
        .await?;
    Ok(())
}
