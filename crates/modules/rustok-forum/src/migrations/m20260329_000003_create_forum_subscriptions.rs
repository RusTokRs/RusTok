use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ForumCategorySubscriptions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ForumCategorySubscriptions::CategoryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumCategorySubscriptions::UserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumCategorySubscriptions::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumCategorySubscriptions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(ForumCategorySubscriptions::CategoryId)
                            .col(ForumCategorySubscriptions::UserId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_forum_category_subscriptions_category")
                            .from(
                                ForumCategorySubscriptions::Table,
                                ForumCategorySubscriptions::CategoryId,
                            )
                            .to(ForumCategories::Table, ForumCategories::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_forum_category_subscriptions_tenant_category")
                    .table(ForumCategorySubscriptions::Table)
                    .col(ForumCategorySubscriptions::TenantId)
                    .col(ForumCategorySubscriptions::CategoryId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ForumTopicSubscriptions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ForumTopicSubscriptions::TopicId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumTopicSubscriptions::UserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumTopicSubscriptions::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumTopicSubscriptions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(ForumTopicSubscriptions::TopicId)
                            .col(ForumTopicSubscriptions::UserId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_forum_topic_subscriptions_topic")
                            .from(
                                ForumTopicSubscriptions::Table,
                                ForumTopicSubscriptions::TopicId,
                            )
                            .to(ForumTopics::Table, ForumTopics::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_forum_topic_subscriptions_tenant_topic")
                    .table(ForumTopicSubscriptions::Table)
                    .col(ForumTopicSubscriptions::TenantId)
                    .col(ForumTopicSubscriptions::TopicId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ForumTopicSubscriptions::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(ForumCategorySubscriptions::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ForumCategorySubscriptions {
    Table,
    CategoryId,
    UserId,
    TenantId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ForumTopicSubscriptions {
    Table,
    TopicId,
    UserId,
    TenantId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ForumCategories {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ForumTopics {
    Table,
    Id,
}
