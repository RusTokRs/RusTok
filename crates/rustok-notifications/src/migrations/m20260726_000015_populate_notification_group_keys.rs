use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => manager
                .get_connection()
                .execute_unprepared(POSTGRES_UP)
                .await
                .map(|_| ()),
            DatabaseBackend::Sqlite => manager
                .get_connection()
                .execute_unprepared(SQLITE_UP)
                .await
                .map(|_| ()),
            backend => Err(DbErr::Custom(format!(
                "notification group-key population does not support database backend {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => manager
                .get_connection()
                .execute_unprepared(POSTGRES_DOWN)
                .await
                .map(|_| ()),
            DatabaseBackend::Sqlite => manager
                .get_connection()
                .execute_unprepared(SQLITE_DOWN)
                .await
                .map(|_| ()),
            backend => Err(DbErr::Custom(format!(
                "notification group-key population does not support database backend {backend:?}"
            ))),
        }
    }
}

const POSTGRES_UP: &str = r#"
CREATE OR REPLACE FUNCTION rustok_notifications_assign_group_key()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.group_key IS NULL THEN
        NEW.group_key := 'g1:' || NEW.target_owner || ':' || NEW.target_id::text;
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_notifications_assign_group_key ON notifications;
CREATE TRIGGER trg_notifications_assign_group_key
BEFORE INSERT ON notifications
FOR EACH ROW
EXECUTE FUNCTION rustok_notifications_assign_group_key();

UPDATE notifications
SET group_key = 'g1:' || target_owner || ':' || target_id::text
WHERE group_key IS NULL;
"#;

const SQLITE_UP: &str = r#"
DROP TRIGGER IF EXISTS trg_notifications_assign_group_key;
CREATE TRIGGER trg_notifications_assign_group_key
AFTER INSERT ON notifications
FOR EACH ROW
WHEN NEW.group_key IS NULL
BEGIN
    UPDATE notifications
    SET group_key = 'g1:' || NEW.target_owner || ':' || NEW.target_id
    WHERE tenant_id = NEW.tenant_id
      AND id = NEW.id
      AND group_key IS NULL;
END;

UPDATE notifications
SET group_key = 'g1:' || target_owner || ':' || target_id
WHERE group_key IS NULL;
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS trg_notifications_assign_group_key ON notifications;
DROP FUNCTION IF EXISTS rustok_notifications_assign_group_key();

UPDATE notifications
SET group_key = NULL
WHERE group_key = 'g1:' || target_owner || ':' || target_id::text;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS trg_notifications_assign_group_key;

UPDATE notifications
SET group_key = NULL
WHERE group_key = 'g1:' || target_owner || ':' || target_id;
"#;
