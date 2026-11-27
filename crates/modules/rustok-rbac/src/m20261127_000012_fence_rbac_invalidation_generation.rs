//! Preserve the singleton cache recovery watermark across ordinary SQL writes.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES_GUARDS: &str = r#"
CREATE FUNCTION rbac_guard_invalidation_generation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'RBAC invalidation watermark cannot be inserted or removed' USING ERRCODE = '23514';
    END IF;
    IF NEW.scope IS DISTINCT FROM OLD.scope OR NEW.scope <> 'permissions'
       OR OLD.generation < 0 OR NEW.generation <= OLD.generation THEN
        RAISE EXCEPTION 'RBAC invalidation watermark must advance monotonically' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER rbac_invalidation_generation_guard
BEFORE INSERT OR UPDATE OR DELETE ON rbac_invalidation_state
FOR EACH ROW EXECUTE FUNCTION rbac_guard_invalidation_generation();
CREATE TRIGGER rbac_invalidation_generation_no_truncate
BEFORE TRUNCATE ON rbac_invalidation_state
FOR EACH STATEMENT EXECUTE FUNCTION rbac_guard_invalidation_generation();
"#;

const SQLITE_GUARDS: &str = r#"
CREATE TRIGGER rbac_invalidation_generation_no_insert
BEFORE INSERT ON rbac_invalidation_state
BEGIN SELECT RAISE(ABORT, 'RBAC invalidation watermark cannot be inserted'); END;
CREATE TRIGGER rbac_invalidation_generation_no_delete
BEFORE DELETE ON rbac_invalidation_state
BEGIN SELECT RAISE(ABORT, 'RBAC invalidation watermark cannot be removed'); END;
CREATE TRIGGER rbac_invalidation_generation_guard
BEFORE UPDATE ON rbac_invalidation_state
WHEN NEW.scope IS NOT OLD.scope OR NEW.scope <> 'permissions'
 OR typeof(OLD.generation) <> 'integer' OR typeof(NEW.generation) <> 'integer'
 OR OLD.generation < 0 OR NEW.generation <= OLD.generation
