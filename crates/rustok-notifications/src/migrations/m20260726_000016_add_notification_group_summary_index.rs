use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => manager
                .get_connection()
                .execute_unprepared(UP)
                .await
                .map(|_| ()),
            backend => Err(DbErr::Custom(format!(
                "notification group-summary index does not support database backend {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_notifications_group_summary;")
            .await
            .map(|_| ())
    }
}

const UP: &str = r#"
CREATE INDEX IF NOT EXISTS idx_notifications_group_summary
    ON notifications (tenant_id, recipient_id, created_at DESC, id DESC, group_key)
    WHERE group_key IS NOT NULL AND state <> 'archived';
"#;
