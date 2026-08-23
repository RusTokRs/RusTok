use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ForumCategories::Table)
                    .add_column(ColumnDef::new(ForumCategories::TaxonomyCategoryId).uuid())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_forum_categories_tenant_taxonomy_category")
                    .table(ForumCategories::Table)
                    .col(ForumCategories::TenantId)
                    .col(ForumCategories::TaxonomyCategoryId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("uq_forum_categories_tenant_taxonomy_category")
                    .table(ForumCategories::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ForumCategories::Table)
                    .drop_column(ForumCategories::TaxonomyCategoryId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum ForumCategories {
    #[iden = "forum_categories"]
    Table,
    TenantId,
    TaxonomyCategoryId,
}
