//! Static RBAC FK cascades must not wait on a child or surviving revision owner.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
CREATE FUNCTION rbac_fence_static_parent_cascade() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_TABLE_NAME = 'users' THEN
        LOCK TABLE user_roles IN ROW EXCLUSIVE MODE NOWAIT;
        LOCK TABLE roles IN ROW SHARE MODE NOWAIT;
        IF row_security_active('user_roles'::regclass) OR row_security_active('roles'::regclass)
           OR row_security_active('users'::regclass) THEN
            RAISE EXCEPTION 'RBAC cascade visibility incomplete' USING ERRCODE = '42501';
        END IF;
        PERFORM 1 FROM user_roles WHERE user_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
    ELSIF TG_TABLE_NAME = 'roles' THEN
        LOCK TABLE users, user_roles, role_permissions IN ROW EXCLUSIVE MODE NOWAIT;
        LOCK TABLE permissions IN ROW SHARE MODE NOWAIT;
        IF row_security_active('users'::regclass) OR row_security_active('roles'::regclass)
           OR row_security_active('user_roles'::regclass) OR row_security_active('role_permissions'::regclass)
           OR row_security_active('permissions'::regclass) THEN
            RAISE EXCEPTION 'RBAC cascade visibility incomplete' USING ERRCODE = '42501';
        END IF;
        -- A role DELETE already owns a later authority parent. Never wait
        -- for a user whose membership revision the cascade must advance.
        PERFORM 1 FROM users u WHERE EXISTS (
            SELECT 1 FROM user_roles m WHERE m.role_id = OLD.id AND m.user_id = u.id
        ) ORDER BY u.id FOR NO KEY UPDATE OF u NOWAIT;
        PERFORM 1 FROM user_roles WHERE role_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
        PERFORM 1 FROM role_permissions WHERE role_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
    ELSIF TG_TABLE_NAME = 'permissions' THEN
        LOCK TABLE roles, role_permissions IN ROW EXCLUSIVE MODE NOWAIT;
        IF row_security_active('roles'::regclass) OR row_security_active('permissions'::regclass)
           OR row_security_active('role_permissions'::regclass) THEN
            RAISE EXCEPTION 'RBAC cascade visibility incomplete' USING ERRCODE = '42501';
        END IF;
        PERFORM 1 FROM roles r WHERE EXISTS (
            SELECT 1 FROM role_permissions g WHERE g.permission_id = OLD.id AND g.role_id = r.id
        ) ORDER BY r.id FOR NO KEY UPDATE OF r NOWAIT;
        PERFORM 1 FROM role_permissions WHERE permission_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
    ELSE
        RAISE EXCEPTION 'Unknown static RBAC cascade parent' USING ERRCODE = '23514';
    END IF;
    RETURN OLD;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'RBAC static parent cascade fence busy' USING ERRCODE = '55P03';
