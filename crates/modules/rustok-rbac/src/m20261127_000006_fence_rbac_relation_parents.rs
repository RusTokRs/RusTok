//! Prevent blocking reverse edges from RBAC relation rows to authority parents.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES_GUARDS: &str = r#"
CREATE FUNCTION rbac_fence_membership_parents() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    subject_ids uuid[];
    role_ids uuid[];
BEGIN
    -- Hold table visibility stable before inspecting parent RLS. Do not
    -- wait for conflicting DDL while already holding a child row.
    LOCK TABLE users, roles IN ROW SHARE MODE NOWAIT;
    -- The revision writer must see every surviving subject, including cascades.
    IF row_security_active('users'::regclass) OR row_security_active('roles'::regclass) THEN
        RAISE EXCEPTION 'RBAC membership parent visibility is incomplete' USING ERRCODE = '42501';
    END IF;
    IF TG_OP = 'INSERT' THEN
        subject_ids := ARRAY[NEW.user_id];
        role_ids := ARRAY[NEW.role_id];
    ELSIF TG_OP = 'UPDATE' THEN
        IF NEW.id IS DISTINCT FROM OLD.id THEN
            RAISE EXCEPTION 'RBAC membership ID is immutable' USING ERRCODE = '23514';
        END IF;
        subject_ids := ARRAY[OLD.user_id, NEW.user_id];
        role_ids := ARRAY[OLD.role_id, NEW.role_id];
    ELSE
        subject_ids := ARRAY[OLD.user_id];
        -- DELETE requires no new role FK lock. The role or user may already
        -- be gone in an ON DELETE CASCADE from this transaction.
        role_ids := ARRAY[]::uuid[];
    END IF;

    -- UPDATE/DELETE may already own the relation row. Never wait for a
    -- parent here. NO KEY UPDATE is sufficient for the revision increment;
    -- SHARE protects a referenced role's non-key tenant changes as well.
    PERFORM 1 FROM users WHERE id = ANY(subject_ids)
        ORDER BY id FOR NO KEY UPDATE NOWAIT;
    PERFORM 1 FROM roles WHERE id = ANY(role_ids)
        ORDER BY id FOR SHARE NOWAIT;
    -- Existing tenant-integrity BEFORE triggers run after this guard
    -- (trigger names sort lexically); they validate the locked NEW parents.
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'RBAC membership parent fence busy' USING ERRCODE = '55P03';
END; $$;
CREATE TRIGGER rbac_membership_parent_fence
BEFORE INSERT OR UPDATE OR DELETE ON user_roles
FOR EACH ROW EXECUTE FUNCTION rbac_fence_membership_parents();

CREATE FUNCTION rbac_fence_grant_parents() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    role_ids uuid[];
    permission_ids uuid[];
BEGIN
    LOCK TABLE roles, permissions IN ROW SHARE MODE NOWAIT;
    IF row_security_active('roles'::regclass) OR row_security_active('permissions'::regclass) THEN
        RAISE EXCEPTION 'RBAC grant parent visibility is incomplete' USING ERRCODE = '42501';
    END IF;
    IF TG_OP = 'INSERT' THEN
        role_ids := ARRAY[NEW.role_id];
        permission_ids := ARRAY[NEW.permission_id];
    ELSIF TG_OP = 'UPDATE' THEN
        IF NEW.id IS DISTINCT FROM OLD.id THEN
            RAISE EXCEPTION 'RBAC grant ID is immutable' USING ERRCODE = '23514';
        END IF;
        role_ids := ARRAY[OLD.role_id, NEW.role_id];
        permission_ids := ARRAY[OLD.permission_id, NEW.permission_id];
    ELSE
        role_ids := ARRAY[OLD.role_id];
        permission_ids := ARRAY[]::uuid[];
    END IF;

    PERFORM 1 FROM roles WHERE id = ANY(role_ids)
        ORDER BY id FOR NO KEY UPDATE NOWAIT;
    PERFORM 1 FROM permissions WHERE id = ANY(permission_ids)
        ORDER BY id FOR SHARE NOWAIT;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'RBAC grant parent fence busy' USING ERRCODE = '55P03';
