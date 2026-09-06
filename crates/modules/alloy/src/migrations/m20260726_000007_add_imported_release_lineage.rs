use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_column("scripts", "parent_release_slug").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Scripts::Table)
                        .add_column(ColumnDef::new(Scripts::ParentReleaseSlug).string_len(128))
                        .to_owned(),
                )
                .await?;
        }
        if !manager
            .has_column("scripts", "parent_release_version")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Scripts::Table)
                        .add_column(ColumnDef::new(Scripts::ParentReleaseVersion).string_len(64))
                        .to_owned(),
                )
                .await?;
        }
        if !manager
            .has_column("scripts", "parent_release_digest")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Scripts::Table)
                        .add_column(ColumnDef::new(Scripts::ParentReleaseDigest).string_len(71))
                        .to_owned(),
                )
                .await?;
        }
        if !manager
            .has_column("alloy_script_revisions", "parent_release_slug")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(AlloyScriptRevisions::Table)
                        .add_column(
                            ColumnDef::new(AlloyScriptRevisions::ParentReleaseSlug).string_len(128),
                        )
                        .to_owned(),
                )
                .await?;
        }
        if !manager
            .has_column("alloy_script_revisions", "parent_release_version")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(AlloyScriptRevisions::Table)
                        .add_column(
                            ColumnDef::new(AlloyScriptRevisions::ParentReleaseVersion)
                                .string_len(64),
                        )
                        .to_owned(),
                )
                .await?;
        }
        if !manager
            .has_column("alloy_script_revisions", "parent_release_digest")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(AlloyScriptRevisions::Table)
                        .add_column(
                            ColumnDef::new(AlloyScriptRevisions::ParentReleaseDigest)
                                .string_len(71),
                        )
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (table_name, table, columns) in [
            (
                "alloy_script_revisions",
                Tables::AlloyScriptRevisions,
                [
                    (
                        "parent_release_digest",
                        Columns::RevisionParentReleaseDigest,
                    ),
                    (
                        "parent_release_version",
                        Columns::RevisionParentReleaseVersion,
                    ),
                    ("parent_release_slug", Columns::RevisionParentReleaseSlug),
                ],
            ),
            (
                "scripts",
                Tables::Scripts,
                [
                    ("parent_release_digest", Columns::ScriptParentReleaseDigest),
                    (
                        "parent_release_version",
                        Columns::ScriptParentReleaseVersion,
                    ),
                    ("parent_release_slug", Columns::ScriptParentReleaseSlug),
                ],
            ),
        ] {
            for (column_name, column) in columns {
                if manager.has_column(table_name, column_name).await? {
                    manager
                        .alter_table(Table::alter().table(table).drop_column(column).to_owned())
                        .await?;
                }
            }
        }
        Ok(())
    }
}

#[derive(DeriveIden, Clone, Copy)]
enum Scripts {
    Table,
    ParentReleaseSlug,
    ParentReleaseVersion,
    ParentReleaseDigest,
}

#[derive(DeriveIden, Clone, Copy)]
enum AlloyScriptRevisions {
    Table,
    ParentReleaseSlug,
    ParentReleaseVersion,
    ParentReleaseDigest,
}

#[derive(Iden, Clone, Copy)]
enum Tables {
    Scripts,
    AlloyScriptRevisions,
}

#[derive(Iden, Clone, Copy)]
enum Columns {
    #[iden = "parent_release_slug"]
    ScriptParentReleaseSlug,
    #[iden = "parent_release_version"]
    ScriptParentReleaseVersion,
    #[iden = "parent_release_digest"]
    ScriptParentReleaseDigest,
    #[iden = "parent_release_slug"]
    RevisionParentReleaseSlug,
    #[iden = "parent_release_version"]
    RevisionParentReleaseVersion,
    #[iden = "parent_release_digest"]
    RevisionParentReleaseDigest,
}
