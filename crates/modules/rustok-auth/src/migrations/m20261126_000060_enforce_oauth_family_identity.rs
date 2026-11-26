//! Canonical OAuth lineage identity and root retention. The database, not an
//! application fallback, owns the one-unrevoked-refresh invariant.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[cfg(test)]
const NIL: &str = "00000000-0000-0000-0000-000000000000";

// The old reader interpreted NULL as a self-root. Validate that exact
// interpretation before changing any rows. Never guess a missing root or
// silently choose one of several unrevoked branches.
const INVALID_FAMILY: &str = r#"
SELECT 1 FROM oauth_tokens t
WHERE t.id = '00000000-0000-0000-0000-000000000000'
   OR COALESCE(t.family_id, t.id) = '00000000-0000-0000-0000-000000000000'
   OR NOT EXISTS (
       SELECT 1 FROM oauth_tokens r
       WHERE r.id = COALESCE(t.family_id, t.id)
         AND COALESCE(r.family_id, r.id) = r.id
         AND r.tenant_id = t.tenant_id
         AND r.app_id = t.app_id
         AND r.user_id IS NOT DISTINCT FROM t.user_id
         AND r.authorization_code_id IS NOT DISTINCT FROM t.authorization_code_id
         AND r.consent_granted_at IS NOT DISTINCT FROM t.consent_granted_at
         AND r.grant_type = t.grant_type
         AND (r.id = t.id OR r.token_kind = 'refresh')
   )
LIMIT 1
"#;

const FORKED_FAMILY: &str = r#"
SELECT 1 FROM oauth_tokens
WHERE token_kind = 'refresh' AND revoked_at IS NULL
GROUP BY COALESCE(family_id, id)
HAVING COUNT(*) > 1
LIMIT 1
"#;

const ONE_REFRESH: &str = r#"
CREATE UNIQUE INDEX uq_oauth_tokens_unrevoked_refresh_family
ON oauth_tokens (family_id)
WHERE token_kind = 'refresh' AND revoked_at IS NULL
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = db.get_database_backend();
        match backend {
            DatabaseBackend::Postgres => {
                // The platform migrator executes DDL transactionally.
                db.execute_unprepared("LOCK TABLE oauth_tokens IN ACCESS EXCLUSIVE MODE").await?;
            }
            DatabaseBackend::Sqlite => {
                // Acquire the SQLite writer before inspecting the old lineage.
                db.execute_unprepared("UPDATE oauth_tokens SET family_id = family_id WHERE 0").await?;
            }
            _ => return Err(DbErr::Custom("Unsupported OAuth lineage database".into())),
        }
        for (sql, message) in [
            (INVALID_FAMILY, "OAuth family migration found a missing or inconsistent root"),
            (FORKED_FAMILY, "OAuth family migration found multiple unrevoked refresh branches"),
        ] {
            if db.query_one_raw(Statement::from_string(backend, sql.to_owned())).await?.is_some() {
                return Err(DbErr::Custom(message.into()));
            }
        }
        db.execute_unprepared("UPDATE oauth_tokens SET family_id = id WHERE family_id IS NULL").await?;
        db.execute_unprepared(ONE_REFRESH).await?;
        match backend {
            DatabaseBackend::Postgres => { db.execute_unprepared(POSTGRES_UP).await?; }
            DatabaseBackend::Sqlite => {
                for sql in [SQLITE_INSERT, SQLITE_UPDATE, SQLITE_DELETE] {
                    db.execute_unprepared(sql).await?;
                }
            }
            _ => unreachable!("backend checked above"),
        };
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match db.get_database_backend() {
            DatabaseBackend::Postgres => {
                db.execute_unprepared(r#"
DROP TRIGGER trg_oauth_family_identity ON oauth_tokens;
DROP FUNCTION rustok_guard_oauth_family_identity();
ALTER TABLE oauth_tokens DROP CONSTRAINT fk_oauth_tokens_family_root;
ALTER TABLE oauth_tokens DROP CONSTRAINT ck_oauth_tokens_family_non_nil;
ALTER TABLE oauth_tokens ALTER COLUMN family_id DROP NOT NULL;
"#).await?;
            }
            DatabaseBackend::Sqlite => {
                for trigger in ["trg_oauth_family_insert", "trg_oauth_family_update", "trg_oauth_family_delete"] {
                    db.execute_unprepared(&format!("DROP TRIGGER {trigger}")).await?;
                }
            }
            _ => return Err(DbErr::Custom("Unsupported OAuth lineage database".into())),
        }
        // The truthful backfill is not reversed into ambiguous NULLs.
        db.execute_unprepared("DROP INDEX uq_oauth_tokens_unrevoked_refresh_family").await?;
        Ok(())
    }
}

