use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProductTags::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ProductTags::ProductId).uuid().not_null())
                    .col(ColumnDef::new(ProductTags::TermId).uuid().not_null())
                    .col(ColumnDef::new(ProductTags::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(ProductTags::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_product_tags")
                            .col(ProductTags::ProductId)
                            .col(ProductTags::TermId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_tags_product")
                            .from(ProductTags::Table, ProductTags::ProductId)
                            .to(Products::Table, Products::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_tags_term")
                            .from(ProductTags::Table, ProductTags::TermId)
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
                    .name("idx_product_tags_tenant_term")
                    .table(ProductTags::Table)
                    .col(ProductTags::TenantId)
                    .col(ProductTags::TermId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProductTags::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ProductTags {
    Table,
    ProductId,
    TermId,
    TenantId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Products {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum TaxonomyTerms {
    Table,
    Id,
}
