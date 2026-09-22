use super::shared::*;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Categories::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Categories::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Categories::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Categories::ParentId).uuid())
                    .col(
                        ColumnDef::new(Categories::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Categories::Depth)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Categories::NodeCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Categories::Settings)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Categories::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Categories::Table, Categories::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Categories::Table, Categories::ParentId)
                            .to(Categories::Table, Categories::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CategoryTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CategoryTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CategoryTranslations::CategoryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CategoryTranslations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CategoryTranslations::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CategoryTranslations::Name)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CategoryTranslations::Slug)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(CategoryTranslations::Description).text())
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                CategoryTranslations::Table,
                                CategoryTranslations::CategoryId,
                            )
                            .to(Categories::Table, Categories::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CategoryTranslations::Table, CategoryTranslations::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_nodes_category")
                        .from(Nodes::Table, Nodes::CategoryId)
                        .to(Categories::Table, Categories::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_index(
                Index::create()
                    .name("idx_categories_tenant")
                    .table(Categories::Table)
                    .col(Categories::TenantId)
                    .col(Categories::Position)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_categories_parent")
                    .table(Categories::Table)
                    .col(Categories::ParentId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_cat_trans_unique")
                    .table(CategoryTranslations::Table)
                    .col(CategoryTranslations::CategoryId)
                    .col(CategoryTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_cat_trans_slug")
                    .table(CategoryTranslations::Table)
                    .col(CategoryTranslations::TenantId)
                    .col(CategoryTranslations::Locale)
                    .col(CategoryTranslations::Slug)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .table(Nodes::Table)
                        .name("fk_nodes_category")
                        .to_owned(),
                )
                .await?;
        }
        manager
            .drop_table(Table::drop().table(CategoryTranslations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Categories::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Categories {
    Table,
    Id,
    TenantId,
    ParentId,
    Position,
    Depth,
    NodeCount,
    Settings,
    CreatedAt,
}

#[derive(Iden)]
enum CategoryTranslations {
    Table,
    Id,
    CategoryId,
    TenantId,
    Locale,
    Name,
    Slug,
    Description,
}

#[derive(Iden)]
enum Nodes {
    Table,
    CategoryId,
}