END; $$;
CREATE TRIGGER rbac_user_static_cascade_fence BEFORE DELETE ON users
FOR EACH ROW EXECUTE FUNCTION rbac_fence_static_parent_cascade();
CREATE TRIGGER rbac_role_static_cascade_fence BEFORE DELETE ON roles
FOR EACH ROW EXECUTE FUNCTION rbac_fence_static_parent_cascade();
CREATE TRIGGER rbac_permission_static_cascade_fence BEFORE DELETE ON permissions
FOR EACH ROW EXECUTE FUNCTION rbac_fence_static_parent_cascade();
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> { Some(true) }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager.get_connection().execute_unprepared(POSTGRES).await?;
                Ok(())
            }
            DatabaseBackend::Sqlite => Ok(()),
            _ => Err(DbErr::Migration("unsupported RBAC static cascade backend".into())),
        }
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("RBAC static cascade fence cannot be downgraded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection, DatabaseTransaction, Statement, TransactionTrait};
    use std::time::Duration;
    use uuid::Uuid;

    const U: &str = "10000000-0000-0000-0000-000000000001";
    const V: &str = "10000000-0000-0000-0000-000000000002";
    const R: &str = "20000000-0000-0000-0000-000000000001";
    const S: &str = "20000000-0000-0000-0000-000000000002";
    const P: &str = "30000000-0000-0000-0000-000000000001";
    const Q: &str = "30000000-0000-0000-0000-000000000002";
    const M: &str = "40000000-0000-0000-0000-000000000001";
    const N: &str = "40000000-0000-0000-0000-000000000002";
    const T: &str = "50000000-0000-0000-0000-000000000001";

    async fn tx(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx
    }

    async fn value(tx: &DatabaseTransaction, sql: &str) -> i64 {
        tx.query_one_raw(Statement::from_string(DatabaseBackend::Postgres, sql.to_owned()))
            .await.unwrap().unwrap().try_get("", "value").unwrap()
    }

    fn assert_busy(error: &DbErr) {
        assert_eq!(
            rustok_core::resilience::retry::PostgresTransactionConflict::classify(DatabaseBackend::Postgres, error),
            Some(rustok_core::resilience::retry::PostgresTransactionConflict::LockUnavailable),
        );
    }

    async fn reject_busy(db: &DatabaseConnection, schema: &str, lock: &str, deletion: &str) {
        let holder = tx(db, schema).await;
        holder.execute_unprepared(lock).await.unwrap();
        let writer = tx(db, schema).await;
        let error = tokio::time::timeout(Duration::from_secs(3), writer.execute_unprepared(deletion))
            .await.expect("static cascade must not wait").unwrap_err();
        assert_busy(&error);
        writer.rollback().await.unwrap();
        holder.rollback().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_RBAC_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_static_parent_cascades_fence_children_and_keep_generation_atomic() {
        let db = Database::connect(std::env::var("RUSTOK_RBAC_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("rbac_parent_cascade_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = tx(&db, &schema).await;
        setup.execute_unprepared(&format!(
            "CREATE TABLE users (id UUID PRIMARY KEY, tenant_id UUID NOT NULL);
             CREATE TABLE roles (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, slug TEXT NOT NULL, is_system BOOLEAN NOT NULL);
             CREATE TABLE permissions (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, resource TEXT, action TEXT);
             CREATE TABLE user_roles (id UUID PRIMARY KEY, user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                 role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE, UNIQUE(user_id,role_id));
             CREATE TABLE role_permissions (id UUID PRIMARY KEY, role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
                 permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE, UNIQUE(role_id,permission_id));
             CREATE TABLE rbac_artifact_permission_definitions (id UUID PRIMARY KEY, scope_key TEXT);
             CREATE TABLE rbac_artifact_role_permissions (tenant_id UUID, role_id UUID, artifact_permission_id UUID, permission_scope_key TEXT, granted_by_actor_id UUID);
             CREATE TABLE rbac_artifact_role_permission_operations (tenant_id UUID, role_id UUID, artifact_permission_id UUID, permission_scope_key TEXT, actor_id UUID);
             CREATE TABLE rbac_invalidation_state (scope TEXT PRIMARY KEY, generation BIGINT NOT NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP);
             INSERT INTO rbac_invalidation_state (scope,generation) VALUES ('permissions',0);
             INSERT INTO users VALUES ('{U}','{T}'), ('{V}','{T}');
             INSERT INTO roles VALUES ('{R}','{T}','editor',FALSE), ('{S}','{T}','reader',FALSE);
             INSERT INTO permissions VALUES ('{P}','{T}','posts','write'), ('{Q}','{T}','posts','read');
             INSERT INTO user_roles VALUES ('{M}','{U}','{R}'), ('{N}','{V}','{S}');
             INSERT INTO role_permissions VALUES ('{M}','{R}','{P}'), ('{N}','{S}','{Q}');"
        )).await.unwrap();
        crate::m20261127_000001_create_rbac_membership_revision::Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        crate::m20261127_000003_create_rbac_role_grant_revision::Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        crate::m20261127_000006_fence_rbac_relation_parents::Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        crate::m20261127_000207_couple_static_relation_generation::Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        let initial = crate::read_permission_invalidation_generation(&setup).await.unwrap();
        setup.commit().await.unwrap();

        let delete_user = format!("DELETE FROM users WHERE id = '{U}'");
        let delete_role = format!("DELETE FROM roles WHERE id = '{R}'");
        let delete_permission = format!("DELETE FROM permissions WHERE id = '{P}'");
        for deletion in [&delete_user, &delete_role] {
            reject_busy(&db, &schema, &format!("SELECT id FROM user_roles WHERE id = '{M}' FOR UPDATE"), deletion).await;
            reject_busy(&db, &schema, "LOCK TABLE user_roles IN SHARE MODE", deletion).await;
        }
        for deletion in [&delete_role, &delete_permission] {
            reject_busy(&db, &schema, &format!("SELECT id FROM role_permissions WHERE id = '{M}' FOR UPDATE"), deletion).await;
            reject_busy(&db, &schema, "LOCK TABLE role_permissions IN SHARE MODE", deletion).await;
        }
        reject_busy(&db, &schema, &format!("SELECT id FROM users WHERE id = '{U}' FOR UPDATE"), &delete_role).await;
        reject_busy(&db, &schema, &format!("SELECT id FROM roles WHERE id = '{R}' FOR UPDATE"), &delete_permission).await;

        // A busy late watermark rolls back parent, children and revision.
        let holder = tx(&db, &schema).await;
        holder.execute_unprepared("SELECT scope FROM rbac_invalidation_state WHERE scope = 'permissions' FOR UPDATE").await.unwrap();
        let writer = tx(&db, &schema).await;
        writer.execute_unprepared(&delete_role).await.unwrap();
        let error = tokio::time::timeout(Duration::from_secs(3), crate::flush_static_relation_invalidation_generation(&writer))
            .await.expect("deferred generation must not wait").unwrap_err();
        match error {
            crate::RbacInvalidationGenerationError::Database(error) => assert_busy(&error),
            other => panic!("unexpected generation failure: {other:?}"),
        }
        writer.rollback().await.unwrap();
        holder.rollback().await.unwrap();

        let holder = tx(&db, &schema).await;
        holder.execute_unprepared(&format!("SELECT id FROM user_roles WHERE id = '{N}' FOR UPDATE; SELECT id FROM role_permissions WHERE id = '{N}' FOR UPDATE")).await.unwrap();
        let writer = tx(&db, &schema).await;
        assert_eq!(value(&writer, "SELECT COUNT(*) AS value FROM roles").await, 2);
        assert_eq!(value(&writer, &format!("SELECT rbac_membership_revision AS value FROM users WHERE id = '{U}'")).await, 0);
        assert_eq!(crate::read_permission_invalidation_generation(&writer).await.unwrap(), initial);
        tokio::time::timeout(Duration::from_secs(3), writer.execute_unprepared(&delete_role))
            .await.expect("unrelated relations must not block role deletion").unwrap();
        assert_eq!(value(&writer, &format!("SELECT rbac_membership_revision AS value FROM users WHERE id = '{U}'")).await, 1);
        assert_eq!(crate::flush_static_relation_invalidation_generation(&writer).await.unwrap(), initial + 2);
        writer.commit().await.unwrap();
        holder.rollback().await.unwrap();

        let writer = tx(&db, &schema).await;
        writer.execute_unprepared(&format!("DELETE FROM permissions WHERE id = '{Q}'")).await.unwrap();
        assert_eq!(value(&writer, &format!("SELECT grant_revision AS value FROM roles WHERE id = '{S}'")).await, 1);
        writer.execute_unprepared(&format!("DELETE FROM users WHERE id = '{V}'")).await.unwrap();
        assert_eq!(crate::flush_static_relation_invalidation_generation(&writer).await.unwrap(), initial + 4);
        assert_eq!(value(&writer, "SELECT COUNT(*) AS value FROM user_roles").await, 0);
        assert_eq!(value(&writer, "SELECT COUNT(*) AS value FROM role_permissions").await, 0);
        writer.commit().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
