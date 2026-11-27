//! Reject late PostgreSQL artifact-catalog parent lock waits.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
CREATE OR REPLACE FUNCTION rbac_fence_artifact_authority_parents() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    acting_user uuid;
BEGIN
    LOCK TABLE users, roles, rbac_artifact_permission_definitions IN ROW SHARE MODE NOWAIT;
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

CREATE FUNCTION rbac_fence_artifact_catalog_parent() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_TABLE_NAME = 'rbac_artifact_permission_definitions' THEN
        -- The definition is immutable; its exact installation tuple is the FK.
        LOCK TABLE rbac_artifact_permission_installations IN ROW SHARE MODE NOWAIT;
        PERFORM 1 FROM rbac_artifact_permission_installations
            WHERE installation_id = NEW.installation_id AND scope_key = NEW.scope_key
              AND module_slug = NEW.module_slug AND release_digest = NEW.release_digest
            FOR KEY SHARE NOWAIT;
    ELSIF TG_TABLE_NAME = 'rbac_artifact_permission_translations' THEN
        IF TG_OP = 'UPDATE' THEN
            IF NEW.artifact_permission_id IS NOT DISTINCT FROM OLD.artifact_permission_id THEN
                RETURN NEW;
            END IF;
        END IF;
        LOCK TABLE rbac_artifact_permission_definitions IN ROW SHARE MODE NOWAIT;
        PERFORM 1 FROM rbac_artifact_permission_definitions
            WHERE id = NEW.artifact_permission_id FOR KEY SHARE NOWAIT;
    ELSE
        IF TG_OP = 'UPDATE' THEN
            IF NEW.release_permission_id IS NOT DISTINCT FROM OLD.release_permission_id THEN
                RETURN NEW;
            END IF;
        END IF;
        LOCK TABLE rbac_artifact_release_permission_definitions IN ROW SHARE MODE NOWAIT;
        PERFORM 1 FROM rbac_artifact_release_permission_definitions
            WHERE id = NEW.release_permission_id FOR KEY SHARE NOWAIT;
    END IF;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'RBAC artifact catalog parent missing or invisible' USING ERRCODE = '23503';
    END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'RBAC artifact catalog parent fence busy' USING ERRCODE = '55P03';
END; $$;

