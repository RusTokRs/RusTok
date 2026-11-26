//! Durable membership history, independent of cache invalidation.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const SQLITE_TRIGGERS: &[&str] = &[
    "CREATE TRIGGER rbac_membership_revision_insert AFTER INSERT ON user_roles 
     BEGIN
       SELECT CASE WHEN EXISTS (SELECT 1 FROM users WHERE (id = NEW.user_id) AND rbac_membership_revision = 9223372036854775807)
         THEN RAISE(ABORT, 'RBAC membership revision exhausted') END;
       UPDATE users SET rbac_membership_revision = rbac_membership_revision + 1 WHERE id = NEW.user_id;
     END",
    "CREATE TRIGGER rbac_membership_revision_delete AFTER DELETE ON user_roles 
     BEGIN
       SELECT CASE WHEN EXISTS (SELECT 1 FROM users WHERE (id = OLD.user_id) AND rbac_membership_revision = 9223372036854775807)
         THEN RAISE(ABORT, 'RBAC membership revision exhausted') END;
       UPDATE users SET rbac_membership_revision = rbac_membership_revision + 1 WHERE id = OLD.user_id;
     END",
    "CREATE TRIGGER rbac_membership_revision_update AFTER UPDATE OF user_id, role_id ON user_roles WHEN NEW.user_id IS NOT OLD.user_id OR NEW.role_id IS NOT OLD.role_id
     BEGIN
       SELECT CASE WHEN EXISTS (SELECT 1 FROM users WHERE (id = OLD.user_id OR id = NEW.user_id) AND rbac_membership_revision = 9223372036854775807)
         THEN RAISE(ABORT, 'RBAC membership revision exhausted') END;
       UPDATE users SET rbac_membership_revision = rbac_membership_revision + 1 WHERE id = OLD.user_id OR id = NEW.user_id;
     END",
    "CREATE TRIGGER rbac_membership_revision_monotonic BEFORE UPDATE OF rbac_membership_revision ON users
     WHEN NEW.rbac_membership_revision < OLD.rbac_membership_revision
     BEGIN SELECT RAISE(ABORT, 'RBAC membership revision cannot decrease'); END",
    "CREATE TRIGGER rbac_membership_identity_insert BEFORE INSERT ON user_roles
     WHEN EXISTS (SELECT 1 FROM user_roles WHERE id = NEW.id AND user_id IS NOT NEW.user_id)
     BEGIN SELECT RAISE(ABORT, 'RBAC membership identity conflict'); END",
    "CREATE TRIGGER rbac_membership_identity_update BEFORE UPDATE OF id ON user_roles
     WHEN NEW.id IS NOT OLD.id
     BEGIN SELECT RAISE(ABORT, 'RBAC membership identity is immutable'); END",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                db.execute_unprepared("ALTER TABLE users ADD COLUMN rbac_membership_revision BIGINT NOT NULL DEFAULT 0 CHECK (rbac_membership_revision >= 0)").await?;
                db.execute_unprepared(r#"
CREATE FUNCTION rbac_advance_membership_revision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE users SET rbac_membership_revision = rbac_membership_revision + 1 WHERE id = NEW.user_id;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE users SET rbac_membership_revision = rbac_membership_revision + 1 WHERE id = OLD.user_id;
    ELSIF NEW.user_id IS DISTINCT FROM OLD.user_id OR NEW.role_id IS DISTINCT FROM OLD.role_id THEN
        UPDATE users SET rbac_membership_revision = rbac_membership_revision + 1
        WHERE id = OLD.user_id OR id = NEW.user_id;
    END IF;
    RETURN NULL;
END; $$;
CREATE TRIGGER rbac_membership_revision_change AFTER INSERT OR UPDATE OR DELETE ON user_roles
FOR EACH ROW EXECUTE FUNCTION rbac_advance_membership_revision();
CREATE FUNCTION rbac_guard_membership_revision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.rbac_membership_revision < OLD.rbac_membership_revision THEN
        RAISE EXCEPTION 'RBAC membership revision cannot decrease';
    END IF;
    RETURN NEW;
END; $$;
CREATE TRIGGER rbac_membership_revision_monotonic BEFORE UPDATE OF rbac_membership_revision ON users
FOR EACH ROW EXECUTE FUNCTION rbac_guard_membership_revision();
CREATE FUNCTION rbac_reject_membership_truncate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'RBAC membership history requires row deletion';
END; $$;
CREATE TRIGGER rbac_membership_no_truncate BEFORE TRUNCATE ON user_roles
FOR EACH STATEMENT EXECUTE FUNCTION rbac_reject_membership_truncate();
"#).await?;
            }
            DatabaseBackend::Sqlite => {
                db.execute_unprepared("ALTER TABLE users ADD COLUMN rbac_membership_revision BIGINT NOT NULL DEFAULT 0 CHECK (typeof(rbac_membership_revision) = 'integer' AND rbac_membership_revision >= 0)").await?;
                for sql in SQLITE_TRIGGERS { db.execute_unprepared(sql).await?; }
            }
            _ => return Err(DbErr::Custom("unsupported RBAC membership revision backend".into())),
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                db.execute_unprepared(r#"
DROP TRIGGER rbac_membership_no_truncate ON user_roles;
DROP TRIGGER rbac_membership_revision_change ON user_roles;
DROP TRIGGER rbac_membership_revision_monotonic ON users;
DROP FUNCTION rbac_reject_membership_truncate();
DROP FUNCTION rbac_advance_membership_revision();
DROP FUNCTION rbac_guard_membership_revision();
"#).await?;
            }
            DatabaseBackend::Sqlite => {
                for name in ["rbac_membership_revision_insert", "rbac_membership_revision_delete",
                    "rbac_membership_revision_update", "rbac_membership_revision_monotonic",
                    "rbac_membership_identity_insert", "rbac_membership_identity_update"] {
                    db.execute_unprepared(&format!("DROP TRIGGER {name}")).await?;
                }
            }
            _ => return Err(DbErr::Custom("unsupported RBAC membership revision backend".into())),
        }
        db.execute_unprepared("ALTER TABLE users DROP COLUMN rbac_membership_revision").await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, Statement, TransactionTrait};

    async fn revision(db: &impl ConnectionTrait, id: &str) -> i64 {
        db.query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite, "SELECT rbac_membership_revision FROM users WHERE id = ?", [id.into()],
        )).await.unwrap().unwrap().try_get("", "rbac_membership_revision").unwrap()
    }

    #[tokio::test]
    async fn membership_epoch_tracks_aba_rollback_noops_and_both_subjects() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for sql in [
            "PRAGMA foreign_keys = ON",
            "CREATE TABLE users (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)",
            "CREATE TABLE roles (id TEXT PRIMARY KEY)",
            "CREATE TABLE user_roles (id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE, role_id TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE, UNIQUE(user_id, role_id))",
            "INSERT INTO users VALUES ('u', 't'), ('v', 't')",
            "INSERT INTO roles VALUES ('r'), ('s')",
        ] { db.execute_unprepared(sql).await.unwrap(); }
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();
        assert_eq!(revision(&db, "u").await, 0);
        db.execute_unprepared("INSERT INTO user_roles VALUES ('a','u','r')").await.unwrap();
        db.execute_unprepared("INSERT INTO user_roles VALUES ('b','u','r') ON CONFLICT(user_id,role_id) DO NOTHING").await.unwrap();
        db.execute_unprepared("UPDATE user_roles SET role_id = role_id WHERE id = 'a'").await.unwrap();
        assert_eq!(revision(&db, "u").await, 1);
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("DELETE FROM user_roles WHERE id = 'a'").await.unwrap();
        assert_eq!(revision(&tx, "u").await, 2);
        tx.rollback().await.unwrap();
        assert_eq!(revision(&db, "u").await, 1);
        db.execute_unprepared("DELETE FROM user_roles WHERE id = 'a'").await.unwrap();
        db.execute_unprepared("INSERT INTO user_roles VALUES ('a','u','r')").await.unwrap();
        assert_eq!(revision(&db, "u").await, 3);
        assert!(db.execute_unprepared("INSERT OR REPLACE INTO user_roles VALUES ('a','v','r')").await.is_err());
        db.execute_unprepared("UPDATE user_roles SET user_id = 'v' WHERE id = 'a'").await.unwrap();
        assert_eq!(revision(&db, "u").await, 4);
        assert_eq!(revision(&db, "v").await, 1);
        db.execute_unprepared("DELETE FROM roles WHERE id = 'r'").await.unwrap();
        assert_eq!(revision(&db, "v").await, 2);
        assert!(db.execute_unprepared("UPDATE users SET rbac_membership_revision = 0 WHERE id = 'v'").await.is_err());
        db.execute_unprepared("UPDATE users SET rbac_membership_revision = 9223372036854775807 WHERE id = 'u'").await.unwrap();
        assert!(db.execute_unprepared("INSERT OR IGNORE INTO user_roles VALUES ('z','u','s')").await.is_err());
        Migration.down(&manager).await.unwrap();
    }
}
