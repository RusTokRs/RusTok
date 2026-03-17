use super::shared::*;
use sea_orm_migration::prelude::*;



#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Collections::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Collections::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Collections::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(Collections::Type)
                            .string_len(32)
                            .not_null()
                            .default("manual"),
                    )
                    .col(ColumnDef::new(Collections::Conditions).json_binary())
                    .col(
                        ColumnDef::new(Collections::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Collections::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Collections::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Collections::DeletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Collections::Table, Collections::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CollectionTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CollectionTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CollectionTranslations::CollectionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CollectionTranslations::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CollectionTranslations::Title)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CollectionTranslations::Handle)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(CollectionTranslations::Description).text())
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                CollectionTranslations::Table,
                                CollectionTranslations::CollectionId,
                            )
                            .to(Collections::Table, Collections::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CollectionProducts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CollectionProducts::CollectionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CollectionProducts::ProductId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CollectionProducts::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CollectionProducts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(CollectionProducts::CollectionId)
                            .col(CollectionProducts::ProductId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CollectionProducts::Table, CollectionProducts::CollectionId)
                            .to(Collections::Table, Collections::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CollectionProducts::Table, CollectionProducts::ProductId)
                            .to(Products::Table, Products::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_collections_tenant")
                    .table(Collections::Table)
                    .col(Collections::TenantId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_collection_trans_unique")
                    .table(CollectionTranslations::Table)
                    .col(CollectionTranslations::CollectionId)
                    .col(CollectionTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_collection_trans_handle")
                    .table(CollectionTranslations::Table)
                    .col(CollectionTranslations::Locale)
                    .col(CollectionTranslations::Handle)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CollectionProducts::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(CollectionTranslations::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Collections::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Collections {
    Table,
    Id,
    TenantId,
    Type,
    Conditions,
    Metadata,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}

#[derive(Iden)]
enum CollectionTranslations {
    Table,
    Id,
    CollectionId,
    Locale,
    Title,
    Handle,
    Description,
}

#[derive(Iden)]
enum CollectionProducts {
    Table,
    CollectionId,
    ProductId,
    Position,
    CreatedAt,
}

#[derive(Iden)]
enum Products {
    Table,
    Id,
}
