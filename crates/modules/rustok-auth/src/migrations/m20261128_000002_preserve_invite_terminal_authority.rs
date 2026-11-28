//! Preserve invitation terminal facts independently of metadata retention.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
CREATE FUNCTION auth_guard_invite_digest_witness() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'INSERT' THEN
        RAISE EXCEPTION 'Invite digest witness is permanent' USING ERRCODE = '23514';
    END IF;
    PERFORM auth_fence_invite_consumption_digest(NEW.token_hash);
    RETURN NEW;
END; $$;
CREATE TRIGGER auth_invite_digest_witness_guard BEFORE INSERT OR UPDATE OR DELETE ON auth_invite_digest_witnesses
FOR EACH ROW EXECUTE FUNCTION auth_guard_invite_digest_witness();
CREATE TRIGGER auth_invite_digest_witness_no_truncate BEFORE TRUNCATE ON auth_invite_digest_witnesses
FOR EACH STATEMENT EXECUTE FUNCTION auth_guard_invite_digest_witness();

CREATE FUNCTION auth_fence_invite_issuance_id(target uuid) RETURNS void LANGUAGE plpgsql AS $
BEGIN
    IF NOT pg_try_advisory_xact_lock(
        hashtextextended('rustok:auth-invite-issuance:id:' || target::text, 0)) THEN
        RAISE EXCEPTION 'Invite issuance identity busy' USING ERRCODE = '55P03';
    END IF;
END; $;

CREATE FUNCTION auth_guard_invite_birth() RETURNS trigger LANGUAGE plpgsql AS $
BEGIN
    LOCK TABLE auth_invite_digest_witnesses IN ROW EXCLUSIVE MODE NOWAIT;
    PERFORM auth_fence_invite_consumption_digest(NEW.token_hash);
    IF TG_TABLE_NAME = 'auth_invites' THEN
        PERFORM auth_fence_invite_issuance_id(NEW.id);
    END IF;
    IF row_security_active('auth_invite_digest_witnesses'::regclass) THEN
        RAISE EXCEPTION 'Invite witness visibility incomplete' USING ERRCODE = '42501';
    END IF;
    IF EXISTS (SELECT 1 FROM auth_invite_digest_witnesses
               WHERE token_hash = NEW.token_hash AND
                 (kind = TG_ARGV[0] OR TG_ARGV[0] = 'issued')) THEN
        RAISE EXCEPTION 'Invite digest already registered' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END; $$;
CREATE FUNCTION auth_claim_invite_birth() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO auth_invite_digest_witnesses (kind, token_hash) VALUES (TG_ARGV[0], NEW.token_hash);
    RETURN NULL;
END; $$;
CREATE TRIGGER auth_invite_birth_guard BEFORE INSERT ON auth_invites
FOR EACH ROW EXECUTE FUNCTION auth_guard_invite_birth('issued');
CREATE TRIGGER auth_invite_receipt_birth_guard BEFORE INSERT ON auth_invite_consumptions
FOR EACH ROW EXECUTE FUNCTION auth_guard_invite_birth('consumed');
CREATE TRIGGER auth_invite_birth_claim AFTER INSERT ON auth_invites
FOR EACH ROW EXECUTE FUNCTION auth_claim_invite_birth('issued');
CREATE TRIGGER auth_invite_receipt_birth_claim AFTER INSERT ON auth_invite_consumptions
FOR EACH ROW EXECUTE FUNCTION auth_claim_invite_birth('consumed');

CREATE FUNCTION auth_guard_invite_issued_claims() RETURNS trigger LANGUAGE plpgsql AS $
BEGIN
    -- Raw UPDATE may already own the tuple; the gate must never wait.
    PERFORM auth_fence_invite_consumption_digest(OLD.token_hash);
    PERFORM auth_fence_invite_issuance_id(OLD.id);
    IF ROW(NEW.id, NEW.tenant_id, NEW.token_hash, NEW.email, NEW.role,
           NEW.created_at, NEW.expires_at, NEW.issuer_recovery_revision,
           NEW.issuer_membership_revision, NEW.role_grant_binding, NEW.module_policy_binding)
       IS DISTINCT FROM
       ROW(OLD.id, OLD.tenant_id, OLD.token_hash, OLD.email, OLD.role,
           OLD.created_at, OLD.expires_at, OLD.issuer_recovery_revision,
           OLD.issuer_membership_revision, OLD.role_grant_binding, OLD.module_policy_binding)
       OR (OLD.revoked_at IS NOT NULL AND NEW.revoked_at IS DISTINCT FROM OLD.revoked_at) THEN
        RAISE EXCEPTION 'Invite issued claims and revocation are immutable' USING ERRCODE = '23514';
    END IF;
    IF NEW.issued_by IS DISTINCT FROM OLD.issued_by THEN
        LOCK TABLE users IN ACCESS SHARE MODE NOWAIT;
        IF NEW.issued_by IS NOT NULL OR OLD.issued_by IS NULL
           OR row_security_active('users'::regclass)
           OR EXISTS (SELECT 1 FROM users WHERE id = OLD.issued_by) THEN
            RAISE EXCEPTION 'Invite issuer can only be cleared by parent deletion' USING ERRCODE = '23514';
        END IF;
    END IF;
    RETURN NEW;
