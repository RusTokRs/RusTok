use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TaxonomyCategoryHierarchy::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TaxonomyCategoryHierarchy::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryHierarchy::TermId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TaxonomyCategoryHierarchy::ParentTermId).uuid())
                    .col(
                        ColumnDef::new(TaxonomyCategoryHierarchy::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_taxonomy_category_hierarchy")
                            .col(TaxonomyCategoryHierarchy::TenantId)
                            .col(TaxonomyCategoryHierarchy::TermId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_taxonomy_category_hierarchy_term")
                            .from_tbl(TaxonomyCategoryHierarchy::Table)
                            .from_col(TaxonomyCategoryHierarchy::TenantId)
                            .from_col(TaxonomyCategoryHierarchy::TermId)
                            .to_tbl(TaxonomyTerms::Table)
                            .to_col(TaxonomyTerms::TenantId)
                            .to_col(TaxonomyTerms::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_taxonomy_category_hierarchy_parent")
                            .from_tbl(TaxonomyCategoryHierarchy::Table)
                            .from_col(TaxonomyCategoryHierarchy::TenantId)
                            .from_col(TaxonomyCategoryHierarchy::ParentTermId)
                            .to_tbl(TaxonomyTerms::Table)
                            .to_col(TaxonomyTerms::TenantId)
                            .to_col(TaxonomyTerms::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_taxonomy_category_hierarchy_parent_position")
                    .table(TaxonomyCategoryHierarchy::Table)
                    .col(TaxonomyCategoryHierarchy::TenantId)
                    .col(TaxonomyCategoryHierarchy::ParentTermId)
                    .col(TaxonomyCategoryHierarchy::Position)
                    .col(TaxonomyCategoryHierarchy::TermId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(TaxonomyCategoryHierarchy::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum TaxonomyCategoryHierarchy {
    Table,
    TenantId,
    TermId,
    ParentTermId,
    Position,
}

#[derive(DeriveIden)]
enum TaxonomyTerms {
    Table,
    TenantId,
    Id,
}
