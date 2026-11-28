//! Reject late waits on the global artifact security receipt identity.
use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
CREATE FUNCTION module_artifact_security_receipt_key_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE target_key uuid;
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key THEN
            RAISE EXCEPTION 'Artifact security receipt key is immutable' USING ERRCODE = '23514';
        END IF;
    END IF;
    IF TG_OP = 'DELETE' THEN target_key := OLD.idempotency_key;
    ELSE target_key := NEW.idempotency_key; END IF;
    -- BEFORE INSERT runs before unique-index admission. UPDATE/DELETE may
    -- already own the tuple, so none of these paths may wait on the key.
    IF NOT pg_try_advisory_xact_lock(hashtextextended(
        'rustok:module-artifact-security-receipt:' || target_key::text, 0)) THEN
        RAISE EXCEPTION 'Artifact security receipt key busy' USING ERRCODE = '55P03';
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER module_artifact_security_receipt_key_fence
BEFORE INSERT OR UPDATE OR DELETE ON module_artifact_security_operations
FOR EACH ROW EXECUTE FUNCTION module_artifact_security_receipt_key_guard();
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if SupportedBackend::new(manager.get_database_backend())? == SupportedBackend::Postgres {
            manager.get_connection().execute_unprepared(POSTGRES).await?;
        }
        // SQLite serializes writers; there is no independent unique-key wait.
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if SupportedBackend::new(manager.get_database_backend())? == SupportedBackend::Postgres {
            return Err(DbErr::Migration(
                "artifact security receipt key fence cannot be downgraded safely".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DatabaseConnection, DatabaseTransaction, DbBackend, TransactionTrait};
    use std::time::Duration;
    use uuid::Uuid;

    async fn tx(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = crate::backend::begin_fenced_transaction(db).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}"))
            .await.unwrap();
        tx
    }

    fn insert(key: Uuid) -> String {
        format!("INSERT INTO module_artifact_security_operations
                 (idempotency_key, operation_kind, request_digest, principal_id, trace_id, correlation_id)
                 VALUES ('{key}', 'quarantine', 'sha256:{}', '{}', 'trace', '{}')
                 ON CONFLICT (idempotency_key) DO NOTHING",
                 "a".repeat(64), Uuid::new_v4(), Uuid::new_v4())
    }

    async fn reject_busy(db: &DatabaseConnection, schema: &str, sql: &str) {
        let contender = tx(db, schema).await;
        let error = tokio::time::timeout(
            Duration::from_secs(3), contender.execute_unprepared(sql),
        ).await.expect("late receipt key must not wait").unwrap_err();
        assert!(error.to_string().contains("Artifact security receipt key busy"), "{error}");
        contender.rollback().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_security_receipt_key_contention_is_nonblocking_and_transaction_bound() {
        let db = Database::connect(std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").unwrap())
            .await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let schema = format!("security_receipt_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = tx(&db, &schema).await;
        super::super::m20260722_000036_artifact_security_state::Migration
            .up(&SchemaManager::new(&setup)).await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.commit().await.unwrap();

        let key = Uuid::new_v4();
        let other = Uuid::new_v4();
        let owner = tx(&db, &schema).await;
        owner.execute_unprepared(&insert(key)).await.unwrap();
        reject_busy(&db, &schema, &insert(key)).await;
        let unrelated = tx(&db, &schema).await;
        unrelated.execute_unprepared(&insert(other)).await.unwrap();
        unrelated.commit().await.unwrap();
        owner.rollback().await.unwrap();

        let retry = tx(&db, &schema).await;
        assert_eq!(retry.execute_unprepared(&insert(key)).await.unwrap().rows_affected(), 1);
        retry.execute_unprepared(&format!(
            "UPDATE module_artifact_security_operations SET receipt_json = '{{}}',
             completed_at = CURRENT_TIMESTAMP WHERE idempotency_key = '{key}'"
        )).await.unwrap();
        retry.commit().await.unwrap();

        let replay = tx(&db, &schema).await;
        assert_eq!(replay.execute_unprepared(&insert(key)).await.unwrap().rows_affected(), 0);
        replay.rollback().await.unwrap();

        let editor = tx(&db, &schema).await;
        editor.execute_unprepared(&format!(
            "UPDATE module_artifact_security_operations SET trace_id = 'maintenance'
             WHERE idempotency_key = '{key}'"
        )).await.unwrap();
        reject_busy(&db, &schema, &insert(key)).await;
        editor.rollback().await.unwrap();

        let invalid = tx(&db, &schema).await;
        let error = invalid.execute_unprepared(&format!(
            "UPDATE module_artifact_security_operations SET idempotency_key = '{}'
             WHERE idempotency_key = '{key}'", Uuid::new_v4()
        )).await.unwrap_err();
        assert!(error.to_string().contains("Artifact security receipt key is immutable"), "{error}");
        invalid.rollback().await.unwrap();

        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