const POSTGRES_UP: &str = r#"
ALTER TABLE oauth_tokens ALTER COLUMN family_id SET NOT NULL;
ALTER TABLE oauth_tokens ADD CONSTRAINT ck_oauth_tokens_family_non_nil
CHECK (family_id <> '00000000-0000-0000-0000-000000000000'
   AND id <> '00000000-0000-0000-0000-000000000000');
ALTER TABLE oauth_tokens ADD CONSTRAINT fk_oauth_tokens_family_root
FOREIGN KEY (family_id) REFERENCES oauth_tokens(id)
DEFERRABLE INITIALLY DEFERRED;

CREATE FUNCTION rustok_guard_oauth_family_identity() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF ROW(NEW.id, NEW.family_id, NEW.tenant_id, NEW.app_id, NEW.user_id,
               NEW.authorization_code_id, NEW.consent_granted_at, NEW.grant_type,
               NEW.token_kind, NEW.created_at)
           IS DISTINCT FROM
           ROW(OLD.id, OLD.family_id, OLD.tenant_id, OLD.app_id, OLD.user_id,
               OLD.authorization_code_id, OLD.consent_granted_at, OLD.grant_type,
               OLD.token_kind, OLD.created_at) THEN
            RAISE EXCEPTION 'OAuth lineage identity is immutable' USING ERRCODE = '23514';
        END IF;
    ELSIF NEW.id <> NEW.family_id AND NOT EXISTS (
        SELECT 1 FROM oauth_tokens r
        WHERE r.id = NEW.family_id AND r.family_id = r.id
          AND r.tenant_id = NEW.tenant_id AND r.app_id = NEW.app_id
          AND r.user_id IS NOT DISTINCT FROM NEW.user_id
          AND r.authorization_code_id IS NOT DISTINCT FROM NEW.authorization_code_id
          AND r.consent_granted_at IS NOT DISTINCT FROM NEW.consent_granted_at
          AND r.grant_type = NEW.grant_type AND r.token_kind = 'refresh'
    ) THEN
        RAISE EXCEPTION 'OAuth family root mismatch' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER trg_oauth_family_identity BEFORE INSERT OR UPDATE ON oauth_tokens
FOR EACH ROW EXECUTE FUNCTION rustok_guard_oauth_family_identity();
"#;

// SQLite cannot ALTER an existing column to NOT NULL without rebuilding
// the table and its tenant guards. Enforce the equivalent write invariant
// using triggers; the Rust model is non-optional on both backends.
const SQLITE_INSERT: &str = r#"
CREATE TRIGGER trg_oauth_family_insert BEFORE INSERT ON oauth_tokens
FOR EACH ROW WHEN
    NEW.family_id IS NULL OR NEW.family_id = '00000000-0000-0000-0000-000000000000'
    OR NEW.id IS NULL OR NEW.id = '00000000-0000-0000-0000-000000000000'
    OR (NEW.id <> NEW.family_id AND NOT EXISTS (
        SELECT 1 FROM oauth_tokens r
        WHERE r.id = NEW.family_id AND r.family_id = r.id
          AND r.tenant_id = NEW.tenant_id AND r.app_id = NEW.app_id
          AND r.user_id IS NEW.user_id
          AND r.authorization_code_id IS NEW.authorization_code_id
          AND r.consent_granted_at IS NEW.consent_granted_at
          AND r.grant_type = NEW.grant_type AND r.token_kind = 'refresh'
    ))
BEGIN SELECT RAISE(ABORT, 'OAuth family root mismatch'); END
"#;
const SQLITE_UPDATE: &str = r#"
CREATE TRIGGER trg_oauth_family_update BEFORE UPDATE ON oauth_tokens
FOR EACH ROW WHEN NEW.id IS NOT OLD.id OR NEW.family_id IS NOT OLD.family_id
    OR NEW.tenant_id IS NOT OLD.tenant_id OR NEW.app_id IS NOT OLD.app_id
    OR NEW.user_id IS NOT OLD.user_id
    OR NEW.authorization_code_id IS NOT OLD.authorization_code_id
    OR NEW.consent_granted_at IS NOT OLD.consent_granted_at
    OR NEW.grant_type IS NOT OLD.grant_type OR NEW.token_kind IS NOT OLD.token_kind
    OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'OAuth lineage identity is immutable'); END
"#;

