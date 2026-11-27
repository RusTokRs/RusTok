//! Static role-grant history for durable delegation credentials.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const SQLITE_TRIGGERS: &[&str] = &[
    "CREATE TRIGGER rbac_grant_revision_insert AFTER INSERT ON role_permissions 
     BEGIN
       SELECT CASE WHEN EXISTS (SELECT 1 FROM roles WHERE (id = NEW.role_id) AND grant_revision = 9223372036854775807)
         THEN RAISE(ABORT, 'RBAC grant revision exhausted') END;
       UPDATE roles SET grant_revision = grant_revision + 1 WHERE id = NEW.role_id;
     END",
    "CREATE TRIGGER rbac_grant_revision_delete AFTER DELETE ON role_permissions 
     BEGIN
       SELECT CASE WHEN EXISTS (SELECT 1 FROM roles WHERE (id = OLD.role_id) AND grant_revision = 9223372036854775807)
         THEN RAISE(ABORT, 'RBAC grant revision exhausted') END;
       UPDATE roles SET grant_revision = grant_revision + 1 WHERE id = OLD.role_id;
     END",
    "CREATE TRIGGER rbac_grant_revision_update AFTER UPDATE OF role_id, permission_id ON role_permissions WHEN NEW.role_id IS NOT OLD.role_id OR NEW.permission_id IS NOT OLD.permission_id
     BEGIN
       SELECT CASE WHEN EXISTS (SELECT 1 FROM roles WHERE (id = OLD.role_id OR id = NEW.role_id) AND grant_revision = 9223372036854775807)
         THEN RAISE(ABORT, 'RBAC grant revision exhausted') END;
       UPDATE roles SET grant_revision = grant_revision + 1 WHERE id = OLD.role_id OR id = NEW.role_id;
     END",
    "CREATE TRIGGER rbac_grant_revision_monotonic BEFORE UPDATE OF grant_revision ON roles
     WHEN NEW.grant_revision < OLD.grant_revision
     BEGIN SELECT RAISE(ABORT, 'RBAC grant revision cannot decrease'); END",
    "CREATE TRIGGER rbac_role_identity_change AFTER UPDATE OF tenant_id, slug, is_system ON roles
     WHEN NEW.tenant_id IS NOT OLD.tenant_id OR NEW.slug IS NOT OLD.slug OR NEW.is_system IS NOT OLD.is_system
     BEGIN
       SELECT CASE WHEN NEW.grant_revision = 9223372036854775807 THEN RAISE(ABORT, 'RBAC grant revision exhausted') END;
       UPDATE roles SET grant_revision = NEW.grant_revision + 1 WHERE id = NEW.id;
     END",
    "CREATE TRIGGER rbac_role_id_immutable BEFORE UPDATE OF id ON roles
     WHEN NEW.id IS NOT OLD.id BEGIN SELECT RAISE(ABORT, 'RBAC role ID is immutable'); END",
    "CREATE TRIGGER rbac_permission_identity_immutable BEFORE UPDATE OF id, tenant_id, resource, action ON permissions
     WHEN NEW.id IS NOT OLD.id OR NEW.tenant_id IS NOT OLD.tenant_id OR NEW.resource IS NOT OLD.resource OR NEW.action IS NOT OLD.action
     BEGIN SELECT RAISE(ABORT, 'RBAC permission identity is immutable'); END",
    "CREATE TRIGGER rbac_permission_replace_guard BEFORE INSERT ON permissions
     WHEN EXISTS (SELECT 1 FROM permissions WHERE id = NEW.id AND (tenant_id IS NOT NEW.tenant_id OR resource IS NOT NEW.resource OR action IS NOT NEW.action))
     BEGIN SELECT RAISE(ABORT, 'RBAC permission identity conflict'); END",
    "CREATE TRIGGER rbac_grant_identity_insert BEFORE INSERT ON role_permissions
     WHEN EXISTS (SELECT 1 FROM role_permissions WHERE id = NEW.id AND role_id IS NOT NEW.role_id)
     BEGIN SELECT RAISE(ABORT, 'RBAC grant identity conflict'); END",
    "CREATE TRIGGER rbac_grant_identity_update BEFORE UPDATE OF id ON role_permissions
     WHEN NEW.id IS NOT OLD.id BEGIN SELECT RAISE(ABORT, 'RBAC grant ID is immutable'); END",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                db.execute_unprepared("ALTER TABLE roles ADD COLUMN grant_revision BIGINT NOT NULL DEFAULT 0 CHECK (grant_revision >= 0)").await?;
                db.execute_unprepared(r#"
CREATE FUNCTION rbac_advance_grant_revision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE roles SET grant_revision = grant_revision + 1 WHERE id = NEW.role_id;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE roles SET grant_revision = grant_revision + 1 WHERE id = OLD.role_id;
    ELSIF NEW.role_id IS DISTINCT FROM OLD.role_id OR NEW.permission_id IS DISTINCT FROM OLD.permission_id THEN
        UPDATE roles SET grant_revision = grant_revision + 1 WHERE id = OLD.role_id OR id = NEW.role_id;
    END IF;
    RETURN NULL;
END; $$;
CREATE TRIGGER rbac_grant_revision_change AFTER INSERT OR UPDATE OR DELETE ON role_permissions
FOR EACH ROW EXECUTE FUNCTION rbac_advance_grant_revision();
CREATE FUNCTION rbac_guard_role_revision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.id IS DISTINCT FROM OLD.id OR NEW.grant_revision < OLD.grant_revision THEN
        RAISE EXCEPTION 'RBAC role identity or revision regression';
    END IF;
    IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id OR NEW.slug IS DISTINCT FROM OLD.slug OR NEW.is_system IS DISTINCT FROM OLD.is_system THEN
        NEW.grant_revision := NEW.grant_revision + 1;
    END IF;
    RETURN NEW;
END; $$;
CREATE TRIGGER rbac_role_revision_guard BEFORE UPDATE ON roles
FOR EACH ROW EXECUTE FUNCTION rbac_guard_role_revision();
CREATE FUNCTION rbac_guard_permission_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.id IS DISTINCT FROM OLD.id OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id OR NEW.resource IS DISTINCT FROM OLD.resource OR NEW.action IS DISTINCT FROM OLD.action THEN
        RAISE EXCEPTION 'RBAC permission identity is immutable';
    END IF;
    RETURN NEW;
END; $$;
CREATE TRIGGER rbac_permission_identity_guard BEFORE UPDATE ON permissions
FOR EACH ROW EXECUTE FUNCTION rbac_guard_permission_identity();
CREATE FUNCTION rbac_reject_grant_truncate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'RBAC role history requires row deletion';
END; $$;
CREATE TRIGGER rbac_grant_no_truncate BEFORE TRUNCATE ON role_permissions
FOR EACH STATEMENT EXECUTE FUNCTION rbac_reject_grant_truncate();
"#).await?;
            }
            DatabaseBackend::Sqlite => {
                db.execute_unprepared("ALTER TABLE roles ADD COLUMN grant_revision BIGINT NOT NULL DEFAULT 0 CHECK (typeof(grant_revision) = 'integer' AND grant_revision >= 0)").await?;
                for sql in SQLITE_TRIGGERS {
                    db.execute_unprepared(sql).await?;
                }
            }
            _ => {
                return Err(DbErr::Custom(
                    "unsupported RBAC grant revision backend".into(),
                ));
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                db.execute_unprepared(
                    r#"
DROP TRIGGER rbac_grant_no_truncate ON role_permissions;
DROP TRIGGER rbac_grant_revision_change ON role_permissions;
DROP TRIGGER rbac_role_revision_guard ON roles;
DROP TRIGGER rbac_permission_identity_guard ON permissions;
DROP FUNCTION rbac_reject_grant_truncate();
DROP FUNCTION rbac_advance_grant_revision();
DROP FUNCTION rbac_guard_role_revision();
DROP FUNCTION rbac_guard_permission_identity();
"#,
                )
                .await?;
            }
            DatabaseBackend::Sqlite => {
                for name in [
                    "rbac_grant_revision_insert",
                    "rbac_grant_revision_delete",
                    "rbac_grant_revision_update",
                    "rbac_grant_revision_monotonic",
                    "rbac_role_identity_change",
                    "rbac_role_id_immutable",
                    "rbac_permission_identity_immutable",
                    "rbac_permission_replace_guard",
                    "rbac_grant_identity_insert",
                    "rbac_grant_identity_update",
                ] {
                    db.execute_unprepared(&format!("DROP TRIGGER {name}"))
                        .await?;
                }
            }
            _ => {
                return Err(DbErr::Custom(
                    "unsupported RBAC grant revision backend".into(),
                ));
            }
        }
        db.execute_unprepared("ALTER TABLE roles DROP COLUMN grant_revision")
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, Statement, TransactionTrait};

    async fn revision(db: &impl ConnectionTrait, id: &str) -> i64 {
        db.query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT grant_revision FROM roles WHERE id = ?",
            [id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "grant_revision")
        .unwrap()
    }

    #[tokio::test]
    async fn grant_epoch_tracks_aba_rollback_and_role_identity() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for sql in [
            "PRAGMA foreign_keys = ON",
            "CREATE TABLE roles (id TEXT PRIMARY KEY, tenant_id TEXT, slug TEXT, is_system BOOLEAN, name TEXT)",
            "CREATE TABLE permissions (id TEXT PRIMARY KEY, tenant_id TEXT, resource TEXT, action TEXT)",
            "CREATE TABLE role_permissions (id TEXT PRIMARY KEY, role_id TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE, permission_id TEXT NOT NULL REFERENCES permissions(id) ON DELETE CASCADE, UNIQUE(role_id, permission_id))",
            "INSERT INTO roles VALUES ('r','t','admin',TRUE,'Admin'), ('s','t','customer',TRUE,'Customer')",
            "INSERT INTO permissions VALUES ('p','t','users','manage')",
        ] {
            db.execute_unprepared(sql).await.unwrap();
        }
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();
        db.execute_unprepared("INSERT INTO role_permissions VALUES ('a','r','p')")
            .await
            .unwrap();
        db.execute_unprepared("INSERT INTO role_permissions VALUES ('b','r','p') ON CONFLICT(role_id,permission_id) DO NOTHING").await.unwrap();
        db.execute_unprepared(
            "UPDATE role_permissions SET permission_id = permission_id WHERE id = 'a'",
        )
        .await
        .unwrap();
        db.execute_unprepared("UPDATE roles SET name = 'New name' WHERE id = 'r'")
            .await
            .unwrap();
        assert_eq!(revision(&db, "r").await, 1);
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("DELETE FROM role_permissions WHERE id = 'a'")
            .await
            .unwrap();
        assert_eq!(revision(&tx, "r").await, 2);
        tx.rollback().await.unwrap();
        assert_eq!(revision(&db, "r").await, 1);
        db.execute_unprepared("DELETE FROM role_permissions WHERE id = 'a'")
            .await
            .unwrap();
        db.execute_unprepared("INSERT INTO role_permissions VALUES ('a','r','p')")
            .await
            .unwrap();
        assert_eq!(revision(&db, "r").await, 3);
        assert_eq!(revision(&db, "s").await, 0);
        db.execute_unprepared("UPDATE role_permissions SET role_id = 's' WHERE id = 'a'")
            .await
            .unwrap();
        assert_eq!(revision(&db, "r").await, 4);
        assert_eq!(revision(&db, "s").await, 1);
        assert!(
            db.execute_unprepared("UPDATE permissions SET action = 'create' WHERE id = 'p'")
                .await
                .is_err()
        );
        db.execute_unprepared("DELETE FROM permissions WHERE id = 'p'")
            .await
            .unwrap();
        assert_eq!(revision(&db, "s").await, 2);
        db.execute_unprepared("UPDATE roles SET slug = 'other' WHERE id = 's'")
            .await
            .unwrap();
        db.execute_unprepared("UPDATE roles SET slug = 'customer' WHERE id = 's'")
            .await
            .unwrap();
        assert_eq!(revision(&db, "s").await, 4);
        Migration.down(&manager).await.unwrap();
    }
}