CREATE TRIGGER rbac_artifact_catalog_parent_fence
BEFORE INSERT ON rbac_artifact_permission_definitions
FOR EACH ROW EXECUTE FUNCTION rbac_fence_artifact_catalog_parent();
CREATE TRIGGER rbac_artifact_catalog_parent_fence
BEFORE INSERT OR UPDATE OF artifact_permission_id ON rbac_artifact_permission_translations
FOR EACH ROW EXECUTE FUNCTION rbac_fence_artifact_catalog_parent();
CREATE TRIGGER rbac_artifact_catalog_parent_fence
BEFORE INSERT OR UPDATE OF release_permission_id ON rbac_artifact_release_permission_translations
FOR EACH ROW EXECUTE FUNCTION rbac_fence_artifact_catalog_parent();
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager.get_connection().execute_unprepared(POSTGRES).await?;
            }
            // SQLite serializes writers; existing FK checks remain authoritative.
            DatabaseBackend::Sqlite => {}
            _ => return Err(DbErr::Migration("unsupported RBAC artifact catalog backend".into())),
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => Err(DbErr::Migration(
                "RBAC artifact catalog parent fences cannot be downgraded safely".into(),
            )),
            DatabaseBackend::Sqlite => Ok(()),
            _ => Err(DbErr::Migration("unsupported RBAC artifact catalog backend".into())),
        }
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
    async fn postgres_artifact_catalog_parent_fences_reject_late_row_and_relation_locks() {
        let db = Database::connect(std::env::var("RUSTOK_RBAC_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("rbac_catalog_fence_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = tx(&db, &schema).await;
        let id = Uuid::new_v4();
        let other = Uuid::new_v4();
        setup.execute_unprepared(&format!(
            "CREATE TABLE rbac_artifact_permission_installations (
                installation_id UUID PRIMARY KEY, scope_key TEXT NOT NULL, module_slug TEXT NOT NULL,
                release_digest TEXT NOT NULL, UNIQUE(installation_id,scope_key,module_slug,release_digest));
             CREATE TABLE rbac_artifact_permission_definitions (
                id UUID PRIMARY KEY, installation_id UUID NOT NULL, scope_key TEXT NOT NULL,
                module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, UNIQUE(id,scope_key),
                FOREIGN KEY(installation_id,scope_key,module_slug,release_digest)
                  REFERENCES rbac_artifact_permission_installations(installation_id,scope_key,module_slug,release_digest));
             CREATE TABLE rbac_artifact_release_permission_definitions (id UUID PRIMARY KEY);
             CREATE TABLE rbac_artifact_permission_translations (
                id UUID PRIMARY KEY, artifact_permission_id UUID NOT NULL REFERENCES rbac_artifact_permission_definitions(id),
                label TEXT NOT NULL);
             CREATE TABLE rbac_artifact_release_permission_translations (
                id UUID PRIMARY KEY, release_permission_id UUID NOT NULL REFERENCES rbac_artifact_release_permission_definitions(id),
                label TEXT NOT NULL);
             INSERT INTO rbac_artifact_permission_installations VALUES ('{id}','platform','sample','digest');
             INSERT INTO rbac_artifact_permission_definitions VALUES ('{id}','{id}','platform','sample','digest');
             INSERT INTO rbac_artifact_release_permission_definitions VALUES ('{id}');
             INSERT INTO rbac_artifact_permission_translations VALUES ('{id}','{id}','old');
             INSERT INTO rbac_artifact_release_permission_translations VALUES ('{id}','{id}','old');"
        )).await.unwrap();
        // Install the actual predecessor triggers before replacing their function.
        // The assignment and operation tables keep the three exact FK tuples.
        setup.execute_unprepared(&format!(
            "CREATE TABLE users (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, UNIQUE(tenant_id,id));
             CREATE TABLE roles (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, UNIQUE(tenant_id,id));
             CREATE TABLE permissions (id UUID PRIMARY KEY);
             CREATE TABLE user_roles (id UUID PRIMARY KEY, user_id UUID, role_id UUID);
             CREATE TABLE role_permissions (id UUID PRIMARY KEY, role_id UUID, permission_id UUID);
             CREATE TABLE rbac_artifact_role_permissions (
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, role_id UUID NOT NULL,
                artifact_permission_id UUID NOT NULL, permission_scope_key TEXT NOT NULL,
                granted_by_actor_id UUID NOT NULL,
                FOREIGN KEY(tenant_id,role_id) REFERENCES roles(tenant_id,id),
                FOREIGN KEY(tenant_id,granted_by_actor_id) REFERENCES users(tenant_id,id),
                FOREIGN KEY(artifact_permission_id,permission_scope_key)
                    REFERENCES rbac_artifact_permission_definitions(id,scope_key));
             CREATE TABLE rbac_artifact_role_permission_operations (
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, role_id UUID NOT NULL,
                artifact_permission_id UUID NOT NULL, permission_scope_key TEXT NOT NULL,
                actor_id UUID NOT NULL,
                FOREIGN KEY(tenant_id,role_id) REFERENCES roles(tenant_id,id),
                FOREIGN KEY(tenant_id,actor_id) REFERENCES users(tenant_id,id),
                FOREIGN KEY(artifact_permission_id,permission_scope_key)
                    REFERENCES rbac_artifact_permission_definitions(id,scope_key));
             INSERT INTO users VALUES ('{id}','{other}');
             INSERT INTO roles VALUES ('{id}','{other}');"
        )).await.unwrap();
        let manager = SchemaManager::new(&setup);
        crate::m20261127_000006_fence_rbac_relation_parents::Migration.up(&manager).await.unwrap();
        Migration.up(&manager).await.unwrap();
        setup.commit().await.unwrap();

        let cases = [
            ("rbac_artifact_permission_installations", format!("INSERT INTO rbac_artifact_permission_definitions VALUES ('{other}','{id}','platform','sample','digest')")),
            ("rbac_artifact_permission_definitions", format!("INSERT INTO rbac_artifact_permission_translations VALUES ('{other}','{id}','new')")),
            ("rbac_artifact_release_permission_definitions", format!("INSERT INTO rbac_artifact_release_permission_translations VALUES ('{other}','{id}','new')")),
        ];
        for (parent, insert) in cases {
            for lock in [format!("SELECT 1 FROM {parent} FOR UPDATE"), format!("LOCK TABLE {parent} IN ACCESS EXCLUSIVE MODE")] {
                let holder = tx(&db, &schema).await;
                holder.execute_unprepared(&lock).await.unwrap();
                let writer = tx(&db, &schema).await;
                let error = tokio::time::timeout(Duration::from_secs(5), writer.execute_unprepared(&insert))
                    .await.expect("late catalog parent must not wait").unwrap_err();
                assert!(error.to_string().contains("RBAC artifact catalog parent fence busy"), "{error}");
                writer.rollback().await.unwrap();
                holder.rollback().await.unwrap();
            }
            let retry = tx(&db, &schema).await;
            retry.execute_unprepared(&insert).await.unwrap();
            retry.rollback().await.unwrap();
        }

        // Exercise both existing triggers, not just the new catalog trigger.
        for table in ["rbac_artifact_role_permissions", "rbac_artifact_role_permission_operations"] {
            let insert = format!(
                "INSERT INTO {table} VALUES ('{id}','{other}','{id}','{id}','platform','{id}')"
            );
            for parent in ["users", "roles", "rbac_artifact_permission_definitions"] {
                for lock in [
                    format!("SELECT 1 FROM {parent} FOR UPDATE"),
                    format!("LOCK TABLE {parent} IN ACCESS EXCLUSIVE MODE"),
                ] {
                    let holder = tx(&db, &schema).await;
                    holder.execute_unprepared(&lock).await.unwrap();
                    let writer = tx(&db, &schema).await;
                    let error = tokio::time::timeout(Duration::from_secs(5), writer.execute_unprepared(&insert))
                        .await.expect("late assignment parent must not wait").unwrap_err();
                    assert!(error.to_string().contains("RBAC artifact authority parent fence busy"), "{error}");
                    writer.rollback().await.unwrap();
                    holder.rollback().await.unwrap();
                }
            }

            // An exact earlier parent held by the owner transaction is reentrant.
            let retry = tx(&db, &schema).await;
            retry.execute_unprepared("SELECT 1 FROM users FOR UPDATE; SELECT 1 FROM roles FOR UPDATE; SELECT 1 FROM rbac_artifact_permission_definitions FOR UPDATE")
                .await.unwrap();
            retry.execute_unprepared(&insert).await.unwrap();
            retry.rollback().await.unwrap();

            // A different tenant cannot reuse the actor/role FK identity.
            let writer = tx(&db, &schema).await;
            let wrong = format!("INSERT INTO {table} VALUES ('{id}','{id}','{id}','{id}','platform','{id}')");
            let error = writer.execute_unprepared(&wrong).await.unwrap_err();
            assert!(error.to_string().contains("RBAC artifact actor parent missing"), "{error}");
            writer.rollback().await.unwrap();
        }

        let holder = tx(&db, &schema).await;
        holder.execute_unprepared("SELECT 1 FROM rbac_artifact_permission_definitions FOR UPDATE").await.unwrap();
        let writer = tx(&db, &schema).await;
        writer.execute_unprepared("UPDATE rbac_artifact_permission_translations SET label = 'localized'").await.unwrap();
        writer.commit().await.unwrap();
        holder.rollback().await.unwrap();

        let writer = tx(&db, &schema).await;
        let error = writer.execute_unprepared(&format!(
            "INSERT INTO rbac_artifact_permission_definitions VALUES ('{other}','{id}','platform','wrong','digest')"
        )).await.unwrap_err();
        assert!(error.to_string().contains("RBAC artifact catalog parent missing or invisible"), "{error}");
        writer.rollback().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
