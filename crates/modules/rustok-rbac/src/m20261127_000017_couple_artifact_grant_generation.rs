//! Couple artifact grant mutations to the existing durable RBAC watermark.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
CREATE FUNCTION rbac_fence_artifact_grant_roles() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    role_ids uuid[];
BEGIN
    IF TG_OP = 'INSERT' THEN
        role_ids := ARRAY[NEW.role_id];
    ELSIF TG_OP = 'DELETE' THEN
        role_ids := ARRAY[OLD.role_id];
    ELSE
        IF NEW.id IS DISTINCT FROM OLD.id THEN
            RAISE EXCEPTION 'Artifact grant identity is immutable' USING ERRCODE = '23514';
        END IF;
        IF ROW(NEW.tenant_id, NEW.role_id, NEW.artifact_permission_id, NEW.permission_scope_key)
            IS NOT DISTINCT FROM
           ROW(OLD.tenant_id, OLD.role_id, OLD.artifact_permission_id, OLD.permission_scope_key) THEN
            RETURN NEW;
        END IF;
        role_ids := ARRAY[OLD.role_id, NEW.role_id];
    END IF;
    -- A direct DELETE or UPDATE can already own the grant row. Never wait
    -- for an earlier role fence from this path.
    LOCK TABLE roles IN ROW SHARE MODE NOWAIT;
    IF row_security_active('roles'::regclass) THEN
        RAISE EXCEPTION 'Artifact grant role visibility incomplete' USING ERRCODE = '42501';
    END IF;
    PERFORM 1 FROM roles WHERE id = ANY(role_ids)
        ORDER BY id FOR NO KEY UPDATE NOWAIT;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'Artifact grant role fence busy' USING ERRCODE = '55P03';
END; $$;
CREATE TRIGGER rbac_artifact_grant_role_fence
BEFORE INSERT OR UPDATE OR DELETE ON rbac_artifact_role_permissions
FOR EACH ROW EXECUTE FUNCTION rbac_fence_artifact_grant_roles();

CREATE FUNCTION rbac_advance_artifact_grant_generation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    -- Deferred to the commit tail by default. Even when a caller makes
    -- constraints immediate, never wait on this global row below domain locks.
    LOCK TABLE rbac_invalidation_state IN ROW EXCLUSIVE MODE NOWAIT;
    PERFORM 1 FROM rbac_invalidation_state
      WHERE scope = 'permissions' AND generation >= 0 AND generation < 9223372036854775807
      FOR UPDATE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'RBAC invalidation watermark missing, invalid or exhausted' USING ERRCODE = '23514';
    END IF;
    UPDATE rbac_invalidation_state SET generation = generation + 1, updated_at = CURRENT_TIMESTAMP
      WHERE scope = 'permissions';
    RETURN NULL;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'Artifact grant invalidation watermark busy' USING ERRCODE = '55P03';
END; $$;
CREATE CONSTRAINT TRIGGER rbac_artifact_grant_generation_change
AFTER INSERT OR DELETE ON rbac_artifact_role_permissions
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION rbac_advance_artifact_grant_generation();
CREATE CONSTRAINT TRIGGER rbac_artifact_grant_generation_update
AFTER UPDATE ON rbac_artifact_role_permissions
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW WHEN (
    ROW(NEW.tenant_id, NEW.role_id, NEW.artifact_permission_id, NEW.permission_scope_key)
    IS DISTINCT FROM
    ROW(OLD.tenant_id, OLD.role_id, OLD.artifact_permission_id, OLD.permission_scope_key)
) EXECUTE FUNCTION rbac_advance_artifact_grant_generation();
CREATE FUNCTION rbac_reject_artifact_grant_truncate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'Artifact grant history requires row deletion' USING ERRCODE = '23514';
END; $$;
CREATE TRIGGER rbac_artifact_grant_no_truncate
BEFORE TRUNCATE ON rbac_artifact_role_permissions
FOR EACH STATEMENT EXECUTE FUNCTION rbac_reject_artifact_grant_truncate();
"#;

const SQLITE_IDENTITY: &str = "
CREATE TRIGGER rbac_artifact_grant_id_immutable
BEFORE UPDATE OF id ON rbac_artifact_role_permissions
WHEN NEW.id IS NOT OLD.id
BEGIN SELECT RAISE(ABORT, 'Artifact grant identity is immutable'); END;
";

const SQLITE_ADVANCE: &str = "
    SELECT RAISE(ABORT, 'RBAC invalidation watermark missing, invalid or exhausted')
      WHERE NOT EXISTS (SELECT 1 FROM rbac_invalidation_state
        WHERE scope = 'permissions' AND typeof(generation) = 'integer'
          AND generation >= 0 AND generation < 9223372036854775807);
    UPDATE rbac_invalidation_state SET generation = generation + 1, updated_at = CURRENT_TIMESTAMP
      WHERE scope = 'permissions';
