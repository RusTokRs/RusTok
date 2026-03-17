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
                    .table(Tags::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Tags::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Tags::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(Tags::UseCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Tags::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Tags::Table, Tags::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TagTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TagTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TagTranslations::TagId).uuid().not_null())
                    .col(ColumnDef::new(TagTranslations::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(TagTranslations::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TagTranslations::Name)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TagTranslations::Slug)
                            .string_len(100)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagTranslations::Table, TagTranslations::TagId)
                            .to(Tags::Table, Tags::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagTranslations::Table, TagTranslations::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Taggables::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Taggables::TagId).uuid().not_null())
                    .col(
                        ColumnDef::new(Taggables::TargetType)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Taggables::TargetId).uuid().not_null())
                    .col(
                        ColumnDef::new(Taggables::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(Taggables::TagId)
                            .col(Taggables::TargetType)
                            .col(Taggables::TargetId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Taggables::Table, Taggables::TagId)
                            .to(Tags::Table, Tags::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tags_tenant")
                    .table(Tags::Table)
                    .col(Tags::TenantId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_tags_popular")
                    .table(Tags::Table)
                    .col(Tags::TenantId)
                    .col(Tags::UseCount)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_tag_trans_unique")
                    .table(TagTranslations::Table)
                    .col(TagTranslations::TagId)
                    .col(TagTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_tag_trans_slug")
                    .table(TagTranslations::Table)
                    .col(TagTranslations::TenantId)
                    .col(TagTranslations::Locale)
                    .col(TagTranslations::Slug)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_taggables_target")
                    .table(Taggables::Table)
                    .col(Taggables::TargetType)
                    .col(Taggables::TargetId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Taggables::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TagTranslations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Tags::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Tags {
    Table,
    Id,
    TenantId,
    UseCount,
    CreatedAt,
}

#[derive(Iden)]
enum TagTranslations {
    Table,
    Id,
    TagId,
    TenantId,
    Locale,
    Name,
    Slug,
}

#[derive(Iden)]
enum Taggables {
    Table,
    TagId,
    TargetType,
    TargetId,
    CreatedAt,
}