END; $$;
CREATE TRIGGER auth_invite_issued_claims_guard BEFORE UPDATE ON auth_invites
FOR EACH ROW EXECUTE FUNCTION auth_guard_invite_issued_claims();

CREATE FUNCTION auth_guard_invite_receipt_claims() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF ROW(NEW.id, NEW.tenant_id, NEW.token_hash, NEW.email, NEW.role,
           NEW.expires_at, NEW.consumed_at) IS DISTINCT FROM
       ROW(OLD.id, OLD.tenant_id, OLD.token_hash, OLD.email, OLD.role,
           OLD.expires_at, OLD.consumed_at) THEN
        RAISE EXCEPTION 'Invite receipt claims are immutable' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END; $$;
CREATE TRIGGER auth_invite_receipt_claims_guard BEFORE UPDATE ON auth_invite_consumptions
FOR EACH ROW EXECUTE FUNCTION auth_guard_invite_receipt_claims();

CREATE FUNCTION auth_guard_invite_metadata_delete() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'TRUNCATE' THEN
        RAISE EXCEPTION 'Invite metadata cannot be truncated' USING ERRCODE = '23514';
    END IF;
    PERFORM auth_fence_invite_consumption_digest(OLD.token_hash);
    IF TG_TABLE_NAME = 'auth_invites' THEN
        PERFORM auth_fence_invite_issuance_id(OLD.id);
    END IF;
    LOCK TABLE tenants IN ACCESS SHARE MODE NOWAIT;
    IF row_security_active('tenants'::regclass) THEN
        RAISE EXCEPTION 'Invite tenant visibility incomplete' USING ERRCODE = '42501';
    END IF;
    -- Tenant FK cascade observes its own parent removal. Witnesses have
    -- no parent FK and therefore survive even this deletion.
    IF NOT EXISTS (SELECT 1 FROM tenants WHERE id = OLD.tenant_id) THEN RETURN OLD; END IF;
    IF OLD.expires_at >= clock_timestamp() - (TG_ARGV[0]::integer * interval '24 hours') THEN
        RAISE EXCEPTION 'Invite metadata is still retained' USING ERRCODE = '23514';
    END IF;
    IF TG_TABLE_NAME = 'auth_invite_consumptions' THEN
        LOCK TABLE auth_invites IN ACCESS SHARE MODE NOWAIT;
        IF row_security_active('auth_invites'::regclass) OR
           EXISTS (SELECT 1 FROM auth_invites WHERE token_hash = OLD.token_hash) THEN
            RAISE EXCEPTION 'Invite receipt requires issuance ledger retention first' USING ERRCODE = '23514';
        END IF;
    END IF;
    RETURN OLD;
END; $$;
"#;

const SQLITE_WITNESS: &str = r#"
CREATE TRIGGER auth_invite_digest_witness_insert_guard BEFORE INSERT ON auth_invite_digest_witnesses
WHEN NEW.kind IS NULL OR NEW.kind NOT IN ('issued', 'consumed')
 OR typeof(NEW.token_hash) <> 'text' OR length(NEW.token_hash) <> 64
 OR NEW.token_hash GLOB '*[^0-9a-f]*'
 OR EXISTS (SELECT 1 FROM auth_invite_digest_witnesses
             WHERE kind = NEW.kind AND token_hash = NEW.token_hash)
