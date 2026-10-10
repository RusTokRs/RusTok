use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m0001_create_content_revisions::Migration)]
    }
}

mod m0001_create_content_revisions {
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
                            ColumnDef::new(ContentRevisions::ContentType)
                                .string_len(100)
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ContentRevisions::ContentId)
                                .uuid()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ContentRevisions::Locale)
                                .string_len(10)
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ContentRevisions::RevisionNumber)
                                .integer()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ContentRevisions::ParentRevisionId)
                                .uuid()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(ContentRevisions::Delta)
                                .json_binary()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ContentRevisions::CreatedBy)
                                .uuid()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ContentRevisions::CreatedAt)
                                .timestamp_with_time_zone()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ContentRevisions::ChangeSource)
                                .string_len(50)
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ContentRevisions::ChangeSummary)
                                .text()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(ContentRevisions::VersionName)
                                .string_len(100)
                                .null(),
                        )
                        .to_owned(),
                )
                .await?;

            // Create composite unique index
            manager
                .create_index(
                    Index::create()
                        .name("idx_content_revisions_unique")
                        .table(ContentRevisions::Table)
                        .col(ContentRevisions::TenantId)
                        .col(ContentRevisions::ContentType)
                        .col(ContentRevisions::ContentId)
                        .col(ContentRevisions::Locale)
                        .col(ContentRevisions::RevisionNumber)
                        .unique()
                        .to_owned(),
                )
                .await?;

            // Create index for listing revisions
            manager
                .create_index(
                    Index::create()
                        .name("idx_content_revisions_lookup")
                        .table(ContentRevisions::Table)
                        .col(ContentRevisions::TenantId)
                        .col(ContentRevisions::ContentType)
                        .col(ContentRevisions::ContentId)
                        .col(ContentRevisions::Locale)
                        .to_owned(),
                )
                .await?;

            // Create index for created_at (for retention policies)
            manager
                .create_index(
                    Index::create()
                        .name("idx_content_revisions_created_at")
                        .table(ContentRevisions::Table)
                        .col(ContentRevisions::CreatedAt)
                        .to_owned(),
                )
                .await?;

            // Create index for named versions
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
        ContentType,
        ContentId,
        Locale,
        RevisionNumber,
        ParentRevisionId,
        Delta,
        CreatedBy,
        CreatedAt,
        ChangeSource,
        ChangeSummary,
        VersionName,
    }
}
