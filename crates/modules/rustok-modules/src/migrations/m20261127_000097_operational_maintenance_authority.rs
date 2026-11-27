//! Durable serving-maintenance authority, independent of upgrade operations.
use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        db.execute_unprepared(
            "CREATE TABLE module_operational_maintenance_state (
                id TEXT PRIMARY KEY CHECK (id = 'current'),
                revision BIGINT NOT NULL CHECK (revision > 0),
                scopes TEXT NOT NULL
             );
             INSERT INTO module_operational_maintenance_state (id, revision, scopes)
               VALUES ('current', 1, '[]');
             CREATE TABLE module_operational_maintenance_history (
                revision BIGINT PRIMARY KEY CHECK (revision > 1),
                scopes TEXT NOT NULL,
                command_context TEXT NOT NULL,
                reason TEXT NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
             )"
        ).await?;
        match backend {
            SupportedBackend::Postgres => {
                db.execute_unprepared(
                    "CREATE FUNCTION module_guard_operational_maintenance() RETURNS TRIGGER LANGUAGE plpgsql AS $$
                     BEGIN
                       IF TG_OP <> 'UPDATE' THEN
                         RAISE EXCEPTION 'Maintenance authority cannot be removed' USING ERRCODE = '23514';
                       END IF;
                       IF NEW.id IS DISTINCT FROM OLD.id OR
                          (NEW.scopes IS DISTINCT FROM OLD.scopes AND
                           (OLD.revision = 9223372036854775807 OR NEW.revision - OLD.revision <> 1)) OR
                          (NEW.scopes IS NOT DISTINCT FROM OLD.scopes AND NEW.revision <> OLD.revision) THEN
                         RAISE EXCEPTION 'Invalid maintenance authority transition' USING ERRCODE = '23514';
                       END IF;
                       RETURN NEW;
                     END $$;
                     CREATE TRIGGER module_guard_operational_maintenance_row
                       BEFORE UPDATE OR DELETE ON module_operational_maintenance_state
                       FOR EACH ROW EXECUTE FUNCTION module_guard_operational_maintenance();
                     CREATE TRIGGER module_guard_operational_maintenance_truncate
                       BEFORE TRUNCATE ON module_operational_maintenance_state
                       FOR EACH STATEMENT EXECUTE FUNCTION module_guard_operational_maintenance()"
                ).await?;
            }
            SupportedBackend::Sqlite => {
                db.execute_unprepared(
                    "CREATE TRIGGER module_guard_operational_maintenance_update
                       BEFORE UPDATE ON module_operational_maintenance_state
                       WHEN NEW.id IS NOT OLD.id OR
                         (NEW.scopes IS NOT OLD.scopes AND
                          (OLD.revision = 9223372036854775807 OR NEW.revision - OLD.revision <> 1)) OR
                         (NEW.scopes IS OLD.scopes AND NEW.revision <> OLD.revision)
                       BEGIN SELECT RAISE(ABORT, 'Invalid maintenance authority transition'); END;
                     CREATE TRIGGER module_guard_operational_maintenance_delete
                       BEFORE DELETE ON module_operational_maintenance_state
                       BEGIN SELECT RAISE(ABORT, 'Maintenance authority cannot be removed'); END;
                     CREATE TRIGGER module_guard_operational_maintenance_insert
                       BEFORE INSERT ON module_operational_maintenance_state
                       WHEN EXISTS (SELECT 1 FROM module_operational_maintenance_state)
                       BEGIN SELECT RAISE(ABORT, 'Maintenance authority already exists'); END"
                ).await?;
            }
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("operational maintenance authority cannot be downgraded safely".into()))
    }
}
