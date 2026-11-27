//! Permanent credential identity and session/OAuth digest claims. Auth owns the
//! semantics; no credential plaintext or live authorization is stored here.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

// The session row is owned by Auth semantically, although the platform
// identity schema creates it. Its digest is claimed on birth and rotation,
// but unlike OAuth digests it is not immutable on the live row.
const CLAIMS: &[(&str, &str, &str, bool)] = &[
    ("sessions", "id", "session", true),
    ("sessions", "token_hash", "session_digest", false),
    ("oauth_apps", "id", "oauth_app", true),
    ("oauth_apps", "client_id", "oauth_client", true),
    ("oauth_tokens", "id", "oauth_token", true),
    ("oauth_tokens", "token_hash", "oauth_token_digest", false),
    ("oauth_authorization_codes", "id", "oauth_code", true),
    ("oauth_authorization_codes", "code_hash", "oauth_code_digest", false),
];

const KINDS: &str = "'session', 'session_digest', 'oauth_app', 'oauth_client', 'oauth_token', 'oauth_token_digest', 'oauth_code', 'oauth_code_digest'";

const POSTGRES_GUARDS: &str = r#"
CREATE FUNCTION auth_guard_credential_witness() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'INSERT' THEN
        RAISE EXCEPTION 'Auth credential witness is permanent' USING ERRCODE = '23514';
    END IF;
    IF NOT pg_try_advisory_xact_lock(hashtextextended(
        'rustok:auth-credential:' || NEW.kind || ':' || NEW.identity_key, 0)) THEN
        RAISE EXCEPTION 'Auth credential witness busy' USING ERRCODE = '55P03';
    END IF;
    RETURN NEW;
END; $$;
CREATE TRIGGER auth_credential_witness_guard
BEFORE INSERT OR UPDATE OR DELETE ON auth_credential_witnesses
FOR EACH ROW EXECUTE FUNCTION auth_guard_credential_witness();
CREATE TRIGGER auth_credential_witness_no_truncate
BEFORE TRUNCATE ON auth_credential_witnesses
FOR EACH STATEMENT EXECUTE FUNCTION auth_guard_credential_witness();

CREATE FUNCTION auth_claim_credential_witness() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    identity_value text;
BEGIN
    LOCK TABLE auth_credential_witnesses IN ROW EXCLUSIVE MODE NOWAIT;
    IF row_security_active('auth_credential_witnesses'::regclass) THEN
        RAISE EXCEPTION 'Auth credential witness visibility incomplete' USING ERRCODE = '42501';
    END IF;
    identity_value := to_jsonb(NEW) ->> TG_ARGV[1];
    IF TG_ARGV[2] = 'uuid' THEN
        identity_value := replace(identity_value, '-', '');
    END IF;
    INSERT INTO auth_credential_witnesses (kind, identity_key)
    VALUES (TG_ARGV[0], identity_value);
    RETURN NULL;
END; $$;

CREATE FUNCTION auth_guard_credential_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF (to_jsonb(NEW) -> TG_ARGV[0]) IS DISTINCT FROM (to_jsonb(OLD) -> TG_ARGV[0]) THEN
        RAISE EXCEPTION 'Auth credential identity is immutable' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END; $$;
"#;

const SQLITE_GUARDS: &str = r#"
CREATE TRIGGER auth_credential_witness_insert_guard BEFORE INSERT ON auth_credential_witnesses
WHEN NEW.kind IS NULL OR NEW.identity_key IS NULL OR typeof(NEW.identity_key) <> 'text'
 OR length(NEW.identity_key) = 0
 OR (NEW.kind NOT IN ('session_digest', 'oauth_token_digest', 'oauth_code_digest') AND
     (length(NEW.identity_key) <> 32 OR NEW.identity_key GLOB '*[^0-9a-f]*'))
 OR EXISTS (SELECT 1 FROM auth_credential_witnesses
            WHERE kind = NEW.kind AND identity_key = NEW.identity_key)
BEGIN SELECT RAISE(ABORT, 'Auth credential identity invalid or already consumed'); END;
CREATE TRIGGER auth_credential_witness_no_update BEFORE UPDATE ON auth_credential_witnesses
BEGIN SELECT RAISE(ABORT, 'Auth credential witness is permanent'); END;
CREATE TRIGGER auth_credential_witness_no_delete BEFORE DELETE ON auth_credential_witnesses
BEGIN SELECT RAISE(ABORT, 'Auth credential witness is permanent'); END;
"#;

