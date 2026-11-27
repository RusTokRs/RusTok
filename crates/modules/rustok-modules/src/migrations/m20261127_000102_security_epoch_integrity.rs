//! Keep the durable security epoch permanent across ordinary SQL mutations.
use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
CREATE FUNCTION module_security_guard_epoch() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'Security epoch singleton cannot be inserted or removed' USING ERRCODE = '23514';
    END IF;
    IF NEW.singleton_id IS DISTINCT FROM OLD.singleton_id OR NEW.singleton_id <> 1
       OR OLD.epoch < 1 THEN
        RAISE EXCEPTION 'Security epoch identity is immutable' USING ERRCODE = '23514';
    END IF;
    IF NEW.epoch IS NOT DISTINCT FROM OLD.epoch
       AND NEW.last_reason IS NOT DISTINCT FROM OLD.last_reason
       AND NEW.updated_at IS NOT DISTINCT FROM OLD.updated_at THEN
        RETURN NEW;
    END IF;
    IF OLD.epoch = 9223372036854775807 THEN
        RAISE EXCEPTION 'Security epoch exhausted' USING ERRCODE = '23514';
    END IF;
    IF NEW.epoch IS DISTINCT FROM OLD.epoch + 1 THEN
        RAISE EXCEPTION 'Security epoch must advance by one' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER module_security_epoch_guard
BEFORE INSERT OR UPDATE OR DELETE ON module_security_epoch
FOR EACH ROW EXECUTE FUNCTION module_security_guard_epoch();
CREATE TRIGGER module_security_epoch_no_truncate
BEFORE TRUNCATE ON module_security_epoch
FOR EACH STATEMENT EXECUTE FUNCTION module_security_guard_epoch();
"#;

const SQLITE: &str = r#"
CREATE TRIGGER module_security_epoch_no_insert
BEFORE INSERT ON module_security_epoch
BEGIN SELECT RAISE(ABORT, 'Security epoch singleton cannot be inserted'); END;
CREATE TRIGGER module_security_epoch_no_delete
BEFORE DELETE ON module_security_epoch
BEGIN SELECT RAISE(ABORT, 'Security epoch singleton cannot be removed'); END;
CREATE TRIGGER module_security_epoch_guard
BEFORE UPDATE ON module_security_epoch
WHEN NEW.singleton_id IS NOT OLD.singleton_id OR NEW.singleton_id <> 1
 OR typeof(OLD.epoch) <> 'integer' OR typeof(NEW.epoch) <> 'integer'
 OR OLD.epoch < 1
 OR NOT (
    (NEW.epoch = OLD.epoch AND NEW.last_reason IS OLD.last_reason AND NEW.updated_at IS OLD.updated_at)
    OR (OLD.epoch < 9223372036854775807 AND NEW.epoch = OLD.epoch + 1)
 )
