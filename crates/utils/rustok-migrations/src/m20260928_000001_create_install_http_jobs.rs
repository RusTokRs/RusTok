use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(InstallHttpJobs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InstallHttpJobs::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(InstallHttpJobs::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InstallHttpJobs::SubmittedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InstallHttpJobs::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InstallHttpJobs::FinishedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(InstallHttpJobs::SessionId).uuid().null())
                    .col(ColumnDef::new(InstallHttpJobs::TenantId).uuid().null())
                    .col(ColumnDef::new(InstallHttpJobs::Output).json_binary().null())
                    .col(ColumnDef::new(InstallHttpJobs::ErrorMessage).text().null())
                    .col(
                        ColumnDef::new(InstallHttpJobs::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_install_http_jobs_status_updated")
                    .table(InstallHttpJobs::Table)
                    .col(InstallHttpJobs::Status)
                    .col(InstallHttpJobs::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_install_http_jobs_submitted")
                    .table(InstallHttpJobs::Table)
                    .col(InstallHttpJobs::SubmittedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(InstallHttpJobs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum InstallHttpJobs {
    Table,
    Id,
    Status,
    SubmittedAt,
    StartedAt,
    FinishedAt,
    SessionId,
    TenantId,
    Output,
    ErrorMessage,
    UpdatedAt,
}

#[cfg(test)]
mod tests {
    use sea_orm::Database;
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn creates_durable_http_job_table() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("install_http_jobs migration should apply");

        assert!(
            SchemaManager::new(&db)
                .has_table("install_http_jobs")
                .await
                .expect("table lookup should succeed")
        );
    }
}
