//! Migration to create the content_revisions table.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ContentRevisions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ContentRevisions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::ContentId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::ContentType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::Locale)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::RevisionNumber)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::ParentRevisionId)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::Event)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::Content)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::UserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::Source)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::Summary)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::IpAddress)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::UserAgent)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::CustomMetadata)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentRevisions::VersionName)
                            .string()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_content_revisions_tenant_content_locale")
                    .table(ContentRevisions::Table)
                    .col(ContentRevisions::TenantId)
                    .col(ContentRevisions::ContentId)
                    .col(ContentRevisions::Locale)
                    .col(ContentRevisions::RevisionNumber)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_content_revisions_content_type")
                    .table(ContentRevisions::Table)
                    .col(ContentRevisions::ContentType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_content_revisions_created_at")
                    .table(ContentRevisions::Table)
                    .col(ContentRevisions::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_content_revisions_version_name")
                    .table(ContentRevisions::Table)
                    .col(ContentRevisions::VersionName)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ContentRevisions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ContentRevisions {
    Table,
    Id,
    TenantId,
    ContentId,
    ContentType,
    Locale,
    RevisionNumber,
    ParentRevisionId,
    Event,
    Content,
    UserId,
    Source,
    Summary,
    IpAddress,
    UserAgent,
    CustomMetadata,
    CreatedAt,
    VersionName,
}
