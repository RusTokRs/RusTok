use sea_orm::sea_query::{ColumnDef, Index, Table};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(InstallHttpJobs::Table)
                    .add_column(
                        ColumnDef::new(InstallHttpJobs::IdempotencyKey)
                            .string_len(191)
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(InstallHttpJobs::RequestHash)
                            .string_len(64)
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uidx_install_http_jobs_idempotency_key")
                    .table(InstallHttpJobs::Table)
                    .col(InstallHttpJobs::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("uidx_install_http_jobs_idempotency_key")
                    .table(InstallHttpJobs::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(InstallHttpJobs::Table)
                    .drop_column(InstallHttpJobs::RequestHash)
                    .drop_column(InstallHttpJobs::IdempotencyKey)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum InstallHttpJobs {
    Table,
    IdempotencyKey,
    RequestHash,
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn adds_idempotency_columns_and_unique_index() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");

        db.execute_unprepared(
            r#"
            CREATE TABLE install_http_jobs (
                id TEXT PRIMARY KEY NOT NULL,
                status TEXT NOT NULL,
                submitted_at TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NULL,
                session_id TEXT NULL,
                tenant_id TEXT NULL,
                output TEXT NULL,
                error_message TEXT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .await
        .expect("base install_http_jobs table");

        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("idempotency migration should apply");

        let indexes = db
            .query_all_raw(
                sea_orm::Statement::from_string(
                    db.get_database_backend(),
                    "PRAGMA index_list('install_http_jobs')".to_string(),
                ),
            )
            .await
            .expect("index list should load");

        assert!(indexes.iter().any(|row| {
            row.try_get::<String>("", "name")
                .map(|name| name == "uidx_install_http_jobs_idempotency_key")
                .unwrap_or(false)
        }));
    }
}