BEGIN SELECT RAISE(ABORT, 'Security epoch must advance by one without changing identity'); END;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let (integer_guard, guards) = match backend {
            SupportedBackend::Postgres => {
                db.execute_unprepared("LOCK TABLE module_security_epoch IN SHARE ROW EXCLUSIVE MODE")
                    .await?;
                ("", POSTGRES)
            }
            SupportedBackend::Sqlite => {
                // Reserve the writer before validating the preexisting singleton.
                db.execute_unprepared("UPDATE module_security_epoch SET epoch = epoch WHERE singleton_id = 1")
                    .await?;
                (" AND typeof(epoch) = 'integer'", SQLITE)
            }
        };
        let row = db
            .query_one_raw(Statement::from_string(
                backend.db_backend(),
                format!("SELECT COUNT(*) AS total, COUNT(CASE WHEN singleton_id = 1 AND epoch >= 1{integer_guard} THEN 1 END) AS valid FROM module_security_epoch"),
            ))
            .await?
            .ok_or_else(|| DbErr::Migration("security epoch validation unavailable".into()))?;
        if row.try_get::<i64>("", "total")? != 1 || row.try_get::<i64>("", "valid")? != 1 {
            return Err(DbErr::Migration(
                "invalid security epoch singleton; offline recovery required".into(),
            ));
        }
        db.execute_unprepared(guards).await?;
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "security epoch integrity cannot be downgraded safely".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security_epoch::{GlobalSecurityEpoch, SeaOrmSecurityEpochStore};
    use chrono::Utc;
    use sea_orm::{Database, DatabaseConnection, DatabaseTransaction, DbBackend, TransactionTrait};

    async fn seed(manager: &SchemaManager<'_>) {
        super::super::m20261021_000055_module_security_epoch::Migration
            .up(manager)
            .await
            .unwrap();
        manager.get_connection().execute_unprepared(
            "UPDATE module_security_epoch SET epoch = 7 WHERE singleton_id = 1"
        ).await.unwrap();
    }

    const INVALID_WRITES: &[&str] = &[
        "DELETE FROM module_security_epoch",
        "UPDATE module_security_epoch SET epoch = 1",
        "UPDATE module_security_epoch SET epoch = 9",
        "UPDATE module_security_epoch SET singleton_id = 2",
        "UPDATE module_security_epoch SET last_reason = 'forged'",
        "INSERT INTO module_security_epoch SELECT * FROM module_security_epoch",
    ];

    #[tokio::test]
    async fn sqlite_epoch_integrity_preserves_reservation_rollback_and_restart_authority() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let manager = SchemaManager::new(&db);
        seed(&manager).await;
        Migration.up(&manager).await.unwrap();
        db.execute_unprepared("PRAGMA recursive_triggers = OFF").await.unwrap();
        for sql in INVALID_WRITES.iter().copied().chain([
            "UPDATE module_security_epoch SET epoch = 7.5",
            "INSERT OR REPLACE INTO module_security_epoch VALUES (1, 1, 'reset', '1970-01-01T00:00:00+00:00')",
        ]) {
            assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
            assert_eq!(SeaOrmSecurityEpochStore::current(&db).await.unwrap(), GlobalSecurityEpoch(7));
        }

        let tx = db.begin().await.unwrap();
        let snapshot = SeaOrmSecurityEpochStore::load_registry_for_commit(&tx).await.unwrap();
        assert_eq!(snapshot.current_epoch(), GlobalSecurityEpoch(7));
        assert_eq!(SeaOrmSecurityEpochStore::advance(&tx, "rollback", Utc::now()).await.unwrap(), GlobalSecurityEpoch(8));
        tx.rollback().await.unwrap();
        assert_eq!(SeaOrmSecurityEpochStore::current(&db).await.unwrap(), GlobalSecurityEpoch(7));

        let tx = db.begin().await.unwrap();
        SeaOrmSecurityEpochStore::lock_for_advance(&tx).await.unwrap();
        assert_eq!(SeaOrmSecurityEpochStore::advance(&tx, "revoked", Utc::now()).await.unwrap(), GlobalSecurityEpoch(8));
        tx.commit().await.unwrap();
        let current = SeaOrmSecurityEpochStore::load_registry(&db).await.unwrap();
        assert!(current.validate_epoch(snapshot.current_epoch()).is_err());
        assert_eq!(current.latest_reason(), "revoked");
    }

    #[tokio::test]
    async fn sqlite_epoch_integrity_rejects_corrupt_or_missing_cutover_baseline() {
        for sql in [
            "DELETE FROM module_security_epoch",
            "UPDATE module_security_epoch SET epoch = 7.5",
        ] {
            let db = Database::connect("sqlite::memory:").await.unwrap();
            let manager = SchemaManager::new(&db);
            seed(&manager).await;
            db.execute_unprepared(sql).await.unwrap();
            let tx = db.begin().await.unwrap();
            assert!(Migration.up(&SchemaManager::new(&tx)).await.is_err(), "{sql}");
            tx.rollback().await.unwrap();
        }
    }

    async fn transaction(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = crate::backend::begin_fenced_transaction(db).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_epoch_integrity_rejects_resets_and_preserves_commit_fence() {
        use std::time::Duration;
        let db = Database::connect(std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").unwrap())
            .await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let schema = format!("security_epoch_integrity_{}", uuid::Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = transaction(&db, &schema).await;
        seed(&SchemaManager::new(&setup)).await;
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.commit().await.unwrap();

        for sql in INVALID_WRITES.iter().copied().chain(["TRUNCATE module_security_epoch"]) {
            let tx = transaction(&db, &schema).await;
            assert!(tx.execute_unprepared(sql).await.is_err(), "{sql}");
            tx.rollback().await.unwrap();
        }

        let reader = transaction(&db, &schema).await;
        let snapshot = SeaOrmSecurityEpochStore::load_registry_for_commit(&reader).await.unwrap();
        assert_eq!(snapshot.current_epoch(), GlobalSecurityEpoch(7));
        let writer_db = db.clone();
        let writer_schema = schema.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let mut writer = tokio::spawn(async move {
            let tx = transaction(&writer_db, &writer_schema).await;
            ready_tx.send(()).unwrap();
            SeaOrmSecurityEpochStore::lock_for_advance(&tx).await.unwrap();
            let next = SeaOrmSecurityEpochStore::advance(&tx, "revoked", Utc::now()).await.unwrap();
            tx.commit().await.unwrap();
            next
        });
        ready_rx.await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(150), &mut writer).await.is_err());
        reader.commit().await.unwrap();
        assert_eq!(tokio::time::timeout(Duration::from_secs(5), writer).await.unwrap().unwrap(), GlobalSecurityEpoch(8));
        let check = transaction(&db, &schema).await;
        let current = SeaOrmSecurityEpochStore::load_registry_for_commit(&check).await.unwrap();
        assert!(current.validate_epoch(snapshot.current_epoch()).is_err());
        check.rollback().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