BEGIN SELECT RAISE(ABORT, 'Invite digest invalid or already registered'); END;
CREATE TRIGGER auth_invite_digest_witness_no_update BEFORE UPDATE ON auth_invite_digest_witnesses
BEGIN SELECT RAISE(ABORT, 'Invite digest witness is permanent'); END;
CREATE TRIGGER auth_invite_digest_witness_no_delete BEFORE DELETE ON auth_invite_digest_witnesses
BEGIN SELECT RAISE(ABORT, 'Invite digest witness is permanent'); END;

CREATE TRIGGER auth_invite_issued_claims_guard BEFORE UPDATE ON auth_invites
WHEN NEW.id IS NOT OLD.id OR NEW.tenant_id IS NOT OLD.tenant_id
 OR NEW.token_hash IS NOT OLD.token_hash OR NEW.email IS NOT OLD.email
 OR NEW.role IS NOT OLD.role OR NEW.created_at IS NOT OLD.created_at
 OR NEW.expires_at IS NOT OLD.expires_at
 OR NEW.issuer_recovery_revision IS NOT OLD.issuer_recovery_revision
 OR NEW.issuer_membership_revision IS NOT OLD.issuer_membership_revision
 OR NEW.role_grant_binding IS NOT OLD.role_grant_binding
 OR NEW.module_policy_binding IS NOT OLD.module_policy_binding
 OR (OLD.revoked_at IS NOT NULL AND NEW.revoked_at IS NOT OLD.revoked_at)
 OR (NEW.issued_by IS NOT OLD.issued_by AND
     (NEW.issued_by IS NOT NULL OR OLD.issued_by IS NULL OR
      EXISTS (SELECT 1 FROM users WHERE id = OLD.issued_by)))
BEGIN SELECT RAISE(ABORT, 'Invite issued claims and revocation are immutable'); END;

CREATE TRIGGER auth_invite_receipt_claims_guard BEFORE UPDATE ON auth_invite_consumptions
WHEN NEW.id IS NOT OLD.id OR NEW.tenant_id IS NOT OLD.tenant_id
 OR NEW.token_hash IS NOT OLD.token_hash OR NEW.email IS NOT OLD.email
 OR NEW.role IS NOT OLD.role OR NEW.expires_at IS NOT OLD.expires_at
 OR NEW.consumed_at IS NOT OLD.consumed_at
