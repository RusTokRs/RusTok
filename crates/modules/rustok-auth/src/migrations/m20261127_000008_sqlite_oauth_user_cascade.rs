//! SQLite's token tenant guard substitutes for PostgreSQL's composite user
//! FK on writes, but did not implement its ON DELETE CASCADE.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

const SQLITE_CASCADE: &str = r#"
CREATE TRIGGER trg_auth_users_oauth_tokens_delete
AFTER DELETE ON users FOR EACH ROW
BEGIN
    -- The family root guard requires descendants to disappear first.
    -- Preserve service credentials (NULL subject) and every other user.
    DELETE FROM oauth_tokens
    WHERE tenant_id = OLD.tenant_id AND user_id = OLD.id AND id <> family_id;
    DELETE FROM oauth_tokens
    WHERE tenant_id = OLD.tenant_id AND user_id = OLD.id AND id = family_id;
END
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                // Composite FK ON DELETE CASCADE already owns this invariant.
            }
            DatabaseBackend::Sqlite => {
                // Reserve the writer before validating retained identities.
                db.execute_unprepared("UPDATE oauth_tokens SET revoked_at = revoked_at WHERE 0").await?;
                if db.query_one_raw(Statement::from_string(
                    DatabaseBackend::Sqlite,
                    "SELECT 1 FROM oauth_tokens token WHERE token.user_id IS NOT NULL
                     AND NOT EXISTS (SELECT 1 FROM users subject
                         WHERE subject.id = token.user_id AND subject.tenant_id = token.tenant_id)
                     LIMIT 1",
                )).await?.is_some() {
                    return Err(DbErr::Migration(
                        "SQLite OAuth user cascade migration found orphan or cross-tenant token lineage; reconcile before retry".into(),
                    ));
                }
                db.execute_unprepared(SQLITE_CASCADE).await?;
            }
            _ => return Err(DbErr::Custom("Unsupported OAuth user cascade database".into())),
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => Ok(()),
            DatabaseBackend::Sqlite => Err(DbErr::Migration(
                "SQLite OAuth user cascade cannot be downgraded safely".into(),
            )),
            _ => Err(DbErr::Custom("Unsupported OAuth user cascade database".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection, TransactionTrait};

    async fn fixture() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE tenants (id TEXT PRIMARY KEY);
             CREATE TABLE users (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE);
             CREATE TABLE oauth_apps (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE);
             CREATE TABLE oauth_tokens (
                 id TEXT PRIMARY KEY, family_id TEXT, tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                 app_id TEXT NOT NULL REFERENCES oauth_apps(id) ON DELETE CASCADE,
                 user_id TEXT, authorization_code_id TEXT, consent_granted_at TEXT,
                 grant_type TEXT NOT NULL DEFAULT 'authorization_code',
                 token_kind TEXT NOT NULL DEFAULT 'refresh', created_at TEXT NOT NULL DEFAULT '2026-01-01',
                 revoked_at TEXT);
             INSERT INTO tenants VALUES ('tenant'), ('other-tenant');
             INSERT INTO users VALUES ('user', 'tenant'), ('other-user', 'tenant');
             INSERT INTO oauth_apps VALUES ('app', 'tenant'), ('other-app', 'other-tenant');
             INSERT INTO oauth_tokens (id, family_id, tenant_id, app_id, user_id, revoked_at) VALUES
                ('root', 'root', 'tenant', 'app', 'user', '2026-01-02'),
                ('child', 'root', 'tenant', 'app', 'user', NULL),
                ('other-root', 'other-root', 'tenant', 'app', 'other-user', NULL);
             INSERT INTO oauth_tokens (id, family_id, tenant_id, app_id, user_id, grant_type, token_kind)
                VALUES ('service', 'service', 'tenant', 'app', NULL, 'client_credentials', 'access');"
        ).await.unwrap();
        super::super::m20261126_000060_enforce_oauth_family_identity::Migration
            .up(&SchemaManager::new(&db)).await.unwrap();
        db
    }

    async fn count<C: ConnectionTrait>(db: &C, predicate: &str) -> i64 {
        db.query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite, format!("SELECT COUNT(*) AS count FROM oauth_tokens WHERE {predicate}")
        )).await.unwrap().unwrap().try_get("", "count").unwrap()
    }

    #[tokio::test]
    async fn sqlite_user_delete_removes_exact_oauth_lineage_and_rolls_back_atomically() {
        let db = fixture().await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("DELETE FROM users WHERE id = 'user'").await.unwrap();
        assert_eq!(count(&tx, "user_id = 'user'").await, 0);
        assert_eq!(count(&tx, "id IN ('other-root', 'service')").await, 2);
        tx.rollback().await.unwrap();
        assert_eq!(count(&db, "user_id = 'user'").await, 2);

        db.execute_unprepared("DELETE FROM users WHERE id = 'user'").await.unwrap();
        assert_eq!(count(&db, "user_id = 'user'").await, 0);
        assert_eq!(count(&db, "1 = 1").await, 2);
        db.execute_unprepared("INSERT INTO users VALUES ('user', 'tenant')").await.unwrap();
        assert_eq!(count(&db, "user_id = 'user'").await, 0);
        db.execute_unprepared("DELETE FROM tenants WHERE id = 'tenant'").await.unwrap();
        assert_eq!(count(&db, "1 = 1").await, 0);
    }

    #[tokio::test]
    async fn sqlite_user_cascade_migration_rejects_existing_orphan_lineage() {
        let db = fixture().await;
        db.execute_unprepared("DELETE FROM users WHERE id = 'user'").await.unwrap();
        assert_eq!(count(&db, "user_id = 'user'").await, 2);
        let error = Migration.up(&SchemaManager::new(&db)).await.unwrap_err();
        assert!(error.to_string().contains("orphan or cross-tenant token lineage"), "{error}");
        assert_eq!(count(&db, "user_id = 'user'").await, 2);
    }
}
