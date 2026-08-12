use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_query::Expr;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("forum_category_covers"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("category_id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("tenant_id"))
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("media_id"))
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_forum_category_covers_category")
                            .from(
                                Alias::new("forum_category_covers"),
                                Alias::new("category_id"),
                            )
                            .to(Alias::new("forum_categories"), Alias::new("id"))
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_forum_category_covers_tenant_media")
                    .table(Alias::new("forum_category_covers"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("media_id"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("forum_category_covers"))
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}
