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
                    .table(Media::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Media::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Media::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Media::UploadedBy).uuid())
                    .col(ColumnDef::new(Media::Filename).string_len(255).not_null())
                    .col(
                        ColumnDef::new(Media::OriginalName)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Media::MimeType).string_len(100).not_null())
                    .col(ColumnDef::new(Media::Size).big_integer().not_null())
                    .col(
                        ColumnDef::new(Media::StoragePath)
                            .string_len(500)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Media::StorageDriver)
                            .string_len(32)
                            .not_null()
                            .default("local"),
                    )
                    .col(ColumnDef::new(Media::Width).integer())
                    .col(ColumnDef::new(Media::Height).integer())
                    .col(
                        ColumnDef::new(Media::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Media::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Media::Table, Media::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Media::Table, Media::UploadedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MediaTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MediaTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MediaTranslations::MediaId).uuid().not_null())
                    .col(
                        ColumnDef::new(MediaTranslations::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MediaTranslations::Title).string_len(255))
                    .col(ColumnDef::new(MediaTranslations::AltText).string_len(255))
                    .col(ColumnDef::new(MediaTranslations::Caption).text())
                    .foreign_key(
                        ForeignKey::create()
                            .from(MediaTranslations::Table, MediaTranslations::MediaId)
                            .to(Media::Table, Media::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_media_tenant")
                    .table(Media::Table)
                    .col(Media::TenantId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_media_trans_unique")
                    .table(MediaTranslations::Table)
                    .col(MediaTranslations::MediaId)
                    .col(MediaTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MediaTranslations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Media::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
pub enum Media {
    Table,
    Id,
    TenantId,
    UploadedBy,
    Filename,
    OriginalName,
    MimeType,
    Size,
    StoragePath,
    StorageDriver,
    Width,
    Height,
    Metadata,
    CreatedAt,
}

#[derive(Iden)]
enum MediaTranslations {
    Table,
    Id,
    MediaId,
    Locale,
    Title,
    AltText,
    Caption,
}