fn key_expression(backend: DatabaseBackend, column: &str, uuid: bool) -> String {
    if !uuid {
        return column.to_owned();
    }
    match backend {
        DatabaseBackend::Postgres => format!("replace({column}::text, '-', '')"),
        DatabaseBackend::Sqlite => format!(
            "CASE typeof({column}) WHEN 'blob' THEN lower(hex({column})) \
             WHEN 'text' THEN lower(replace({column}, '-', '')) END"
        ),
        _ => unreachable!("backend checked"),
    }
}

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
                db.execute_unprepared(
                    "LOCK TABLE sessions, session_refresh_history, oauth_apps, oauth_authorization_codes, oauth_tokens IN SHARE ROW EXCLUSIVE MODE;
                     DO $$ BEGIN
                       IF row_security_active('sessions'::regclass) OR row_security_active('session_refresh_history'::regclass)
                          OR row_security_active('oauth_apps'::regclass)
                          OR row_security_active('oauth_authorization_codes'::regclass) OR row_security_active('oauth_tokens'::regclass)
                       THEN RAISE EXCEPTION 'Auth identity backfill requires complete visibility' USING ERRCODE = '42501'; END IF;
                     END $$;"
                ).await?;
            }
            DatabaseBackend::Sqlite => {
                db.execute_unprepared("UPDATE sessions SET id = id WHERE 0").await?;
            }
            _ => return Err(DbErr::Migration("unsupported Auth identity backend".into())),
        }
        let shape = match backend {
            DatabaseBackend::Postgres => "identity_key ~ '^[0-9a-f]{32}$'",
            _ => "typeof(identity_key) = 'text' AND length(identity_key) = 32 AND identity_key NOT GLOB '*[^0-9a-f]*'",
        };
        // SQLite's hidden rowid would be a second conflict key: REPLACE
        // could delete an unrelated consumed identity without a DELETE trigger.
        let table_suffix = if backend == DatabaseBackend::Sqlite { " WITHOUT ROWID" } else { "" };
        db.execute_unprepared(&format!(
            "CREATE TABLE auth_credential_witnesses (
                kind TEXT NOT NULL CHECK (kind IN ({KINDS})),
                identity_key TEXT NOT NULL CHECK (length(identity_key) > 0 AND
                    (kind IN ('session_digest', 'oauth_token_digest', 'oauth_code_digest') OR ({shape}))),
                PRIMARY KEY (kind, identity_key)
             ){table_suffix}"
        )).await?;
        for &(table, column, kind, uuid) in CLAIMS {
            // No conflict suppression: existing alias or duplicate evidence
            // must stop migration rather than be silently merged.
            db.execute_unprepared(&format!(
                "INSERT INTO auth_credential_witnesses (kind, identity_key)
                 SELECT '{kind}', {} FROM {table}",
                key_expression(backend, column, uuid)
            )).await?;
        }
        // History and the current session digest must be disjoint. Do not
        // hide duplicate or inconsistent retained evidence with ON CONFLICT.
        db.execute_unprepared(
            "INSERT INTO auth_credential_witnesses (kind, identity_key)
             SELECT 'session_digest', token_hash FROM session_refresh_history"
        ).await?;
        db.execute_unprepared(match backend {
            DatabaseBackend::Postgres => POSTGRES_GUARDS,
            _ => SQLITE_GUARDS,
        }).await?;
        for &(table, column, kind, uuid) in CLAIMS {
            let trigger = format!("auth_{kind}_identity");
            if kind == "session_digest" {
                // AFTER UPDATE only claims a real rotation. A no-op or
                // idempotent response recovery must consume nothing.
                let sql = match backend {
                    DatabaseBackend::Postgres =>
                        "CREATE TRIGGER auth_session_digest_claim AFTER INSERT ON sessions
                         FOR EACH ROW EXECUTE FUNCTION auth_claim_credential_witness('session_digest', 'token_hash', 'digest');
                         CREATE TRIGGER auth_session_digest_rotate AFTER UPDATE ON sessions
                         FOR EACH ROW WHEN (NEW.token_hash IS DISTINCT FROM OLD.token_hash)
                         EXECUTE FUNCTION auth_claim_credential_witness('session_digest', 'token_hash', 'digest');",
                    DatabaseBackend::Sqlite =>
                        "CREATE TRIGGER auth_session_digest_claim AFTER INSERT ON sessions
                         BEGIN INSERT INTO auth_credential_witnesses (kind, identity_key)
                            VALUES ('session_digest', NEW.token_hash); END;
                         CREATE TRIGGER auth_session_digest_rotate AFTER UPDATE ON sessions
                         WHEN NEW.token_hash IS NOT OLD.token_hash
                         BEGIN INSERT INTO auth_credential_witnesses (kind, identity_key)
                            VALUES ('session_digest', NEW.token_hash); END;",
                    _ => unreachable!("backend checked"),
                };
                db.execute_unprepared(sql).await?;
                continue;
            }
            match backend {
                DatabaseBackend::Postgres => {
                    let key_kind = if uuid { "uuid" } else { "digest" };
                    db.execute_unprepared(&format!(
                        "CREATE TRIGGER {trigger}_claim AFTER INSERT ON {table}
                         FOR EACH ROW EXECUTE FUNCTION auth_claim_credential_witness('{kind}', '{column}', '{key_kind}');
                         CREATE TRIGGER {trigger}_guard BEFORE UPDATE ON {table}
                         FOR EACH ROW EXECUTE FUNCTION auth_guard_credential_identity('{column}');"
                    )).await?;
                }
                DatabaseBackend::Sqlite => {
                    db.execute_unprepared(&format!(
                        "CREATE TRIGGER {trigger}_claim AFTER INSERT ON {table}
                         BEGIN INSERT INTO auth_credential_witnesses (kind, identity_key)
                            VALUES ('{kind}', {}); END;
                         CREATE TRIGGER {trigger}_guard BEFORE UPDATE ON {table}
                         WHEN NEW.{column} IS NOT OLD.{column}
                         BEGIN SELECT RAISE(ABORT, 'Auth credential identity is immutable'); END;",
                        key_expression(backend, &format!("NEW.{column}"), uuid)
                    )).await?;
                }
                _ => unreachable!("backend checked"),
            }
        }
        // SQLite REPLACE on an unrevoked refresh family's unique index can
        // delete an unrelated predecessor without invoking DELETE triggers.
        // Canonical token/code issuance uses plain INSERT, not upsert.
        if backend == DatabaseBackend::Sqlite {
            db.execute_unprepared(
                "CREATE TRIGGER auth_oauth_refresh_replace_guard BEFORE INSERT ON oauth_tokens
                 WHEN NEW.token_kind = 'refresh' AND NEW.revoked_at IS NULL AND EXISTS (
                     SELECT 1 FROM oauth_tokens WHERE family_id = NEW.family_id
                       AND token_kind = 'refresh' AND revoked_at IS NULL)
                 BEGIN SELECT RAISE(ABORT, 'OAuth refresh family already has a live member'); END;
                 CREATE TRIGGER auth_oauth_refresh_update_replace_guard BEFORE UPDATE OF revoked_at ON oauth_tokens
                 WHEN NEW.token_kind = 'refresh' AND NEW.revoked_at IS NULL AND EXISTS (
                     SELECT 1 FROM oauth_tokens WHERE family_id = NEW.family_id AND id <> OLD.id
                       AND token_kind = 'refresh' AND revoked_at IS NULL)
                 BEGIN SELECT RAISE(ABORT, 'OAuth refresh family already has a live member'); END;"
            ).await?;
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("Auth credential witnesses cannot be discarded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, Statement, TransactionTrait};

    const A: &str = "10000000-0000-0000-0000-000000000001";
    const B: &str = "10000000-0000-0000-0000-000000000002";
    const C: &str = "10000000-0000-0000-0000-000000000003";
    const D: &str = "10000000-0000-0000-0000-000000000004";

    async fn fixture(db: &impl ConnectionTrait) {
        db.execute_unprepared(&format!(
            "CREATE TABLE sessions (id UUID PRIMARY KEY NOT NULL, token_hash TEXT NOT NULL);
             CREATE TABLE session_refresh_history (token_hash TEXT PRIMARY KEY, session_id UUID NOT NULL);
             INSERT INTO session_refresh_history VALUES ('historical-session', '{A}');
             CREATE TABLE oauth_apps (id UUID PRIMARY KEY NOT NULL, client_id UUID NOT NULL UNIQUE);
             CREATE TABLE oauth_tokens (id UUID PRIMARY KEY NOT NULL, token_hash TEXT NOT NULL UNIQUE,
               family_id UUID NOT NULL, token_kind TEXT NOT NULL, revoked_at TIMESTAMP);
             CREATE UNIQUE INDEX one_refresh ON oauth_tokens(family_id) WHERE token_kind = 'refresh' AND revoked_at IS NULL;
             CREATE TABLE oauth_authorization_codes (id UUID PRIMARY KEY NOT NULL, code_hash TEXT NOT NULL UNIQUE);
             INSERT INTO sessions VALUES ('{A}', 'old-session');
             INSERT INTO oauth_apps VALUES ('{A}', '{B}');
             INSERT INTO oauth_tokens VALUES ('{A}', 'old-token', '{A}', 'refresh', NULL);
             INSERT INTO oauth_authorization_codes VALUES ('{A}', 'old-code');"
        )).await.unwrap();
    }

    async fn count(db: &impl ConnectionTrait) -> i64 {
        db.query_one_raw(Statement::from_string(db.get_database_backend(),
            "SELECT COUNT(*) AS count FROM auth_credential_witnesses"))
            .await.unwrap().unwrap().try_get("", "count").unwrap()
    }

    #[tokio::test]
    async fn sqlite_identity_and_digest_claims_survive_cleanup_and_replacement() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA recursive_triggers = OFF").await.unwrap();
        fixture(&db).await;
        let tx = db.begin().await.unwrap();
        Migration.up(&SchemaManager::new(&tx)).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(count(&db).await, 9);

        // Session refresh rotation remains legal; it is not a new session.
        db.execute_unprepared("UPDATE sessions SET token_hash = 'rotated-session'").await.unwrap();
        db.execute_unprepared("UPDATE sessions SET token_hash = token_hash").await.unwrap();
        assert_eq!(count(&db).await, 10);
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("UPDATE sessions SET token_hash = 'rolled-back-session'").await.unwrap();
        tx.rollback().await.unwrap();
        assert_eq!(count(&db).await, 10);
        for digest in ["old-session", "historical-session"] {
            assert!(db.execute_unprepared(&format!("UPDATE OR IGNORE sessions SET token_hash = '{digest}'")).await.is_err());
        }
        for sql in [
            format!("INSERT OR REPLACE INTO oauth_tokens VALUES ('{C}', 'new-token', '{A}', 'refresh', NULL)"),
            format!("INSERT OR REPLACE INTO oauth_tokens VALUES ('{C}', 'old-token', '{C}', 'refresh', NULL)"),
            format!("INSERT OR REPLACE INTO oauth_apps VALUES ('{C}', '{B}')"),
            format!("INSERT OR REPLACE INTO oauth_authorization_codes VALUES ('{C}', 'old-code')"),
            format!("UPDATE oauth_apps SET client_id = '{D}'"),
            "UPDATE oauth_tokens SET token_hash = 'changed'".to_owned(),
            "UPDATE oauth_authorization_codes SET code_hash = 'changed'".to_owned(),
        ] {
            assert!(db.execute_unprepared(&sql).await.is_err(), "{sql}");
        }
        for table in ["session_refresh_history", "sessions", "oauth_apps", "oauth_tokens", "oauth_authorization_codes"] {
            db.execute_unprepared(&format!("DELETE FROM {table}")).await.unwrap();
        }
        for sql in [
            format!("INSERT INTO sessions VALUES ('{A}', 'new-session')"),
            format!("INSERT OR REPLACE INTO sessions VALUES ('{C}', 'old-session')"),
            format!("INSERT OR IGNORE INTO sessions VALUES ('{C}', 'historical-session')"),
            format!("INSERT INTO sessions VALUES ('{C}', 'rotated-session')"),
            format!("INSERT INTO oauth_apps VALUES ('{A}', '{D}')"),
            format!("INSERT INTO oauth_apps VALUES ('{C}', '{B}')"),
            format!("INSERT INTO oauth_tokens VALUES ('{A}', 'new-token', '{A}', 'refresh', NULL)"),
            format!("INSERT OR IGNORE INTO oauth_tokens VALUES ('{C}', 'old-token', '{C}', 'refresh', NULL)"),
            format!("INSERT INTO oauth_authorization_codes VALUES ('{A}', 'new-code')"),
            format!("INSERT OR IGNORE INTO oauth_authorization_codes VALUES ('{C}', 'old-code')"),
            format!("INSERT INTO sessions VALUES (X'{}', 'alias')", A.replace('-', "")),
            "DELETE FROM auth_credential_witnesses".to_owned(),
            "UPDATE auth_credential_witnesses SET identity_key = identity_key".to_owned(),
            "INSERT OR REPLACE INTO auth_credential_witnesses VALUES ('oauth_code_digest', 'old-code')".to_owned(),
            "INSERT OR REPLACE INTO auth_credential_witnesses (rowid, kind, identity_key)
             VALUES (1, 'oauth_code_digest', 'different-code')".to_owned(),
            "INSERT OR REPLACE INTO auth_credential_witnesses (_rowid_, kind, identity_key)
             VALUES (1, 'oauth_token_digest', 'different-token')".to_owned(),
            "INSERT OR REPLACE INTO auth_credential_witnesses (oid, kind, identity_key)
             VALUES (1, 'oauth_code_digest', 'different-code')".to_owned(),
        ] {
            assert!(db.execute_unprepared(&sql).await.is_err(), "{sql}");
        }
        assert_eq!(count(&db).await, 10);
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("INSERT INTO oauth_tokens VALUES ('{C}', 'new-token', '{C}', 'refresh', NULL)")).await.unwrap();
        tx.rollback().await.unwrap();
        db.execute_unprepared(&format!("INSERT INTO oauth_tokens VALUES ('{C}', 'new-token', '{C}', 'refresh', NULL)")).await.unwrap();
        assert_eq!(count(&db).await, 12);
        // Rotation after revoking the old live row still works.
        db.execute_unprepared(&format!(
            "UPDATE oauth_tokens SET revoked_at = '2026-01-01' WHERE id = '{C}';
             INSERT INTO oauth_tokens VALUES ('{D}', 'successor', '{C}', 'refresh', NULL)"
        )).await.unwrap();
        assert_eq!(count(&db).await, 14);
        db.execute_unprepared(&format!("INSERT INTO sessions VALUES ('{C}', 'rolled-back-session')")).await.unwrap();
        assert_eq!(count(&db).await, 16);
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_AUTH_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_credential_witnesses_reject_reuse_and_late_contention() {
        use std::time::Duration;
        let db = Database::connect(std::env::var("RUSTOK_AUTH_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("auth_identity_{}", uuid::Uuid::new_v4().simple());
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("CREATE SCHEMA {schema}; SET LOCAL search_path TO {schema}")).await.unwrap();
        fixture(&tx).await;
        Migration.up(&SchemaManager::new(&tx)).await.unwrap();
        assert_eq!(count(&tx).await, 9);
        tx.execute_unprepared("UPDATE sessions SET token_hash = 'rotated-session'").await.unwrap();
        assert_eq!(count(&tx).await, 10);
        tx.execute_unprepared("DELETE FROM session_refresh_history; DELETE FROM sessions").await.unwrap();
        tx.execute_unprepared("DELETE FROM oauth_tokens").await.unwrap();
        tx.commit().await.unwrap();

        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        assert!(tx.execute_unprepared(&format!("INSERT INTO oauth_tokens VALUES ('{C}', 'old-token', '{C}', 'refresh', NULL)")).await.is_err());
        tx.rollback().await.unwrap();
        for digest in ["old-session", "historical-session", "rotated-session"] {
            let tx = db.begin().await.unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
            assert!(tx.execute_unprepared(&format!("INSERT INTO sessions VALUES ('{C}', '{digest}')")).await.is_err());
            tx.rollback().await.unwrap();
        }
        let holder = db.begin().await.unwrap();
        holder.execute_unprepared(&format!("SET LOCAL search_path TO {schema}; INSERT INTO auth_credential_witnesses VALUES ('oauth_token_digest', 'fresh')")).await.unwrap();
        let contender = db.begin().await.unwrap();
        contender.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        let error = tokio::time::timeout(Duration::from_secs(3), contender.execute_unprepared(
            &format!("INSERT INTO oauth_tokens VALUES ('{C}', 'fresh', '{C}', 'refresh', NULL)")
        )).await.expect("witness must not wait").expect_err("competing digest must fail");
        assert!(error.to_string().contains("Auth credential witness busy"), "{error}");
        contender.rollback().await.unwrap();
        holder.rollback().await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}; INSERT INTO oauth_tokens VALUES ('{C}', 'fresh', '{C}', 'refresh', NULL)")).await.unwrap();
        assert_eq!(count(&tx).await, 12);
        tx.commit().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