// Ordinary cleanup must delete descendants before the root. App/tenant
// deletion already cascades all scoped rows; allow that owner cascade.
const SQLITE_DELETE: &str = r#"
CREATE TRIGGER trg_oauth_family_delete BEFORE DELETE ON oauth_tokens
FOR EACH ROW WHEN OLD.id = OLD.family_id
    AND EXISTS (SELECT 1 FROM oauth_tokens t WHERE t.family_id = OLD.id AND t.id <> OLD.id)
    AND EXISTS (SELECT 1 FROM oauth_apps a WHERE a.id = OLD.app_id AND a.tenant_id = OLD.tenant_id)
    AND EXISTS (SELECT 1 FROM tenants t WHERE t.id = OLD.tenant_id)
BEGIN SELECT RAISE(ABORT, 'OAuth family root still has descendants'); END
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::Database;

    async fn fixture() -> sea_orm_migration::sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(r#"
CREATE TABLE tenants (id TEXT PRIMARY KEY);
CREATE TABLE oauth_apps (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL);
CREATE TABLE oauth_tokens (
    id TEXT PRIMARY KEY, family_id TEXT, tenant_id TEXT NOT NULL, app_id TEXT NOT NULL,
    user_id TEXT, authorization_code_id TEXT, consent_granted_at TEXT,
    grant_type TEXT NOT NULL DEFAULT 'authorization_code',
    token_kind TEXT NOT NULL DEFAULT 'refresh', created_at TEXT NOT NULL DEFAULT '2026-01-01',
    revoked_at TEXT
);
INSERT INTO tenants VALUES ('tenant');
INSERT INTO oauth_apps VALUES ('app', 'tenant');
INSERT INTO oauth_tokens (id, tenant_id, app_id, user_id) VALUES ('root', 'tenant', 'app', 'user');
"#).await.unwrap();
        db
    }

    #[tokio::test]
    async fn canonical_family_backfill_and_guards() {
        let db = fixture().await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let root = db.query_one_raw(Statement::from_string(DatabaseBackend::Sqlite,
            "SELECT family_id FROM oauth_tokens WHERE id = 'root'".to_owned())).await.unwrap().unwrap();
        assert_eq!(root.try_get::<String>("", "family_id").unwrap(), "root");
        for sql in [
            "INSERT INTO oauth_tokens (id, tenant_id, app_id) VALUES ('null', 'tenant', 'app')",
            "INSERT INTO oauth_tokens (id, family_id, tenant_id, app_id) VALUES ('missing', 'missing-root', 'tenant', 'app')",
            "INSERT INTO oauth_tokens (id, family_id, tenant_id, app_id, user_id) VALUES ('foreign', 'root', 'tenant', 'app', 'other')",
            "INSERT INTO oauth_tokens (id, family_id, tenant_id, app_id, user_id) VALUES ('fork', 'root', 'tenant', 'app', 'user')",
            "UPDATE oauth_tokens SET family_id = NULL WHERE id = 'root'",
        ] {
            assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
        }
        assert!(db.execute_unprepared(&format!(
            "INSERT INTO oauth_tokens (id, family_id, tenant_id, app_id) VALUES ('{NIL}', '{NIL}', 'tenant', 'app')"
        )).await.is_err());
        db.execute_unprepared("UPDATE oauth_tokens SET revoked_at = '2026-01-02' WHERE id = 'root'").await.unwrap();
        db.execute_unprepared("INSERT INTO oauth_tokens (id, family_id, tenant_id, app_id, user_id) VALUES ('child', 'root', 'tenant', 'app', 'user')").await.unwrap();
        assert!(db.execute_unprepared("UPDATE oauth_tokens SET user_id = 'other' WHERE id = 'root'").await.is_err());
        assert!(db.execute_unprepared("DELETE FROM oauth_tokens WHERE id = 'root'").await.is_err());
        db.execute_unprepared("DELETE FROM oauth_tokens WHERE id = 'child'").await.unwrap();
        db.execute_unprepared("DELETE FROM oauth_tokens WHERE id = 'root'").await.unwrap();
    }

    #[tokio::test]
    async fn migration_rejects_existing_fork_without_reassigning_history() {
        let db = fixture().await;
        db.execute_unprepared("INSERT INTO oauth_tokens (id, family_id, tenant_id, app_id, user_id) VALUES ('fork', 'root', 'tenant', 'app', 'user')").await.unwrap();
        assert!(Migration.up(&SchemaManager::new(&db)).await.is_err());
        let row = db.query_one_raw(Statement::from_string(DatabaseBackend::Sqlite,
            "SELECT family_id FROM oauth_tokens WHERE id = 'root'".to_owned())).await.unwrap().unwrap();
        assert_eq!(row.try_get::<Option<String>>("", "family_id").unwrap(), None);
    }
}