BEGIN SELECT RAISE(ABORT, 'RBAC invalidation watermark must advance monotonically'); END;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();
        let (integer_guard, guards) = match backend {
            DatabaseBackend::Postgres => {
                db.execute_unprepared("LOCK TABLE rbac_invalidation_state IN SHARE ROW EXCLUSIVE MODE").await?;
                ("", POSTGRES_GUARDS)
            }
            DatabaseBackend::Sqlite => {
                // Reserve the writer before validating the existing singleton.
                db.execute_unprepared("UPDATE rbac_invalidation_state SET generation = generation WHERE scope = 'permissions'").await?;
                (" AND typeof(generation) = 'integer'", SQLITE_GUARDS)
            }
            _ => return Err(DbErr::Migration("unsupported RBAC generation backend".into())),
        };
        let row = db.query_one_raw(Statement::from_string(
            backend,
            format!("SELECT COUNT(*) AS total, COUNT(CASE WHEN scope = 'permissions' AND generation >= 0{integer_guard} THEN 1 END) AS valid FROM rbac_invalidation_state"),
        )).await?.ok_or_else(|| DbErr::Migration("RBAC invalidation watermark validation unavailable".into()))?;
        if row.try_get::<i64>("", "total")? != 1 || row.try_get::<i64>("", "valid")? != 1 {
            return Err(DbErr::Migration("invalid RBAC invalidation watermark; offline recovery required".into()));
        }
        db.execute_unprepared(guards).await?;
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("RBAC invalidation watermark integrity cannot be downgraded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection, DatabaseTransaction, TransactionTrait, IsolationLevel};
    use crate::{read_permission_invalidation_generation, reserve_permission_invalidation_generation};

    async fn setup(manager: &SchemaManager<'_>) {
        crate::m20260714_900002_create_rbac_invalidation_state::Migration
            .up(manager).await.unwrap();
        Migration.up(manager).await.unwrap();
    }

    const INVALID_DML: &[&str] = &[
        "DELETE FROM rbac_invalidation_state",
        "UPDATE rbac_invalidation_state SET generation = 0",
        "UPDATE rbac_invalidation_state SET generation = generation",
        "UPDATE rbac_invalidation_state SET scope = 'other', generation = generation + 1",
        "INSERT INTO rbac_invalidation_state (scope, generation) VALUES ('other', 0)",
    ];

    #[tokio::test]
    async fn sqlite_generation_is_permanent_even_with_recursive_triggers_disabled() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        setup(&SchemaManager::new(&db)).await;
        db.execute_unprepared("PRAGMA recursive_triggers = OFF").await.unwrap();
        let tx = db.begin().await.unwrap();
        assert_eq!(reserve_permission_invalidation_generation(&tx).await.unwrap(), 1);
        tx.commit().await.unwrap();
        for sql in INVALID_DML.iter().copied().chain([
            "INSERT OR REPLACE INTO rbac_invalidation_state (scope, generation) VALUES ('permissions', 0)",
            "UPDATE rbac_invalidation_state SET generation = 1.5",
        ]) {
            assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
            assert_eq!(read_permission_invalidation_generation(&db).await.unwrap(), 1);
        }
        let tx = db.begin().await.unwrap();
        assert_eq!(reserve_permission_invalidation_generation(&tx).await.unwrap(), 2);
        tx.rollback().await.unwrap();
        assert_eq!(read_permission_invalidation_generation(&db).await.unwrap(), 1);
        db.execute_unprepared("UPDATE rbac_invalidation_state SET generation = 10").await.unwrap();
        let tx = db.begin().await.unwrap();
        assert_eq!(reserve_permission_invalidation_generation(&tx).await.unwrap(), 11);
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn sqlite_migration_rejects_missing_or_corrupt_predecessor() {
        for invalid in [
            "DELETE FROM rbac_invalidation_state",
            "UPDATE rbac_invalidation_state SET generation = -1",
            "UPDATE rbac_invalidation_state SET generation = 0.5",
            "INSERT INTO rbac_invalidation_state (scope, generation) VALUES ('extra', 0)",
        ] {
            let db = Database::connect("sqlite::memory:").await.unwrap();
            crate::m20260714_900002_create_rbac_invalidation_state::Migration.up(&SchemaManager::new(&db)).await.unwrap();
            db.execute_unprepared(invalid).await.unwrap();
            let tx = db.begin().await.unwrap();
            assert!(Migration.up(&SchemaManager::new(&tx)).await.is_err(), "{invalid}");
            tx.rollback().await.unwrap();
        }
    }

    async fn transaction(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = db.begin_with_config(Some(IsolationLevel::ReadCommitted), None).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_RBAC_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_generation_dml_cannot_reset_watermark_and_reservation_rejects_contention() {
        use std::time::Duration;
        let db = Database::connect(std::env::var("RUSTOK_RBAC_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("rbac_watermark_{}", uuid::Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let initial = transaction(&db, &schema).await;
        setup(&SchemaManager::new(&initial)).await;
        assert_eq!(reserve_permission_invalidation_generation(&initial).await.unwrap(), 1);
        initial.commit().await.unwrap();

        for sql in INVALID_DML.iter().copied().chain(["TRUNCATE rbac_invalidation_state"]) {
            let tx = transaction(&db, &schema).await;
            assert!(tx.execute_unprepared(sql).await.is_err(), "{sql}");
            tx.rollback().await.unwrap();
        }
        let first = transaction(&db, &schema).await;
        assert_eq!(reserve_permission_invalidation_generation(&first).await.unwrap(), 2);
        let contender = transaction(&db, &schema).await;
        let error = tokio::time::timeout(
            Duration::from_secs(3),
            reserve_permission_invalidation_generation(&contender),
        )
        .await.expect("late generation fence must not wait").unwrap_err();
        assert!(matches!(error, crate::RbacInvalidationGenerationError::Database(_)));
        contender.rollback().await.unwrap();
        first.rollback().await.unwrap();

        let retry = transaction(&db, &schema).await;
        assert_eq!(reserve_permission_invalidation_generation(&retry).await.unwrap(), 2);
        retry.commit().await.unwrap();
        let verify = transaction(&db, &schema).await;
        assert_eq!(read_permission_invalidation_generation(&verify).await.unwrap(), 2);
        verify.rollback().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