END; $$;
CREATE TRIGGER rbac_grant_parent_fence
BEFORE INSERT OR UPDATE OR DELETE ON role_permissions
FOR EACH ROW EXECUTE FUNCTION rbac_fence_grant_parents();

-- The owner path already holds these exact FK parents before inserting a
-- grant/receipt. Direct SQL must not wait on them from a child row either.
CREATE FUNCTION rbac_fence_artifact_authority_parents() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    acting_user uuid;
BEGIN
    IF TG_TABLE_NAME = 'rbac_artifact_role_permission_operations' THEN
        acting_user := NEW.actor_id;
    ELSE
        acting_user := NEW.granted_by_actor_id;
    END IF;
    PERFORM 1 FROM users WHERE id = acting_user AND tenant_id = NEW.tenant_id
        FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'RBAC artifact actor parent missing' USING ERRCODE = '23503';
    END IF;
    PERFORM 1 FROM roles WHERE id = NEW.role_id AND tenant_id = NEW.tenant_id
        FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'RBAC artifact role parent missing' USING ERRCODE = '23503';
    END IF;
    PERFORM 1 FROM rbac_artifact_permission_definitions
        WHERE id = NEW.artifact_permission_id AND scope_key = NEW.permission_scope_key
        FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'RBAC artifact definition parent missing' USING ERRCODE = '23503';
    END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'RBAC artifact authority parent fence busy' USING ERRCODE = '55P03';