BEGIN SELECT RAISE(ABORT, 'Invite receipt claims are immutable'); END;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> { Some(true) }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();
        match backend {
            DatabaseBackend::Postgres => {
                db.execute_unprepared(
                    "LOCK TABLE auth_invites, auth_invite_consumptions IN SHARE ROW EXCLUSIVE MODE;
                     DO $$ BEGIN
                        IF row_security_active('auth_invites'::regclass) OR
                           row_security_active('auth_invite_consumptions'::regclass) THEN
                            RAISE EXCEPTION 'Invite witness backfill requires complete visibility' USING ERRCODE = '42501';
                        END IF;
                     END $$;"
                ).await?;
            }
            DatabaseBackend::Sqlite => {
                db.execute_unprepared("UPDATE auth_invites SET id = id WHERE 0").await?;
            }
            _ => return Err(DbErr::Migration("unsupported invitation authority backend".into())),
        }
        let (shape, suffix) = match backend {
            DatabaseBackend::Postgres => ("token_hash ~ '^[0-9a-f]{64}$'", ""),
            _ => ("typeof(token_hash) = 'text' AND length(token_hash) = 64 AND token_hash NOT GLOB '*[^0-9a-f]*'", " WITHOUT ROWID"),
        };
        db.execute_unprepared(&format!(
            "CREATE TABLE auth_invite_digest_witnesses (
                kind TEXT NOT NULL CHECK (kind IN ('issued', 'consumed')),
                token_hash TEXT NOT NULL CHECK ({shape}),
                PRIMARY KEY (token_hash, kind)
             ){suffix};
             INSERT INTO auth_invite_digest_witnesses SELECT 'issued', token_hash FROM auth_invites;
             INSERT INTO auth_invite_digest_witnesses SELECT 'consumed', token_hash FROM auth_invite_consumptions;"
        )).await?;
        let days = crate::retention::AUTH_ARTIFACT_RETENTION_DAYS;
        match backend {
            DatabaseBackend::Postgres => {
                db.execute_unprepared(POSTGRES).await?;
                for table in ["auth_invites", "auth_invite_consumptions"] {
                    db.execute_unprepared(&format!(
                        "CREATE TRIGGER auth_invite_metadata_delete_guard BEFORE DELETE ON {table}
                         FOR EACH ROW EXECUTE FUNCTION auth_guard_invite_metadata_delete('{days}');
                         CREATE TRIGGER auth_invite_metadata_truncate_guard BEFORE TRUNCATE ON {table}
                         FOR EACH STATEMENT EXECUTE FUNCTION auth_guard_invite_metadata_delete('{days}');"
                    )).await?;
                }
            }
            DatabaseBackend::Sqlite => {
                db.execute_unprepared(SQLITE_WITNESS).await?;
                for (table, kind) in [("auth_invites", "issued"), ("auth_invite_consumptions", "consumed")] {
                    let duplicate = if kind == "issued" { "" } else { " AND kind = 'consumed'" };
                    let ledger = if kind == "consumed" {
                        " OR EXISTS (SELECT 1 FROM auth_invites WHERE token_hash = OLD.token_hash)"
                    } else { "" };
                    db.execute_unprepared(&format!(
                        "CREATE TRIGGER auth_invite_birth_guard_{kind} BEFORE INSERT ON {table}
                         WHEN typeof(NEW.token_hash) <> 'text' OR length(NEW.token_hash) <> 64
                            OR NEW.token_hash GLOB '*[^0-9a-f]*'
                            OR EXISTS (SELECT 1 FROM auth_invite_digest_witnesses WHERE token_hash = NEW.token_hash{duplicate})
                            OR EXISTS (SELECT 1 FROM {table} WHERE id = NEW.id OR rowid = NEW.rowid)
                         BEGIN SELECT RAISE(ABORT, 'Invite digest or row already registered'); END;
                         CREATE TRIGGER auth_invite_birth_claim_{kind} AFTER INSERT ON {table}
                         BEGIN INSERT INTO auth_invite_digest_witnesses (kind, token_hash)
                             VALUES ('{kind}', NEW.token_hash); END;
                         CREATE TRIGGER auth_invite_rowid_guard_{kind} BEFORE UPDATE ON {table}
                         WHEN NEW.rowid IS NOT OLD.rowid
                         BEGIN SELECT RAISE(ABORT, 'Invite row identity is immutable'); END;
                         CREATE TRIGGER auth_invite_metadata_delete_guard_{kind} BEFORE DELETE ON {table}
                         WHEN EXISTS (SELECT 1 FROM tenants WHERE id = OLD.tenant_id)
                           AND (julianday(OLD.expires_at) IS NULL
                             OR julianday(OLD.expires_at) >= julianday('now') - {days}{ledger})
                         BEGIN SELECT RAISE(ABORT, 'Invite metadata is still retained'); END;"
                    )).await?;
                }
            }
            _ => unreachable!("backend checked"),
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("permanent invitation authority cannot be discarded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection, DatabaseTransaction, TransactionTrait};

    const TENANT: &str = "10000000-0000-0000-0000-000000000001";
    const USER: &str = "20000000-0000-0000-0000-000000000001";
    const LIVE: &str = "30000000-0000-0000-0000-000000000001";
    const OLD: &str = "30000000-0000-0000-0000-000000000002";
    const FRESH: &str = "30000000-0000-0000-0000-000000000003";

    fn fixture() -> String {
        let live = "a".repeat(64);
        let old = "b".repeat(64);
        format!(
            "CREATE TABLE tenants (id UUID PRIMARY KEY NOT NULL);
             CREATE TABLE users (id UUID PRIMARY KEY NOT NULL, tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE);
             CREATE TABLE auth_invites (
                id UUID PRIMARY KEY NOT NULL, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                token_hash TEXT NOT NULL UNIQUE, email TEXT NOT NULL, role TEXT NOT NULL,
                issued_by UUID REFERENCES users(id) ON DELETE SET NULL,
                created_at TIMESTAMPTZ NOT NULL, expires_at TIMESTAMPTZ NOT NULL, revoked_at TIMESTAMPTZ,
                issuer_recovery_revision BIGINT, issuer_membership_revision BIGINT,
                role_grant_binding TEXT, module_policy_binding TEXT);
             CREATE TABLE auth_invite_consumptions (
                id UUID PRIMARY KEY NOT NULL, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                token_hash TEXT NOT NULL UNIQUE, email TEXT NOT NULL, role TEXT NOT NULL,
                user_id UUID REFERENCES users(id) ON DELETE SET NULL, expires_at TIMESTAMPTZ NOT NULL,
                consumed_at TIMESTAMPTZ NOT NULL);
             INSERT INTO tenants VALUES ('{TENANT}');
             INSERT INTO users VALUES ('{USER}', '{TENANT}');
             INSERT INTO auth_invites VALUES
                ('{LIVE}', '{TENANT}', '{live}', 'live@example.test', 'customer', '{USER}', '2000-01-01', '2099-01-01', '2000-01-02', 1, 2, 'grant', 'policy'),
                ('{OLD}', '{TENANT}', '{old}', 'old@example.test', 'customer', '{USER}', '2000-01-01', '2000-01-02', NULL, 1, 2, 'grant', 'policy');
             INSERT INTO auth_invite_consumptions VALUES
                ('{LIVE}', '{TENANT}', '{live}', 'live@example.test', 'customer', '{USER}', '2099-01-01', '2000-01-01'),
                ('{OLD}', '{TENANT}', '{old}', 'old@example.test', 'customer', '{USER}', '2000-01-02', '2000-01-01');"
        )
    }

    async fn tx(db: &DatabaseConnection, schema: Option<&str>) -> DatabaseTransaction {
        let tx = db.begin().await.unwrap();
        if let Some(schema) = schema {
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        }
        tx
    }

    async fn reject(db: &DatabaseConnection, schema: Option<&str>, sql: &str) {
        let tx = tx(db, schema).await;
        assert!(tx.execute_unprepared(sql).await.is_err(), "{sql}");
        tx.rollback().await.unwrap();
    }

    async fn exercise(db: &DatabaseConnection, schema: Option<&str>) {
        let old = "b".repeat(64);
        let fresh = "c".repeat(64);
        for sql in [
            format!("UPDATE auth_invites SET revoked_at = NULL WHERE id = '{LIVE}'"),
            format!("UPDATE auth_invites SET revoked_at = '2000-01-03' WHERE id = '{LIVE}'"),
            format!("UPDATE auth_invites SET issuer_recovery_revision = 3 WHERE id = '{LIVE}'"),
            format!("UPDATE auth_invites SET module_policy_binding = 'new' WHERE id = '{LIVE}'"),
            format!("UPDATE auth_invites SET issued_by = NULL WHERE id = '{LIVE}'"),
            format!("UPDATE auth_invite_consumptions SET expires_at = '2000-01-01' WHERE id = '{LIVE}'"),
            format!("DELETE FROM auth_invite_consumptions WHERE id = '{LIVE}'"),
            format!("DELETE FROM auth_invite_consumptions WHERE id = '{OLD}'"),
            format!("DELETE FROM auth_invites WHERE id = '{LIVE}'"),
            "DELETE FROM auth_invite_digest_witnesses".into(),
            "UPDATE auth_invite_digest_witnesses SET kind = 'issued'".into(),
        ] {
            reject(db, schema, &sql).await;
        }
        let tx1 = tx(db, schema).await;
        tx1.execute_unprepared(&format!("DELETE FROM auth_invites WHERE id = '{OLD}'; DELETE FROM auth_invite_consumptions WHERE id = '{OLD}';")).await.unwrap();
        tx1.commit().await.unwrap();
        reject(db, schema, &format!(
            "INSERT INTO auth_invites VALUES ('{FRESH}', '{TENANT}', '{old}', 'old@example.test', 'customer', '{USER}', '2000-01-01', '2099-01-01', NULL, 1, 2, 'grant', 'policy')"
        )).await;
        reject(db, schema, &format!(
            "INSERT INTO auth_invite_consumptions VALUES ('{FRESH}', '{TENANT}', '{old}', 'old@example.test', 'customer', NULL, '2099-01-01', '2000-01-01')"
        )).await;
        // Failed birth must not permanently consume a digest.
        let birth = format!("INSERT INTO auth_invites VALUES ('{FRESH}', '{TENANT}', '{fresh}', 'fresh@example.test', 'customer', '{USER}', '2000-01-01', '2099-01-01', NULL, 1, 2, 'grant', 'policy')");
        let tx1 = tx(db, schema).await;
        tx1.execute_unprepared(&birth).await.unwrap();
        tx1.rollback().await.unwrap();
        let tx1 = tx(db, schema).await;
        tx1.execute_unprepared(&birth).await.unwrap();
        tx1.execute_unprepared(&format!("DELETE FROM users WHERE id = '{USER}'")).await.unwrap();
        let row = tx1.query_one_raw(sea_orm_migration::sea_orm::Statement::from_string(
            db.get_database_backend(), "SELECT COUNT(*) AS count FROM auth_invites WHERE issued_by IS NULL",
        )).await.unwrap().unwrap();
        assert_eq!(row.try_get::<i64>("", "count").unwrap(), 2);
        tx1.execute_unprepared(&format!("DELETE FROM tenants WHERE id = '{TENANT}'")).await.unwrap();
        let row = tx1.query_one_raw(sea_orm_migration::sea_orm::Statement::from_string(
            db.get_database_backend(), "SELECT COUNT(*) AS count FROM auth_invite_digest_witnesses",
        )).await.unwrap().unwrap();
        assert_eq!(row.try_get::<i64>("", "count").unwrap(), 5);
        tx1.commit().await.unwrap();
    }

    #[tokio::test]
    async fn sqlite_invite_terminal_claims_survive_retention_and_cascade() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA foreign_keys = ON").await.unwrap();
        db.execute_unprepared(&fixture()).await.unwrap();
        let setup = db.begin().await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.commit().await.unwrap();
        reject(&db, None, &format!(
            "INSERT OR REPLACE INTO auth_invites (rowid,id,tenant_id,token_hash,email,role,created_at,expires_at)
             SELECT rowid,'{FRESH}',tenant_id,'{}',email,role,created_at,expires_at FROM auth_invites WHERE id = '{LIVE}'",
            "c".repeat(64),
        )).await;
        reject(&db, None, "INSERT OR REPLACE INTO auth_invite_digest_witnesses SELECT kind, token_hash FROM auth_invite_digest_witnesses").await;
        // SQLite outer conflict policies can override inner CHECK handling;
        // an explicit RAISE must reject the complete source statement.
        reject(&db, None, &format!(
            "INSERT OR IGNORE INTO auth_invites VALUES ('{FRESH}', '{TENANT}', 'malformed', 'fresh@example.test', 'customer', '{USER}', '2000-01-01', '2099-01-01', NULL, 1, 2, 'grant', 'policy')"
        )).await;
        reject(&db, None, &format!(
            "INSERT OR IGNORE INTO auth_invite_consumptions VALUES ('{FRESH}', '{TENANT}', 'malformed', 'fresh@example.test', 'customer', NULL, '2099-01-01', '2000-01-01')"
        )).await;
        reject(&db, None, "INSERT OR IGNORE INTO auth_invite_digest_witnesses VALUES ('issued', 'malformed')").await;
        exercise(&db, None).await;
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_AUTH_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_invite_terminal_claims_survive_retention_and_cascade() {
        let db = Database::connect(std::env::var("RUSTOK_AUTH_TEST_DATABASE_URL").unwrap()).await.unwrap();
        let schema = format!("invite_terminal_{}", uuid::Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = tx(&db, Some(&schema)).await;
        setup.execute_unprepared(&fixture()).await.unwrap();
        super::super::m20261128_000001_fence_invite_consumption_keys::Migration
            .up(&SchemaManager::new(&setup)).await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.commit().await.unwrap();
        for table in ["auth_invites", "auth_invite_consumptions", "auth_invite_digest_witnesses"] {
            reject(&db, Some(&schema), &format!("TRUNCATE {table}")).await;
        }
        // A concurrent deletion owns the old UUID even if a new writer
        // supplies a different digest. It must not wait at the unique index.
        let owner = tx(&db, Some(&schema)).await;
        owner.execute_unprepared(&format!("DELETE FROM auth_invites WHERE id = '{OLD}'")).await.unwrap();
        let writer = tx(&db, Some(&schema)).await;
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            writer.execute_unprepared(&format!(
                "INSERT INTO auth_invites VALUES ('{OLD}', '{TENANT}', '{}', 'fresh@example.test', 'customer', '{USER}', '2000-01-01', '2099-01-01', NULL, 1, 2, 'grant', 'policy')",
                "d".repeat(64),
            )),
        ).await.expect("issuance identity contention must not wait").unwrap_err();
        assert_eq!(
            rustok_core::resilience::retry::PostgresTransactionConflict::classify(DatabaseBackend::Postgres, &error),
            Some(rustok_core::resilience::retry::PostgresTransactionConflict::LockUnavailable),
        );
        writer.rollback().await.unwrap();
        owner.rollback().await.unwrap();
        exercise(&db, Some(&schema)).await;
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