";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();
        let storage_guard = match backend {
            DatabaseBackend::Postgres => {
                db.execute_unprepared("LOCK TABLE rbac_artifact_role_permissions IN SHARE ROW EXCLUSIVE MODE").await?;
                ""
            }
            DatabaseBackend::Sqlite => " AND typeof(generation) = 'integer'",
            _ => return Err(DbErr::Migration("unsupported artifact grant generation backend".into())),
        };
        // Invalidate pre-cutover receipts once; do not reconstruct historical
        // changes or reseed a missing/exhausted watermark.
        let result = db.execute_raw(Statement::from_string(
            backend,
            format!("UPDATE rbac_invalidation_state SET generation = generation + 1, updated_at = CURRENT_TIMESTAMP
                WHERE scope = 'permissions' AND generation >= 0 AND generation < 9223372036854775807{storage_guard}"),
        )).await?;
        if result.rows_affected() != 1 {
            return Err(DbErr::Migration("RBAC artifact generation cutover requires a valid watermark".into()));
        }
        match backend {
            DatabaseBackend::Postgres => { db.execute_unprepared(POSTGRES).await?; }
            DatabaseBackend::Sqlite => {
                db.execute_unprepared(SQLITE_IDENTITY).await?;
                for (event, predicate) in [
                    ("INSERT", ""),
                    ("DELETE", ""),
                    ("UPDATE", "WHEN NEW.tenant_id IS NOT OLD.tenant_id OR NEW.role_id IS NOT OLD.role_id OR NEW.artifact_permission_id IS NOT OLD.artifact_permission_id OR NEW.permission_scope_key IS NOT OLD.permission_scope_key"),
                ] {
                    db.execute_unprepared(&format!(
                        "CREATE TRIGGER rbac_artifact_grant_generation_{event}
                         AFTER {event} ON rbac_artifact_role_permissions {predicate}
                         BEGIN {SQLITE_ADVANCE} END"
                    )).await?;
                }
            }
            _ => return Err(DbErr::Migration("unsupported artifact grant generation backend".into())),
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("artifact grant generation coupling cannot be downgraded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DatabaseConnection, DatabaseTransaction, IsolationLevel, TransactionTrait};
    use std::time::Duration;
    use uuid::Uuid;

    async fn tx(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = db.begin_with_config(Some(IsolationLevel::ReadCommitted), None).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}; SET LOCAL statement_timeout = '3s'"))
            .await.unwrap();
        tx
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_RBAC_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_artifact_grant_generation_is_deferred_and_direct_delete_fenced() {
        let db = Database::connect(std::env::var("RUSTOK_RBAC_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("rbac_artifact_generation_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = tx(&db, &schema).await;
        let role = Uuid::new_v4();
        let tenant = Uuid::new_v4();
        let grant = Uuid::new_v4();
        let permission = Uuid::new_v4();
        setup.execute_unprepared(&format!(
            "CREATE TABLE roles (id UUID PRIMARY KEY);
             CREATE TABLE rbac_invalidation_state (scope TEXT PRIMARY KEY, generation BIGINT NOT NULL, updated_at TIMESTAMPTZ NOT NULL);
             CREATE TABLE rbac_artifact_role_permissions (id UUID PRIMARY KEY, tenant_id UUID NOT NULL,
                 role_id UUID NOT NULL REFERENCES roles(id), artifact_permission_id UUID NOT NULL,
                 permission_scope_key TEXT NOT NULL);
             INSERT INTO roles VALUES ('{role}');
             INSERT INTO rbac_invalidation_state VALUES ('permissions', 0, CURRENT_TIMESTAMP);
             INSERT INTO rbac_artifact_role_permissions VALUES ('{grant}','{tenant}','{role}','{permission}','platform');"
        )).await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.commit().await.unwrap();

        let holder = tx(&db, &schema).await;
        holder.execute_unprepared("SELECT 1 FROM roles FOR UPDATE").await.unwrap();
        let writer = tx(&db, &schema).await;
        let error = tokio::time::timeout(Duration::from_secs(5),
            writer.execute_unprepared("DELETE FROM rbac_artifact_role_permissions"))
            .await.expect("late role fence must not wait").unwrap_err();
        assert!(error.to_string().contains("Artifact grant role fence busy"), "{error}");
        writer.rollback().await.unwrap();
        holder.rollback().await.unwrap();

        let holder = tx(&db, &schema).await;
        holder.execute_unprepared("SELECT 1 FROM rbac_invalidation_state FOR UPDATE").await.unwrap();
        let writer = tx(&db, &schema).await;
        writer.execute_unprepared("DELETE FROM rbac_artifact_role_permissions").await.unwrap();
        assert_eq!(crate::read_permission_invalidation_generation(&writer).await.unwrap(), 1);
        let error = tokio::time::timeout(Duration::from_secs(5), writer.commit())
            .await.expect("late watermark must not wait").unwrap_err();
        assert!(error.to_string().contains("Artifact grant invalidation watermark busy"), "{error}");
        holder.rollback().await.unwrap();

        let writer = tx(&db, &schema).await;
        assert_eq!(writer.execute_unprepared("DELETE FROM rbac_artifact_role_permissions").await.unwrap().rows_affected(), 1);
        assert_eq!(crate::read_permission_invalidation_generation(&writer).await.unwrap(), 1);
        writer.rollback().await.unwrap();
        let writer = tx(&db, &schema).await;
        writer.execute_unprepared("UPDATE rbac_artifact_role_permissions SET role_id = role_id").await.unwrap();
        writer.commit().await.unwrap();
        let writer = tx(&db, &schema).await;
        assert_eq!(crate::read_permission_invalidation_generation(&writer).await.unwrap(), 1);
        assert_eq!(writer.execute_unprepared("DELETE FROM rbac_artifact_role_permissions").await.unwrap().rows_affected(), 1);
        writer.commit().await.unwrap();
        let check = tx(&db, &schema).await;
        assert_eq!(crate::read_permission_invalidation_generation(&check).await.unwrap(), 2);
        assert!(check.execute_unprepared("TRUNCATE rbac_artifact_role_permissions").await.is_err());
        check.rollback().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
