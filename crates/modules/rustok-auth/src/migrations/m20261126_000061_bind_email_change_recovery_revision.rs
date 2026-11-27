use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let statement = match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                "ALTER TABLE auth_email_change_requests ADD COLUMN recovery_revision BIGINT CHECK (recovery_revision >= 0)"
            }
            DatabaseBackend::Sqlite => {
                "ALTER TABLE auth_email_change_requests ADD COLUMN recovery_revision BIGINT CHECK (recovery_revision IS NULL OR (typeof(recovery_revision) = 'integer' AND recovery_revision >= 0))"
            }
            _ => {
                return Err(DbErr::Custom(
                    "unsupported email-change recovery revision backend".into(),
                ));
            }
        };
        db.execute_unprepared(statement).await?;
        // An outstanding legacy challenge cannot prove its issuing revision.
        // Do not backfill it from the current user or leave its mailbox reserved.
        db.execute_unprepared(
            "DELETE FROM auth_email_change_requests WHERE recovery_revision IS NULL",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        // Do not turn revision-bound challenges into legacy credentials.
        db.execute_unprepared("DELETE FROM auth_email_change_requests")
            .await?;
        db.execute_unprepared(
            "ALTER TABLE auth_email_change_requests DROP COLUMN recovery_revision",
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::Database;

    #[tokio::test]
    async fn migration_invalidates_legacy_and_down_invalidates_bound_challenges() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE auth_email_change_requests (id TEXT PRIMARY KEY)")
            .await
            .unwrap();
        db.execute_unprepared("INSERT INTO auth_email_change_requests (id) VALUES ('old')")
            .await
            .unwrap();

        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();
        // Reusing the primary key proves the legacy reservation was removed.
        db.execute_unprepared(
            "INSERT INTO auth_email_change_requests (id, recovery_revision) VALUES ('old', 0)",
        )
        .await
        .unwrap();
        assert!(db.execute_unprepared("INSERT INTO auth_email_change_requests (id, recovery_revision) VALUES ('invalid', -1)")
            .await.is_err());

        Migration.down(&manager).await.unwrap();
        db.execute_unprepared("INSERT INTO auth_email_change_requests (id) VALUES ('old')")
            .await
            .unwrap();
    }
}
