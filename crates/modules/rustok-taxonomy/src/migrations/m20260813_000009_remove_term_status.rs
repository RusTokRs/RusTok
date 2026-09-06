use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE taxonomy_translation_changes \
                 SET lifecycle = 'active' \
                 WHERE lifecycle = 'archived'",
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(TaxonomyTerms::Table)
                    .drop_column(TaxonomyTerms::Status)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // The removed soft lifecycle is intentionally not reconstructed.
        // A rollback restores the storage shape with all surviving terms active.
        manager
            .alter_table(
                Table::alter()
                    .table(TaxonomyTerms::Table)
                    .add_column(
                        ColumnDef::new(TaxonomyTerms::Status)
                            .string_len(32)
                            .not_null()
                            .default("active"),
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum TaxonomyTerms {
    #[iden = "taxonomy_terms"]
    Table,
    Status,
}
