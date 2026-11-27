//! Session tenant identity and append-only replay evidence.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

const INVALID_SESSIONS: &str = "SELECT 1 FROM sessions s WHERE NOT EXISTS (
    SELECT 1 FROM users u WHERE u.id = s.user_id AND u.tenant_id = s.tenant_id
) LIMIT 1";

const POSTGRES_GUARDS: &str = r#"
-- Reuse the existing Auth composite identity index when installed. The
-- platform chain also works independently of the Auth migration inventory.
CREATE UNIQUE INDEX IF NOT EXISTS uq_auth_users_tenant_id_id ON users (tenant_id, id);
ALTER TABLE sessions ADD CONSTRAINT fk_sessions_tenant_user
    FOREIGN KEY (tenant_id, user_id) REFERENCES users (tenant_id, id) ON DELETE CASCADE;

CREATE FUNCTION rustok_guard_session_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF ROW(NEW.id, NEW.tenant_id, NEW.user_id, NEW.created_at) IS DISTINCT FROM
       ROW(OLD.id, OLD.tenant_id, OLD.user_id, OLD.created_at) THEN
        RAISE EXCEPTION 'Session identity is immutable' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END; $$;
CREATE TRIGGER trg_session_identity_guard BEFORE UPDATE ON sessions
FOR EACH ROW EXECUTE FUNCTION rustok_guard_session_identity();

CREATE FUNCTION rustok_guard_session_history_evidence() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        RAISE EXCEPTION 'Session refresh evidence is immutable' USING ERRCODE = '23514';
    END IF;
    IF TG_OP = 'TRUNCATE' THEN
        RAISE EXCEPTION 'Session refresh evidence cannot be truncated' USING ERRCODE = '23514';
    END IF;
    -- The preceding parent fence (migration 000010) retains a present
    -- session without waiting. A legitimate FK cascade observes its own
    -- parent deletion; an independent DELETE must not remove evidence.
    IF EXISTS (SELECT 1 FROM sessions WHERE id = OLD.session_id) THEN
        RAISE EXCEPTION 'Session refresh evidence requires parent deletion' USING ERRCODE = '23514';
    END IF;
    RETURN OLD;
END; $$;
CREATE TRIGGER trg_session_history_z_evidence_guard BEFORE UPDATE OR DELETE ON session_refresh_history
FOR EACH ROW EXECUTE FUNCTION rustok_guard_session_history_evidence();
CREATE TRIGGER trg_session_history_truncate_guard BEFORE TRUNCATE ON session_refresh_history
FOR EACH STATEMENT EXECUTE FUNCTION rustok_guard_session_history_evidence();
"#;

