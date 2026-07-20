use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AlloyScriptRevisions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AlloyScriptRevisions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AlloyScriptRevisions::ScriptId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyScriptRevisions::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyScriptRevisions::Revision)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(AlloyScriptRevisions::ParentRevision).integer())
                    .col(
                        ColumnDef::new(AlloyScriptRevisions::SourceDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyScriptRevisions::Workspace)
                            .json_binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(AlloyScriptRevisions::AuthorId).string_len(255))
                    .col(
                        ColumnDef::new(AlloyScriptRevisions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("uidx_alloy_script_revisions_script_revision")
                            .col(AlloyScriptRevisions::ScriptId)
                            .col(AlloyScriptRevisions::Revision),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_alloy_script_revisions_tenant_script_revision")
                    .table(AlloyScriptRevisions::Table)
                    .col(AlloyScriptRevisions::TenantId)
                    .col(AlloyScriptRevisions::ScriptId)
                    .col(AlloyScriptRevisions::Revision)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AlloyScriptRevisions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AlloyScriptRevisions {
    Table,
    Id,
    ScriptId,
    TenantId,
    Revision,
    ParentRevision,
    SourceDigest,
    Workspace,
    AuthorId,
    CreatedAt,
}
