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
                    .table(Meta::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Meta::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Meta::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Meta::TargetType).string_len(32).not_null())
                    .col(ColumnDef::new(Meta::TargetId).uuid().not_null())
                    .col(
                        ColumnDef::new(Meta::NoIndex)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Meta::NoFollow)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(Meta::CanonicalUrl).string_len(500))
                    .col(ColumnDef::new(Meta::StructuredData).json_binary())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Meta::Table, Meta::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MetaTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MetaTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MetaTranslations::MetaId).uuid().not_null())
                    .col(
                        ColumnDef::new(MetaTranslations::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MetaTranslations::Title).string_len(255))
                    .col(ColumnDef::new(MetaTranslations::Description).string_len(500))
                    .col(ColumnDef::new(MetaTranslations::Keywords).string_len(255))
                    .col(ColumnDef::new(MetaTranslations::OgTitle).string_len(255))
                    .col(ColumnDef::new(MetaTranslations::OgDescription).string_len(500))
                    .col(ColumnDef::new(MetaTranslations::OgImage).string_len(500))
                    .foreign_key(
                        ForeignKey::create()
                            .from(MetaTranslations::Table, MetaTranslations::MetaId)
                            .to(Meta::Table, Meta::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_meta_target")
                    .table(Meta::Table)
                    .col(Meta::TargetType)
                    .col(Meta::TargetId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_meta_trans_unique")
                    .table(MetaTranslations::Table)
                    .col(MetaTranslations::MetaId)
                    .col(MetaTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MetaTranslations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Meta::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Meta {
    Table,
    Id,
    TenantId,
    TargetType,
    TargetId,
    NoIndex,
    NoFollow,
    CanonicalUrl,
    StructuredData,
}

#[derive(Iden)]
enum MetaTranslations {
    Table,
    Id,
    MetaId,
    Locale,
    Title,
    Description,
    Keywords,
    OgTitle,
    OgDescription,
    OgImage,
}