// SQLite has no ALTER TABLE ADD FOREIGN KEY. The serialized writer and
// these guards enforce the equivalent scoped reference without rebuilding
// sessions or losing the existing history cascade. Reject REPLACE of an
// existing digest explicitly: SQLite can omit implicit DELETE triggers.
const SQLITE_GUARDS: &[&str] = &[
    r#"CREATE TRIGGER trg_session_identity_guard BEFORE UPDATE ON sessions
       FOR EACH ROW WHEN NEW.id IS NOT OLD.id OR NEW.tenant_id IS NOT OLD.tenant_id
           OR NEW.user_id IS NOT OLD.user_id OR NEW.created_at IS NOT OLD.created_at
           OR NEW.rowid IS NOT OLD.rowid
       BEGIN SELECT RAISE(ABORT, 'Session identity is immutable'); END"#,
    r#"CREATE TRIGGER trg_session_tenant_user_insert BEFORE INSERT ON sessions
       FOR EACH ROW WHEN NOT EXISTS (
           SELECT 1 FROM users u WHERE u.id = NEW.user_id AND u.tenant_id = NEW.tenant_id
       ) BEGIN SELECT RAISE(ABORT, 'Session tenant/user mismatch'); END"#,
    r#"CREATE TRIGGER trg_session_user_tenant_move BEFORE UPDATE OF tenant_id ON users
       FOR EACH ROW WHEN NEW.tenant_id IS NOT OLD.tenant_id AND EXISTS (
           SELECT 1 FROM sessions s WHERE s.user_id = OLD.id
       ) BEGIN SELECT RAISE(ABORT, 'User tenant move would invalidate sessions'); END"#,
    r#"CREATE TRIGGER trg_session_insert_replace_guard BEFORE INSERT ON sessions
       FOR EACH ROW WHEN EXISTS (
           SELECT 1 FROM sessions s WHERE s.id = NEW.id OR
               (s.tenant_id = NEW.tenant_id AND s.token_hash = NEW.token_hash)
               OR s.rowid = NEW.rowid
       ) BEGIN SELECT RAISE(ABORT, 'Session replacement is forbidden'); END"#,
    r#"CREATE TRIGGER trg_session_update_replace_guard BEFORE UPDATE OF token_hash ON sessions
       FOR EACH ROW WHEN EXISTS (
           SELECT 1 FROM sessions s WHERE s.id <> OLD.id AND
               s.tenant_id = NEW.tenant_id AND s.token_hash = NEW.token_hash
       ) BEGIN SELECT RAISE(ABORT, 'Session replacement is forbidden'); END"#,
    r#"CREATE TRIGGER trg_session_history_immutable_update BEFORE UPDATE ON session_refresh_history
       FOR EACH ROW BEGIN SELECT RAISE(ABORT, 'Session refresh evidence is immutable'); END"#,
    r#"CREATE TRIGGER trg_session_history_parent_delete BEFORE DELETE ON session_refresh_history
       FOR EACH ROW WHEN EXISTS (SELECT 1 FROM sessions WHERE id = OLD.session_id)
       BEGIN SELECT RAISE(ABORT, 'Session refresh evidence requires parent deletion'); END"#,
    r#"CREATE TRIGGER trg_session_history_insert_replace_guard BEFORE INSERT ON session_refresh_history
       FOR EACH ROW WHEN EXISTS (SELECT 1 FROM session_refresh_history
           WHERE token_hash = NEW.token_hash OR rowid = NEW.rowid)
       BEGIN SELECT RAISE(ABORT, 'Session refresh evidence replacement is forbidden'); END"#,
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();
        match backend {
            DatabaseBackend::Postgres => {
                // Freeze parent identity and both evidence tables during
                // validation and installation, in parent-first order.
                db.execute_unprepared("LOCK TABLE users, sessions, session_refresh_history IN ACCESS EXCLUSIVE MODE").await?;
            }
            DatabaseBackend::Sqlite => {
                db.execute_unprepared("UPDATE sessions SET token_hash = token_hash WHERE 0").await?;
            }
            _ => return Err(DbErr::Custom("Unsupported session evidence database".into())),
        }
        if db.query_one_raw(Statement::from_string(backend, INVALID_SESSIONS)).await?.is_some() {
            return Err(DbErr::Migration("Session evidence migration found missing or cross-tenant user; reconcile before retry".into()));
        }
        match backend {
            DatabaseBackend::Postgres => { db.execute_unprepared(POSTGRES_GUARDS).await?; }
            DatabaseBackend::Sqlite => {
                for sql in SQLITE_GUARDS { db.execute_unprepared(sql).await?; }
            }
            _ => unreachable!("backend checked"),
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("Session evidence integrity cannot be downgraded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection, TransactionTrait};

    const T: &str = "10000000-0000-0000-0000-000000000001";
    const OTHER: &str = "10000000-0000-0000-0000-000000000002";
    const U: &str = "20000000-0000-0000-0000-000000000001";
    const S: &str = "30000000-0000-0000-0000-000000000001";
    const S2: &str = "30000000-0000-0000-0000-000000000002";

    fn fixture_sql() -> String {
        format!(
            "CREATE TABLE tenants (id UUID PRIMARY KEY);
             CREATE TABLE users (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE);
             CREATE TABLE sessions (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                 user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE, token_hash TEXT NOT NULL,
                 created_at TIMESTAMP NOT NULL, UNIQUE (tenant_id, token_hash));
             CREATE TABLE session_refresh_history (token_hash TEXT PRIMARY KEY,
                 session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE, rotated_at TIMESTAMP);
             INSERT INTO tenants VALUES ('{T}'), ('{OTHER}');
             INSERT INTO users VALUES ('{U}', '{T}');
             INSERT INTO sessions VALUES ('{S}', '{T}', '{U}', 'current', '2026-01-01');
             INSERT INTO session_refresh_history VALUES ('historical', '{S}', '2026-01-02');"
        )
    }

    fn invalid_writes() -> Vec<String> {
        vec![
            format!("INSERT INTO sessions VALUES ('{S2}', '{OTHER}', '{U}', 'foreign', '2026-01-01')"),
            format!("UPDATE sessions SET tenant_id = '{OTHER}' WHERE id = '{S}'"),
            format!("UPDATE sessions SET id = '{S2}' WHERE id = '{S}'"),
            format!("UPDATE sessions SET created_at = '2026-01-02' WHERE id = '{S}'"),
            format!("UPDATE users SET tenant_id = '{OTHER}' WHERE id = '{U}'"),
            "UPDATE session_refresh_history SET rotated_at = '2026-01-03'".into(),
            "UPDATE session_refresh_history SET token_hash = 'changed'".into(),
            "DELETE FROM session_refresh_history".into(),
        ]
    }

    async fn count<C: ConnectionTrait>(db: &C, table: &str) -> i64 {
        db.query_one_raw(Statement::from_string(db.get_database_backend(), format!("SELECT COUNT(*) AS count FROM {table}")))
            .await.unwrap().unwrap().try_get("", "count").unwrap()
    }

    #[tokio::test]
    async fn sqlite_session_evidence_is_immutable_and_cascades_only_with_parent() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA foreign_keys = ON").await.unwrap();
        db.execute_unprepared("PRAGMA recursive_triggers = OFF").await.unwrap();
        db.execute_unprepared(&fixture_sql()).await.unwrap();
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        db.execute_unprepared(&format!("INSERT INTO sessions VALUES ('{S2}', '{T}', '{U}', 'other-current', '2026-01-01')")).await.unwrap();
        for sql in invalid_writes().into_iter().chain([
            format!("UPDATE OR REPLACE sessions SET token_hash = 'other-current' WHERE id = '{S}'"),
            format!("INSERT OR REPLACE INTO sessions VALUES ('{S}', '{T}', '{U}', 'replacement', '2026-01-02')"),
            format!("INSERT OR REPLACE INTO session_refresh_history VALUES ('historical', '{S}', '2026-01-03')"),
            format!("INSERT OR REPLACE INTO sessions (rowid, id, tenant_id, user_id, token_hash, created_at)
                SELECT rowid, '{OTHER}', '{T}', '{U}', 'different', created_at FROM sessions WHERE id = '{S}'"),
            format!("UPDATE OR REPLACE sessions SET rowid = (SELECT rowid FROM sessions WHERE id = '{S2}') WHERE id = '{S}'"),
            format!("INSERT OR REPLACE INTO session_refresh_history (rowid, token_hash, session_id, rotated_at)
                SELECT rowid, 'different-history', '{S2}', rotated_at FROM session_refresh_history"),
        ]) {
            assert!(db.execute_unprepared(&sql).await.is_err(), "{sql}");
        }
        db.execute_unprepared(&format!(
            "INSERT INTO session_refresh_history VALUES ('current', '{S}', '2026-01-03');
             UPDATE sessions SET token_hash = 'successor' WHERE id = '{S}'"
        )).await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("DELETE FROM users WHERE id = '{U}'")).await.unwrap();
        assert_eq!(count(&tx, "session_refresh_history").await, 0);
        tx.rollback().await.unwrap();
        assert_eq!(count(&db, "session_refresh_history").await, 2);
        db.execute_unprepared(&format!("DELETE FROM sessions WHERE id = '{S}'")).await.unwrap();
        assert_eq!(count(&db, "session_refresh_history").await, 0);
    }

    #[tokio::test]
    async fn sqlite_session_evidence_migration_rejects_dirty_tenant_identity() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(&fixture_sql()).await.unwrap();
        db.execute_unprepared(&format!("UPDATE sessions SET tenant_id = '{OTHER}'")).await.unwrap();
        assert!(Migration.up(&SchemaManager::new(&db)).await.unwrap_err().to_string().contains("cross-tenant user"));
        assert_eq!(count(&db, "session_refresh_history").await, 1);
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_AUTH_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_session_evidence_is_immutable_and_cascades_only_with_parent() {
        let db: DatabaseConnection = Database::connect(std::env::var("RUSTOK_AUTH_TEST_DATABASE_URL").unwrap()).await.unwrap();
        let schema = format!("session_evidence_{}", uuid::Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx.execute_unprepared(&fixture_sql()).await.unwrap();
        super::super::m20261127_000010_fence_session_history_parents::Migration.up(&SchemaManager::new(&tx)).await.unwrap();
        Migration.up(&SchemaManager::new(&tx)).await.unwrap();
        tx.commit().await.unwrap();

        for sql in invalid_writes().into_iter().chain(["TRUNCATE session_refresh_history".into()]) {
            let tx = db.begin().await.unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
            assert!(tx.execute_unprepared(&sql).await.is_err(), "{sql}");
            tx.rollback().await.unwrap();
        }
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        assert_eq!(count(&tx, "session_refresh_history").await, 1);
        tx.execute_unprepared(&format!("DELETE FROM users WHERE id = '{U}'")).await.unwrap();
        assert_eq!(count(&tx, "session_refresh_history").await, 0);
        tx.commit().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
