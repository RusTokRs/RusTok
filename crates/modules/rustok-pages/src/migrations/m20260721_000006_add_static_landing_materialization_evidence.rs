use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PageStaticLandingArtifacts::Table)
                    .add_column(
                        ColumnDef::new(PageStaticLandingArtifacts::MaterializationHash)
                            .string_len(128)
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(PageStaticLandingArtifacts::Table)
                    .add_column(
                        ColumnDef::new(PageStaticLandingArtifacts::MaterializationIdentity)
                            .json_binary()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(PageStaticLandingArtifacts::Table)
                    .add_column(
                        ColumnDef::new(PageStaticLandingArtifacts::RuntimeSnapshots)
                            .json_binary()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_page_static_landing_artifacts_build")
                    .table(PageStaticLandingArtifacts::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_page_static_landing_artifacts_build")
                    .table(PageStaticLandingArtifacts::Table)
                    .col(PageStaticLandingArtifacts::TenantId)
                    .col(PageStaticLandingArtifacts::PageId)
                    .col(PageStaticLandingArtifacts::Locale)
                    .col(PageStaticLandingArtifacts::BuildHash)
                    .col(PageStaticLandingArtifacts::MaterializationHash)
                    .unique()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only by design. Once multiple runtime materializations share one Fly build hash,
        // restoring the old four-column unique key could discard valid immutable artifacts.
        Ok(())
    }
}

#[derive(DeriveIden)]
enum PageStaticLandingArtifacts {
    Table,
    TenantId,
    PageId,
    Locale,
    BuildHash,
    MaterializationHash,
    MaterializationIdentity,
    RuntimeSnapshots,
}