END; $$;
CREATE TRIGGER rbac_artifact_grant_parent_fence
BEFORE INSERT OR UPDATE ON rbac_artifact_role_permissions
FOR EACH ROW EXECUTE FUNCTION rbac_fence_artifact_authority_parents();
CREATE TRIGGER rbac_artifact_receipt_parent_fence
BEFORE INSERT OR UPDATE ON rbac_artifact_role_permission_operations
FOR EACH ROW EXECUTE FUNCTION rbac_fence_artifact_authority_parents();
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager.get_connection().execute_unprepared(POSTGRES_GUARDS).await?;
            }
            // SQLite serializes writers and retains the existing triggers.
            DatabaseBackend::Sqlite => {}
            _ => return Err(DbErr::Custom("unsupported RBAC parent fence backend".into())),
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager.get_connection().execute_unprepared(
                    "DROP TRIGGER rbac_artifact_receipt_parent_fence ON rbac_artifact_role_permission_operations;
                     DROP TRIGGER rbac_artifact_grant_parent_fence ON rbac_artifact_role_permissions;
                     DROP TRIGGER rbac_grant_parent_fence ON role_permissions;
                     DROP TRIGGER rbac_membership_parent_fence ON user_roles;
                     DROP FUNCTION rbac_fence_artifact_authority_parents();
                     DROP FUNCTION rbac_fence_grant_parents();
                     DROP FUNCTION rbac_fence_membership_parents();"
                ).await?;
            }
            DatabaseBackend::Sqlite => {}
            _ => return Err(DbErr::Custom("unsupported RBAC parent fence backend".into())),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DatabaseConnection, DatabaseTransaction, IsolationLevel, TransactionTrait};
    use std::time::Duration;
    use uuid::Uuid;

    const U: &str = "00000000-0000-0000-0000-000000000001";
    const V: &str = "00000000-0000-0000-0000-000000000002";
    const R: &str = "00000000-0000-0000-0000-000000000003";
    const S: &str = "00000000-0000-0000-0000-000000000004";
    const P: &str = "00000000-0000-0000-0000-000000000005";
    const T: &str = "00000000-0000-0000-0000-000000000006";
    const M: &str = "00000000-0000-0000-0000-000000000007";
    const G: &str = "00000000-0000-0000-0000-000000000008";

    async fn tx(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = db.begin_with_config(Some(IsolationLevel::ReadCommitted), None).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}; SET LOCAL statement_timeout = '3s'"))
            .await.unwrap();
        tx
    }

    async fn reject_busy(db: &DatabaseConnection, schema: &str, sql: &str, message: &str) {
        let writer = tx(db, schema).await;
        let error = tokio::time::timeout(Duration::from_secs(5), writer.execute_unprepared(sql))
            .await.expect("late parent acquisition must not wait")
            .expect_err("late parent acquisition must fail");
        assert!(error.to_string().contains(message), "{error}");
        writer.rollback().await.unwrap();
    }

    async fn revision(tx: &DatabaseTransaction, table: &str, column: &str, id: &str) -> i64 {
        tx.query_one_raw(sea_orm::Statement::from_string(
            DatabaseBackend::Postgres,
            format!("SELECT {column} AS revision FROM {table} WHERE id = '{id}'"),
        )).await.unwrap().unwrap().try_get("", "revision").unwrap()
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_RBAC_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_relation_and_receipt_parent_fences_reject_late_locks() {
        let db = Database::connect(std::env::var("RUSTOK_RBAC_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("rbac_parent_fence_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = tx(&db, &schema).await;
        setup.execute_unprepared(
            "CREATE TABLE users (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, UNIQUE(tenant_id,id));
             CREATE TABLE roles (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, slug TEXT NOT NULL, is_system BOOLEAN NOT NULL, UNIQUE(tenant_id,id));
             CREATE TABLE permissions (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, resource TEXT NOT NULL, action TEXT NOT NULL);
             CREATE TABLE user_roles (id UUID PRIMARY KEY, user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE, role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE, UNIQUE(user_id,role_id));
             CREATE TABLE role_permissions (id UUID PRIMARY KEY, role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE, permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE, UNIQUE(role_id,permission_id));
             CREATE TABLE rbac_artifact_permission_definitions (id UUID PRIMARY KEY, scope_key TEXT NOT NULL, UNIQUE(id,scope_key));
             CREATE TABLE rbac_artifact_role_permissions (tenant_id UUID NOT NULL, role_id UUID NOT NULL, artifact_permission_id UUID NOT NULL, permission_scope_key TEXT NOT NULL, granted_by_actor_id UUID NOT NULL, FOREIGN KEY(tenant_id,role_id) REFERENCES roles(tenant_id,id), FOREIGN KEY(tenant_id,granted_by_actor_id) REFERENCES users(tenant_id,id), FOREIGN KEY(artifact_permission_id,permission_scope_key) REFERENCES rbac_artifact_permission_definitions(id,scope_key));
             CREATE TABLE rbac_artifact_role_permission_operations (tenant_id UUID NOT NULL, role_id UUID NOT NULL, artifact_permission_id UUID NOT NULL, permission_scope_key TEXT NOT NULL, actor_id UUID NOT NULL, FOREIGN KEY(tenant_id,role_id) REFERENCES roles(tenant_id,id), FOREIGN KEY(tenant_id,actor_id) REFERENCES users(tenant_id,id), FOREIGN KEY(artifact_permission_id,permission_scope_key) REFERENCES rbac_artifact_permission_definitions(id,scope_key));"
        ).await.unwrap();
        let manager = SchemaManager::new(&setup);
        crate::m20260714_900001_enforce_rbac_relation_tenant_integrity::Migration.up(&manager).await.unwrap();
        crate::m20261127_000001_create_rbac_membership_revision::Migration.up(&manager).await.unwrap();
        crate::m20261127_000003_create_rbac_role_grant_revision::Migration.up(&manager).await.unwrap();
        Migration.up(&manager).await.unwrap();
        setup.execute_unprepared(&format!(
            "INSERT INTO users(id,tenant_id) VALUES ('{U}','{T}'),('{V}','{T}');
             INSERT INTO roles(id,tenant_id,slug,is_system) VALUES ('{R}','{T}','r',TRUE),('{S}','{T}','s',TRUE);
             INSERT INTO permissions(id,tenant_id,resource,action) VALUES ('{P}','{T}','users','read');
             INSERT INTO user_roles VALUES ('{M}','{U}','{R}');
             INSERT INTO role_permissions VALUES ('{G}','{R}','{P}');
             INSERT INTO rbac_artifact_permission_definitions VALUES ('{P}','platform');"
        )).await.unwrap();
        setup.commit().await.unwrap();

        let owner = tx(&db, &schema).await;
        owner.execute_unprepared(&format!("SELECT 1 FROM users WHERE id = '{U}' FOR UPDATE")).await.unwrap();
        reject_busy(&db, &schema, &format!("DELETE FROM user_roles WHERE id = '{M}'"), "RBAC membership parent fence busy").await;
        reject_busy(&db, &schema, &format!("UPDATE user_roles SET user_id = '{V}' WHERE id = '{M}'"), "RBAC membership parent fence busy").await;
        reject_busy(&db, &schema, &format!("INSERT INTO rbac_artifact_role_permission_operations VALUES ('{T}','{R}','{P}','platform','{U}')"), "RBAC artifact authority parent fence busy").await;
        // A role cascade must fail instead of waiting for a user beneath it.
        reject_busy(&db, &schema, &format!("DELETE FROM roles WHERE id = '{R}'"), "RBAC membership parent fence busy").await;
        owner.execute_unprepared(&format!("UPDATE user_roles SET role_id = role_id WHERE id = '{M}'")).await.unwrap();
        assert_eq!(revision(&owner, "users", "rbac_membership_revision", U).await, 1);
        owner.commit().await.unwrap();

        let owner = tx(&db, &schema).await;
        owner.execute_unprepared(&format!("SELECT 1 FROM roles WHERE id = '{R}' FOR UPDATE")).await.unwrap();
        reject_busy(&db, &schema, &format!("DELETE FROM role_permissions WHERE id = '{G}'"), "RBAC grant parent fence busy").await;
        reject_busy(&db, &schema, &format!("UPDATE role_permissions SET role_id = '{S}' WHERE id = '{G}'"), "RBAC grant parent fence busy").await;
        reject_busy(&db, &schema, &format!("DELETE FROM permissions WHERE id = '{P}'"), "RBAC grant parent fence busy").await;
        reject_busy(&db, &schema, &format!("INSERT INTO rbac_artifact_role_permissions VALUES ('{T}','{R}','{P}','platform','{U}')"), "RBAC artifact authority parent fence busy").await;
        owner.execute_unprepared(&format!("DELETE FROM role_permissions WHERE id = '{G}'")).await.unwrap();
        assert_eq!(revision(&owner, "roles", "grant_revision", R).await, 2);
        owner.rollback().await.unwrap();

        let writer = tx(&db, &schema).await;
        assert_eq!(revision(&writer, "roles", "grant_revision", R).await, 1);
        writer.execute_unprepared(&format!("UPDATE user_roles SET user_id = '{V}' WHERE id = '{M}'")).await.unwrap();
        assert_eq!(revision(&writer, "users", "rbac_membership_revision", U).await, 2);
        assert_eq!(revision(&writer, "users", "rbac_membership_revision", V).await, 1);
        writer.execute_unprepared(&format!("DELETE FROM permissions WHERE id = '{P}'")).await.unwrap();
        assert_eq!(revision(&writer, "roles", "grant_revision", R).await, 2);
        writer.execute_unprepared(&format!("DELETE FROM roles WHERE id = '{R}'")).await.unwrap();
        assert_eq!(revision(&writer, "users", "rbac_membership_revision", V).await, 2);
        writer.commit().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
