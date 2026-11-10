use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

/// Adds the reaper lease column to `page_publish_jobs`.
///
/// A claimed job (`state = 'executing'`) holds its lease until `lease_until`; the
/// sweep reaper requeues jobs whose lease expired (crashed worker) instead of
/// leaving them stranded in `executing` forever. Partial unique index
/// `uq_page_publish_jobs_active` is untouched: the reaper resolves supersession
/// before returning a job to `scheduled`.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PagePublishJobs::Table)
                    .add_column(
                        ColumnDef::new(PagePublishJobs::LeaseUntil)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // The reaper only ever scans executing jobs, and their count is bounded
        // by in-flight claims. Same partial-index syntax on Postgres and SQLite.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_page_publish_jobs_lease \
                 ON page_publish_jobs (lease_until) WHERE state = 'executing'",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_page_publish_jobs_lease")
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(PagePublishJobs::Table)
                    .drop_column(PagePublishJobs::LeaseUntil)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PagePublishJobs {
    Table,
    LeaseUntil,
}
