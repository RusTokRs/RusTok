//! Permanent tenant-scoped operation-key claims for artifact grants.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
LOCK TABLE rbac_artifact_role_permission_operations IN SHARE ROW EXCLUSIVE MODE;
DO $$
BEGIN
    IF row_security_active('rbac_artifact_role_permission_operations'::regclass) THEN
        RAISE EXCEPTION 'RBAC receipt backfill requires complete visibility' USING ERRCODE = '42501';
    END IF;
END $$;
CREATE TABLE rbac_artifact_operation_witnesses (
    tenant_id UUID NOT NULL,
    idempotency_key TEXT NOT NULL,
    PRIMARY KEY (tenant_id, idempotency_key)
);
INSERT INTO rbac_artifact_operation_witnesses
SELECT tenant_id, idempotency_key FROM rbac_artifact_role_permission_operations;
CREATE FUNCTION rbac_try_artifact_operation_key(tenant UUID, operation_key TEXT)
RETURNS void LANGUAGE plpgsql AS $$
BEGIN
    IF NOT pg_try_advisory_xact_lock(hashtextextended(
        'rustok:rbac-artifact-operation:' || tenant::text || ':' || operation_key, 0)) THEN
        RAISE EXCEPTION 'RBAC artifact operation key busy' USING ERRCODE = '55P03';
    END IF;
END $$;
CREATE FUNCTION rbac_guard_artifact_operation_witness() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'INSERT' THEN
        RAISE EXCEPTION 'RBAC artifact operation witness is permanent' USING ERRCODE = '23514';
    END IF;
    PERFORM rbac_try_artifact_operation_key(NEW.tenant_id, NEW.idempotency_key);
    RETURN NEW;
END $$;
CREATE TRIGGER rbac_artifact_operation_witness_guard
BEFORE INSERT OR UPDATE OR DELETE ON rbac_artifact_operation_witnesses
FOR EACH ROW EXECUTE FUNCTION rbac_guard_artifact_operation_witness();
CREATE TRIGGER rbac_artifact_operation_witness_no_truncate
BEFORE TRUNCATE ON rbac_artifact_operation_witnesses
FOR EACH STATEMENT EXECUTE FUNCTION rbac_guard_artifact_operation_witness();
CREATE FUNCTION rbac_guard_artifact_operation_receipt() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' OR TG_OP = 'TRUNCATE' THEN
        RAISE EXCEPTION 'RBAC artifact operation receipt is immutable' USING ERRCODE = '23514';
    END IF;
    IF TG_OP = 'DELETE' THEN
        PERFORM rbac_try_artifact_operation_key(OLD.tenant_id, OLD.idempotency_key);
        RETURN OLD;
    END IF;
    LOCK TABLE rbac_artifact_operation_witnesses IN ROW EXCLUSIVE MODE NOWAIT;
    IF row_security_active('rbac_artifact_operation_witnesses'::regclass) THEN
        RAISE EXCEPTION 'RBAC artifact operation witness visibility incomplete' USING ERRCODE = '42501';
    END IF;
    PERFORM rbac_try_artifact_operation_key(NEW.tenant_id, NEW.idempotency_key);
    IF EXISTS (SELECT 1 FROM rbac_artifact_operation_witnesses
        WHERE tenant_id = NEW.tenant_id AND idempotency_key = NEW.idempotency_key) THEN
        RAISE EXCEPTION 'RBAC artifact operation key already consumed' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER rbac_artifact_operation_receipt_guard
BEFORE INSERT OR UPDATE OR DELETE ON rbac_artifact_role_permission_operations
FOR EACH ROW EXECUTE FUNCTION rbac_guard_artifact_operation_receipt();
CREATE TRIGGER rbac_artifact_operation_receipt_no_truncate
BEFORE TRUNCATE ON rbac_artifact_role_permission_operations
FOR EACH STATEMENT EXECUTE FUNCTION rbac_guard_artifact_operation_receipt();
CREATE FUNCTION rbac_claim_artifact_operation_witness() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO rbac_artifact_operation_witnesses (tenant_id, idempotency_key)
        VALUES (NEW.tenant_id, NEW.idempotency_key);
    RETURN NULL;
END $$;
CREATE TRIGGER rbac_artifact_operation_witness_claim
AFTER INSERT ON rbac_artifact_role_permission_operations
FOR EACH ROW EXECUTE FUNCTION rbac_claim_artifact_operation_witness();
"#;

// Canonicalize SQLite UUID encodings. WITHOUT ROWID prevents a hidden
// rowid conflict from replacing a permanent claim.
fn sqlite_uuid(column: &str) -> String {
    format!("CASE typeof({column}) WHEN 'blob' THEN lower(hex({column})) WHEN 'text' THEN lower(replace({column}, '-', '')) END")
}

