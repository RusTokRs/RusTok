use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Products::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Products::Revision)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;

        // The positive-revision invariant is a PostgreSQL constraint because the portable
        // SQLite test schema cannot express ALTER TABLE CHECK constraints. Every portable
        // write path additionally refuses to persist a non-positive revision.
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    "ALTER TABLE products DROP CONSTRAINT IF EXISTS chk_products_revision_positive",
                )
                .await?;
            manager
                .get_connection()
                .execute_unprepared(
                    "ALTER TABLE products ADD CONSTRAINT chk_products_revision_positive \
                     CHECK (revision > 0)",
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    "ALTER TABLE products DROP CONSTRAINT IF EXISTS chk_products_revision_positive",
                )
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(Products::Table)
                    .drop_column(Products::Revision)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Products {
    Table,
    Revision,
}
