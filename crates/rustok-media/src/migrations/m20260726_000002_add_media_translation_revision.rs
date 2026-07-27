use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MediaTranslations::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(MediaTranslations::Revision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: locale revisions are concurrency evidence and must not
        // be silently discarded after revision-aware writes begin.
        Ok(())
    }
}

#[derive(DeriveIden)]
enum MediaTranslations {
    Table,
    Revision,
}
