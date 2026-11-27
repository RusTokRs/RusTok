//! Couple static relation mutations to the existing durable RBAC watermark.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
CREATE FUNCTION rbac_advance_static_relation_generation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    LOCK TABLE rbac_invalidation_state IN ROW EXCLUSIVE MODE NOWAIT;
    IF row_security_active('rbac_invalidation_state'::regclass) THEN
        RAISE EXCEPTION 'RBAC invalidation watermark visibility incomplete' USING ERRCODE = '42501';
    END IF;
    PERFORM 1 FROM rbac_invalidation_state
        WHERE scope = 'permissions' AND generation >= 0 AND generation < 9223372036854775807
        FOR UPDATE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'RBAC invalidation watermark missing, invalid or exhausted' USING ERRCODE = '23514';
    END IF;
    UPDATE rbac_invalidation_state SET generation = generation + 1, updated_at = CURRENT_TIMESTAMP
        WHERE scope = 'permissions';
    IF NOT FOUND THEN
        RAISE EXCEPTION 'RBAC invalidation watermark update failed' USING ERRCODE = '23514';
    END IF;
    RETURN NULL;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'Static RBAC invalidation watermark busy' USING ERRCODE = '55P03';
END $$;
CREATE CONSTRAINT TRIGGER rbac_static_membership_generation_change
AFTER INSERT OR DELETE ON user_roles DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION rbac_advance_static_relation_generation();
CREATE CONSTRAINT TRIGGER rbac_static_membership_generation_update
AFTER UPDATE ON user_roles DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW WHEN (ROW(NEW.user_id, NEW.role_id) IS DISTINCT FROM ROW(OLD.user_id, OLD.role_id))
EXECUTE FUNCTION rbac_advance_static_relation_generation();
CREATE CONSTRAINT TRIGGER rbac_static_grant_generation_change
AFTER INSERT OR DELETE ON role_permissions DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION rbac_advance_static_relation_generation();
CREATE CONSTRAINT TRIGGER rbac_static_grant_generation_update
AFTER UPDATE ON role_permissions DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW WHEN (ROW(NEW.role_id, NEW.permission_id) IS DISTINCT FROM ROW(OLD.role_id, OLD.permission_id))
EXECUTE FUNCTION rbac_advance_static_relation_generation();
CREATE CONSTRAINT TRIGGER rbac_static_role_identity_generation
AFTER UPDATE ON roles DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW WHEN (ROW(NEW.tenant_id, NEW.slug, NEW.is_system) IS DISTINCT FROM ROW(OLD.tenant_id, OLD.slug, OLD.is_system))
EXECUTE FUNCTION rbac_advance_static_relation_generation();
"#;

const SQLITE_ADVANCE: &str = r#"
    SELECT RAISE(ABORT, 'RBAC invalidation watermark missing, invalid or exhausted')
        WHERE NOT EXISTS (SELECT 1 FROM rbac_invalidation_state
          WHERE scope = 'permissions' AND typeof(generation) = 'integer'
            AND generation >= 0 AND generation < 9223372036854775807);
    UPDATE rbac_invalidation_state SET generation = generation + 1, updated_at = CURRENT_TIMESTAMP
        WHERE scope = 'permissions';
"#;