async fn sqlite_up(db: &impl ConnectionTrait) -> Result<(), DbErr> {
    db.execute_unprepared(
        "UPDATE rbac_artifact_role_permission_operations SET id = id WHERE 0;
         CREATE TABLE rbac_artifact_operation_witnesses (
            tenant_id TEXT NOT NULL CHECK (length(tenant_id) = 32 AND tenant_id NOT GLOB '*[^0-9a-f]*'),
            idempotency_key TEXT NOT NULL CHECK (typeof(idempotency_key) = 'text'),
            PRIMARY KEY (tenant_id, idempotency_key)
         ) WITHOUT ROWID;",
    ).await?;
    db.execute_unprepared(&format!(
        "INSERT INTO rbac_artifact_operation_witnesses SELECT {}, idempotency_key FROM rbac_artifact_role_permission_operations",
        sqlite_uuid("tenant_id"),
    )).await?;
    db.execute_unprepared(&format!(r#"
CREATE TRIGGER rbac_artifact_operation_witness_guard
BEFORE INSERT ON rbac_artifact_operation_witnesses
WHEN NEW.tenant_id IS NULL OR typeof(NEW.tenant_id) <> 'text'
 OR length(NEW.tenant_id) <> 32 OR NEW.tenant_id GLOB '*[^0-9a-f]*'
 OR NEW.idempotency_key IS NULL OR typeof(NEW.idempotency_key) <> 'text'
 OR EXISTS (SELECT 1 FROM rbac_artifact_operation_witnesses
     WHERE tenant_id = NEW.tenant_id AND idempotency_key = NEW.idempotency_key)
BEGIN SELECT RAISE(ABORT, 'RBAC artifact operation witness invalid or consumed'); END;
CREATE TRIGGER rbac_artifact_operation_witness_no_update
BEFORE UPDATE ON rbac_artifact_operation_witnesses
BEGIN SELECT RAISE(ABORT, 'RBAC artifact operation witness is permanent'); END;
CREATE TRIGGER rbac_artifact_operation_witness_no_delete
BEFORE DELETE ON rbac_artifact_operation_witnesses
BEGIN SELECT RAISE(ABORT, 'RBAC artifact operation witness is permanent'); END;
CREATE TRIGGER rbac_artifact_operation_receipt_insert_guard
BEFORE INSERT ON rbac_artifact_role_permission_operations
WHEN NEW.idempotency_key IS NULL OR typeof(NEW.idempotency_key) <> 'text'
 OR ({tenant}) IS NULL OR length({tenant}) <> 32 OR ({tenant}) GLOB '*[^0-9a-f]*'
 OR EXISTS (SELECT 1 FROM rbac_artifact_operation_witnesses
     WHERE tenant_id = ({tenant}) AND idempotency_key = NEW.idempotency_key)
 OR EXISTS (SELECT 1 FROM rbac_artifact_role_permission_operations
     WHERE id = NEW.id OR rowid = NEW.rowid)
BEGIN SELECT RAISE(ABORT, 'RBAC artifact operation receipt identity invalid or consumed'); END;
CREATE TRIGGER rbac_artifact_operation_receipt_no_update
BEFORE UPDATE ON rbac_artifact_role_permission_operations
BEGIN SELECT RAISE(ABORT, 'RBAC artifact operation receipt is immutable'); END;
CREATE TRIGGER rbac_artifact_operation_witness_claim
AFTER INSERT ON rbac_artifact_role_permission_operations
BEGIN
    INSERT INTO rbac_artifact_operation_witnesses (tenant_id, idempotency_key)
        VALUES (({tenant}), NEW.idempotency_key);
END;
"#, tenant=sqlite_uuid("NEW.tenant_id"))).await?;
    Ok(())
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> { Some(true) }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => { db.execute_unprepared(POSTGRES).await?; }
            DatabaseBackend::Sqlite => sqlite_up(db).await?,
            _ => return Err(DbErr::Migration("unsupported artifact operation witness backend".into())),
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("artifact operation witnesses cannot be discarded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, TransactionTrait};

    const T: &str = "10000000-0000-0000-0000-000000000001";

    #[tokio::test]
    #[ignore = "requires RUSTOK_RBAC_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_receipt_cleanup_and_direct_writers_share_permanent_key_fence() {
        use sea_orm_migration::sea_orm::{DatabaseConnection, DatabaseTransaction, IsolationLevel};
        use std::time::Duration;
        use uuid::Uuid;

        async fn tx(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
            let tx = db.begin_with_config(Some(IsolationLevel::ReadCommitted), None).await.unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
            tx
        }
        let db = Database::connect(std::env::var("RUSTOK_RBAC_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("rbac_receipt_witness_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = tx(&db, &schema).await;
        setup.execute_unprepared(&format!(
            "CREATE TABLE rbac_artifact_role_permission_operations (
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, idempotency_key TEXT NOT NULL,
                UNIQUE (tenant_id, idempotency_key));
             INSERT INTO rbac_artifact_role_permission_operations VALUES ('{}', '{T}', 'used')",
            Uuid::new_v4(),
        )).await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.commit().await.unwrap();

        let holder = tx(&db, &schema).await;
        holder.execute_unprepared(&format!(
            "SELECT rbac_try_artifact_operation_key('{T}', 'used')"
        )).await.unwrap();
        let contender = tx(&db, &schema).await;
        let error = tokio::time::timeout(Duration::from_secs(3),
            contender.execute_unprepared("DELETE FROM rbac_artifact_role_permission_operations WHERE idempotency_key = 'used'"))
            .await.expect("receipt cleanup must not wait on a held key").unwrap_err();
        assert_eq!(
            rustok_core::resilience::retry::PostgresTransactionConflict::classify(DatabaseBackend::Postgres, &error),
            Some(rustok_core::resilience::retry::PostgresTransactionConflict::LockUnavailable),
        );
        contender.rollback().await.unwrap();
        holder.rollback().await.unwrap();

        let cleanup = tx(&db, &schema).await;
        cleanup.execute_unprepared("DELETE FROM rbac_artifact_role_permission_operations").await.unwrap();
        cleanup.commit().await.unwrap();
        for sql in [
            format!("INSERT INTO rbac_artifact_role_permission_operations VALUES ('{}', '{T}', 'used') ON CONFLICT DO NOTHING", Uuid::new_v4()),
            "DELETE FROM rbac_artifact_operation_witnesses".into(),
            "UPDATE rbac_artifact_operation_witnesses SET idempotency_key = 'other'".into(),
            "TRUNCATE rbac_artifact_operation_witnesses".into(),
            "TRUNCATE rbac_artifact_role_permission_operations".into(),
        ] {
            let rejected = tx(&db, &schema).await;
            assert!(rejected.execute_unprepared(&sql).await.is_err(), "{sql}");
            rejected.rollback().await.unwrap();
        }

        let holder = tx(&db, &schema).await;
        holder.execute_unprepared(&format!(
            "SELECT rbac_try_artifact_operation_key('{T}', 'fresh')"
        )).await.unwrap();
        let contender = tx(&db, &schema).await;
        let insert = format!(
            "INSERT INTO rbac_artifact_role_permission_operations VALUES ('{}', '{T}', 'fresh')",
            Uuid::new_v4(),
        );
        let error = tokio::time::timeout(Duration::from_secs(3), contender.execute_unprepared(&insert))
            .await.expect("raw insertion must not wait on a held key").unwrap_err();
        assert_eq!(
            rustok_core::resilience::retry::PostgresTransactionConflict::classify(DatabaseBackend::Postgres, &error),
            Some(rustok_core::resilience::retry::PostgresTransactionConflict::LockUnavailable),
        );
        contender.rollback().await.unwrap();
        holder.rollback().await.unwrap();
        let rolled_back = tx(&db, &schema).await;
        rolled_back.execute_unprepared(&insert).await.unwrap();
        rolled_back.rollback().await.unwrap();
        let fresh = tx(&db, &schema).await;
        fresh.execute_unprepared(&insert).await.unwrap();
        fresh.commit().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }

    #[tokio::test]
    async fn sqlite_claim_survives_cleanup_and_rejects_replace_and_uuid_alias() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(&format!(
            "CREATE TABLE rbac_artifact_role_permission_operations (
                id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, idempotency_key TEXT NOT NULL,
                UNIQUE (tenant_id, idempotency_key));
             INSERT INTO rbac_artifact_role_permission_operations VALUES ('old', '{T}', 'used')"
        )).await.unwrap();
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        assert!(db.execute_unprepared("UPDATE rbac_artifact_role_permission_operations SET idempotency_key = 'other'").await.is_err());
        db.execute_unprepared("DELETE FROM rbac_artifact_role_permission_operations").await.unwrap();
        for verb in ["INSERT", "INSERT OR REPLACE", "INSERT OR IGNORE"] {
            assert!(db.execute_unprepared(&format!(
                "{verb} INTO rbac_artifact_role_permission_operations VALUES ('new', '{T}', 'used')"
            )).await.is_err());
        }
        assert!(db.execute_unprepared(
            "INSERT INTO rbac_artifact_role_permission_operations VALUES ('new', X'10000000000000000000000000000001', 'used')"
        ).await.is_err());
        assert!(db.execute_unprepared("DELETE FROM rbac_artifact_operation_witnesses").await.is_err());
        assert!(db.execute_unprepared("UPDATE rbac_artifact_operation_witnesses SET idempotency_key = 'other'").await.is_err());
        assert!(db.execute_unprepared("INSERT OR REPLACE INTO rbac_artifact_operation_witnesses VALUES ('10000000000000000000000000000001', 'used')").await.is_err());
        let tx = db.begin().await.unwrap();
        let insert = format!("INSERT INTO rbac_artifact_role_permission_operations VALUES ('fresh', '{T}', 'fresh')");
        tx.execute_unprepared(&insert).await.unwrap();
        tx.rollback().await.unwrap();
        db.execute_unprepared(&insert).await.unwrap();
        assert!(db.execute_unprepared(&format!("INSERT OR REPLACE INTO rbac_artifact_role_permission_operations VALUES ('fresh', '{T}', 'different')")).await.is_err());
    }
}