async fn install_sqlite(db: &impl ConnectionTrait) -> Result<(), DbErr> {
    for (table, predicate) in [
        ("user_roles", "WHEN NEW.user_id IS NOT OLD.user_id OR NEW.role_id IS NOT OLD.role_id"),
        ("role_permissions", "WHEN NEW.role_id IS NOT OLD.role_id OR NEW.permission_id IS NOT OLD.permission_id"),
    ] {
        for (event, filter) in [("INSERT", ""), ("DELETE", ""), ("UPDATE", predicate)] {
            db.execute_unprepared(&format!(
                "CREATE TRIGGER rbac_static_{table}_generation_{event}
                 AFTER {event} ON {table} {filter}
                 BEGIN {SQLITE_ADVANCE} END"
            )).await?;
        }
    }
    db.execute_unprepared(&format!(
        "CREATE TRIGGER rbac_static_role_identity_generation
         AFTER UPDATE ON roles
         WHEN NEW.tenant_id IS NOT OLD.tenant_id OR NEW.slug IS NOT OLD.slug OR NEW.is_system IS NOT OLD.is_system
         BEGIN {SQLITE_ADVANCE} END"
    )).await?;
    Ok(())
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> { Some(true) }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();
        let guard = match backend {
            DatabaseBackend::Postgres => {
                // Drain writers at cutover; never reconstruct past changes.
                db.execute_unprepared("LOCK TABLE roles, user_roles, role_permissions IN SHARE ROW EXCLUSIVE MODE").await?;
                ""
            }
            DatabaseBackend::Sqlite => " AND typeof(generation) = 'integer'",
            _ => return Err(DbErr::Migration("unsupported static RBAC generation backend".into())),
        };
        let result = db.execute_raw(Statement::from_string(
            backend,
            format!("UPDATE rbac_invalidation_state SET generation = generation + 1, updated_at = CURRENT_TIMESTAMP
                     WHERE scope = 'permissions' AND generation >= 0 AND generation < 9223372036854775807{guard}"),
        )).await?;
        if result.rows_affected() != 1 {
            return Err(DbErr::Migration("static RBAC generation cutover requires a valid watermark".into()));
        }
        match backend {
            DatabaseBackend::Postgres => { db.execute_unprepared(POSTGRES).await?; }
            DatabaseBackend::Sqlite => install_sqlite(db).await?,
            _ => return Err(DbErr::Migration("unsupported static RBAC generation backend".into())),
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("static RBAC generation coupling cannot be downgraded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection, DatabaseTransaction, IsolationLevel, TransactionTrait};
    use crate::{flush_static_relation_invalidation_generation, read_permission_invalidation_generation};
    use uuid::Uuid;

    async fn setup(db: &impl ConnectionTrait) {
        db.execute_unprepared(
            "CREATE TABLE users (id TEXT PRIMARY KEY);
             CREATE TABLE roles (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, slug TEXT NOT NULL, is_system BOOLEAN NOT NULL, name TEXT);
             CREATE TABLE permissions (id TEXT PRIMARY KEY);
             CREATE TABLE user_roles (id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE, role_id TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE, UNIQUE(user_id,role_id));
             CREATE TABLE role_permissions (id TEXT PRIMARY KEY, role_id TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE, permission_id TEXT NOT NULL REFERENCES permissions(id) ON DELETE CASCADE, UNIQUE(role_id,permission_id));
             CREATE TABLE rbac_invalidation_state (scope TEXT PRIMARY KEY, generation BIGINT NOT NULL, updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP);
             INSERT INTO rbac_invalidation_state (scope,generation) VALUES ('permissions', 0);
             INSERT INTO users VALUES ('u');
             INSERT INTO roles VALUES ('r','t','editor',FALSE,'Editor');
             INSERT INTO permissions VALUES ('p'), ('q');"
        ).await.unwrap();
    }

    #[tokio::test]
    async fn sqlite_static_changes_noops_cascades_and_rollback_advance_exactly() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA foreign_keys = ON").await.unwrap();
        setup(&db).await;
        let tx = db.begin().await.unwrap();
        Migration.up(&SchemaManager::new(&tx)).await.unwrap();
        tx.commit().await.unwrap();
        let initial = read_permission_invalidation_generation(&db).await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("INSERT INTO user_roles VALUES ('m','u','r'); INSERT INTO role_permissions VALUES ('g','r','p')").await.unwrap();
        assert_eq!(flush_static_relation_invalidation_generation(&tx).await.unwrap(), initial + 2);
        tx.rollback().await.unwrap();
        assert_eq!(read_permission_invalidation_generation(&db).await.unwrap(), initial);

        db.execute_unprepared("INSERT INTO user_roles VALUES ('m','u','r'); INSERT INTO role_permissions VALUES ('g','r','p')").await.unwrap();
        assert_eq!(read_permission_invalidation_generation(&db).await.unwrap(), initial + 2);
        db.execute_unprepared("UPDATE user_roles SET role_id = role_id; UPDATE role_permissions SET permission_id = permission_id; UPDATE roles SET name = 'Display'").await.unwrap();
        assert_eq!(read_permission_invalidation_generation(&db).await.unwrap(), initial + 2);
        db.execute_unprepared("UPDATE role_permissions SET permission_id = 'q'; UPDATE roles SET slug = 'author'").await.unwrap();
        assert_eq!(read_permission_invalidation_generation(&db).await.unwrap(), initial + 4);
        db.execute_unprepared("DELETE FROM permissions WHERE id = 'q'; DELETE FROM users WHERE id = 'u'").await.unwrap();
        assert_eq!(read_permission_invalidation_generation(&db).await.unwrap(), initial + 6);

        db.execute_unprepared("UPDATE rbac_invalidation_state SET generation = 9223372036854775807").await.unwrap();
        assert!(db.execute_unprepared("INSERT INTO role_permissions VALUES ('g2','r','p')").await.is_err());
        let row = db.query_one_raw(Statement::from_string(DatabaseBackend::Sqlite, "SELECT COUNT(*) AS total FROM role_permissions")).await.unwrap().unwrap();
        assert_eq!(row.try_get::<i64>("", "total").unwrap(), 0);
    }

    async fn tx(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = db.begin_with_config(Some(IsolationLevel::ReadCommitted), None).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}; SET LOCAL statement_timeout = '3s'")).await.unwrap();
        tx
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_RBAC_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_static_generation_is_deferred_and_fails_without_waiting() {
        let db = Database::connect(std::env::var("RUSTOK_RBAC_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("rbac_static_watermark_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let init = tx(&db, &schema).await;
        setup(&init).await;
        Migration.up(&SchemaManager::new(&init)).await.unwrap();
        init.commit().await.unwrap();

        let holder = tx(&db, &schema).await;
        let generation = crate::reserve_permission_invalidation_generation(&holder).await.unwrap();
        let contender = tx(&db, &schema).await;
        contender.execute_unprepared("INSERT INTO user_roles VALUES ('m','u','r')").await.unwrap();
        let error = tokio::time::timeout(std::time::Duration::from_secs(2),
            flush_static_relation_invalidation_generation(&contender)).await.unwrap().unwrap_err();
        assert!(matches!(error, crate::RbacInvalidationGenerationError::Database(ref error)
            if rustok_core::resilience::retry::PostgresTransactionConflict::classify(DatabaseBackend::Postgres, error)
                == Some(rustok_core::resilience::retry::PostgresTransactionConflict::LockUnavailable)));
        contender.rollback().await.unwrap();
        holder.rollback().await.unwrap();

        let retry = tx(&db, &schema).await;
        retry.execute_unprepared("INSERT INTO user_roles VALUES ('m','u','r'); INSERT INTO role_permissions VALUES ('g','r','p')").await.unwrap();
        assert_eq!(read_permission_invalidation_generation(&retry).await.unwrap(), generation - 1);
        assert_eq!(flush_static_relation_invalidation_generation(&retry).await.unwrap(), generation + 1);
        assert_eq!(flush_static_relation_invalidation_generation(&retry).await.unwrap(), generation + 1);
        retry.commit().await.unwrap();
        let raw = tx(&db, &schema).await;
        raw.execute_unprepared("DELETE FROM permissions WHERE id = 'p'").await.unwrap();
        raw.commit().await.unwrap();
        let verify = tx(&db, &schema).await;
        assert_eq!(read_permission_invalidation_generation(&verify).await.unwrap(), generation + 2);
        verify.rollback().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
