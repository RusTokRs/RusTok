//! Permanent UUID witnesses for static RBAC authority identities.
//!
//! A successful birth consumes an identity for the lifetime of this database.
//! Deletion, cascade, and revision reset on a new row cannot resurrect it.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const OWNERS: &[&str] = &["users", "roles", "permissions"];

const POSTGRES_SETUP: &str = r#"
LOCK TABLE users, roles, permissions IN SHARE ROW EXCLUSIVE MODE;
DO $$
BEGIN
    IF row_security_active('users'::regclass) OR row_security_active('roles'::regclass)
       OR row_security_active('permissions'::regclass) THEN
        RAISE EXCEPTION 'RBAC identity backfill requires complete visibility' USING ERRCODE = '42501';
    END IF;
END $$;
CREATE TABLE rbac_identity_witnesses (
    kind TEXT NOT NULL CHECK (kind IN ('users', 'roles', 'permissions')),
    id UUID NOT NULL,
    PRIMARY KEY (kind, id)
);
INSERT INTO rbac_identity_witnesses (kind, id)
SELECT 'users', id FROM users UNION ALL
SELECT 'roles', id FROM roles UNION ALL
SELECT 'permissions', id FROM permissions;

CREATE FUNCTION rbac_guard_identity_witness() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'INSERT' THEN
        RAISE EXCEPTION 'RBAC identity witness is permanent' USING ERRCODE = '23514';
    END IF;
    -- A direct witness insert must participate in the same nonwaiting fence.
    -- A competing reservation must not block under an already-inserted parent.
    IF NOT pg_try_advisory_xact_lock(hashtextextended(
        'rustok:rbac-identity:' || NEW.kind || ':' || NEW.id::text, 0)) THEN
        RAISE EXCEPTION 'RBAC identity witness busy' USING ERRCODE = '55P03';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER rbac_identity_witness_guard
BEFORE INSERT OR UPDATE OR DELETE ON rbac_identity_witnesses
FOR EACH ROW EXECUTE FUNCTION rbac_guard_identity_witness();
CREATE TRIGGER rbac_identity_witness_no_truncate
BEFORE TRUNCATE ON rbac_identity_witnesses
FOR EACH STATEMENT EXECUTE FUNCTION rbac_guard_identity_witness();

CREATE FUNCTION rbac_claim_identity_witness() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    -- AFTER INSERT runs only for an actual birth, not ON CONFLICT DO NOTHING.
    -- No FK back to the parent and no global generation row.
    LOCK TABLE rbac_identity_witnesses IN ROW EXCLUSIVE MODE NOWAIT;
    IF row_security_active('rbac_identity_witnesses'::regclass) THEN
        RAISE EXCEPTION 'RBAC identity witness visibility incomplete' USING ERRCODE = '42501';
    END IF;
    INSERT INTO rbac_identity_witnesses (kind, id) VALUES (TG_TABLE_NAME, NEW.id);
    RETURN NULL;
END $$;

CREATE FUNCTION rbac_guard_authority_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.id IS DISTINCT FROM OLD.id THEN
        RAISE EXCEPTION 'RBAC authority UUID is immutable' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
"#;

const SQLITE_SETUP: &str = r#"
CREATE TABLE rbac_identity_witnesses (
    kind TEXT NOT NULL CHECK (kind IN ('users', 'roles', 'permissions')),
    id TEXT NOT NULL CHECK (length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'),
    PRIMARY KEY (kind, id)
);
"#;

const SQLITE_WITNESS_GUARDS: &str = r#"
CREATE TRIGGER rbac_identity_witness_insert_guard
BEFORE INSERT ON rbac_identity_witnesses
WHEN NEW.kind IS NULL OR NEW.kind NOT IN ('users', 'roles', 'permissions')
 OR NEW.id IS NULL OR typeof(NEW.id) <> 'text' OR length(NEW.id) <> 32
 OR NEW.id GLOB '*[^0-9a-f]*'
 OR EXISTS (SELECT 1 FROM rbac_identity_witnesses WHERE kind = NEW.kind AND id = NEW.id)
BEGIN
    -- Explicit ABORT cannot be overridden by the outer INSERT OR REPLACE
    -- or INSERT OR IGNORE conflict algorithm.
    SELECT RAISE(ABORT, 'RBAC authority UUID is invalid or already consumed');
END;
CREATE TRIGGER rbac_identity_witness_no_update BEFORE UPDATE ON rbac_identity_witnesses
BEGIN SELECT RAISE(ABORT, 'RBAC identity witness is permanent'); END;
CREATE TRIGGER rbac_identity_witness_no_delete BEFORE DELETE ON rbac_identity_witnesses
BEGIN SELECT RAISE(ABORT, 'RBAC identity witness is permanent'); END;
"#;

// SQLite can store UUIDs as text or a 16-byte blob. Collapse equivalent
// encodings into one permanent identity rather than keying by storage class.
// Invalid encodings fail the witness guard instead of allowing a new parent.
fn sqlite_uuid(column: &str) -> String {
    format!(
        "CASE typeof({column}) \
         WHEN 'blob' THEN lower(hex({column})) \
         WHEN 'text' THEN lower(replace({column}, '-', '')) END"
    )
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                db.execute_unprepared(POSTGRES_SETUP).await?;
                for table in OWNERS {
                    db.execute_unprepared(&format!(
                        "CREATE TRIGGER rbac_claim_identity AFTER INSERT ON {table}
                         FOR EACH ROW EXECUTE FUNCTION rbac_claim_identity_witness();
                         CREATE TRIGGER rbac_authority_id_guard BEFORE UPDATE OF id ON {table}
                         FOR EACH ROW EXECUTE FUNCTION rbac_guard_authority_identity();"
                    )).await?;
                }
            }
            DatabaseBackend::Sqlite => {
                // Freeze the SQLite writer before creating and seeding witnesses.
                db.execute_unprepared("UPDATE users SET id = id WHERE 0").await?;
                db.execute_unprepared(SQLITE_SETUP).await?;
                for table in OWNERS {
                    // Plain insert fails the migration on malformed or aliased
                    // legacy IDs. Never silently discard a conflicting witness.
                    db.execute_unprepared(&format!(
                        "INSERT INTO rbac_identity_witnesses (kind, id) SELECT '{table}', {} FROM {table}",
                        sqlite_uuid("id")
                    )).await?;
                }
                db.execute_unprepared(SQLITE_WITNESS_GUARDS).await?;
                for table in OWNERS {
                    db.execute_unprepared(&format!(
                        "CREATE TRIGGER rbac_{table}_claim_identity AFTER INSERT ON {table}
                         BEGIN
                           INSERT INTO rbac_identity_witnesses (kind, id) VALUES ('{table}', {});
                         END;
                         CREATE TRIGGER rbac_{table}_authority_id_guard BEFORE UPDATE OF id ON {table}
                         WHEN NEW.id IS NOT OLD.id
                         BEGIN SELECT RAISE(ABORT, 'RBAC authority UUID is immutable'); END;",
                        sqlite_uuid("NEW.id")
                    )).await?;
                }
            }
            _ => return Err(DbErr::Migration("unsupported RBAC identity backend".into())),
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("RBAC identity witnesses cannot be discarded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, Statement, TransactionTrait};

    const A: &str = "10000000-0000-0000-0000-000000000001";
    const B: &str = "10000000-0000-0000-0000-000000000002";
    const C: &str = "10000000-0000-0000-0000-000000000003";

    async fn setup(db: &impl ConnectionTrait) {
        db.execute_unprepared(&format!(
            "CREATE TABLE users (id UUID PRIMARY KEY NOT NULL, natural_key TEXT UNIQUE);
             CREATE TABLE roles (id UUID PRIMARY KEY NOT NULL, natural_key TEXT UNIQUE);
             CREATE TABLE permissions (id UUID PRIMARY KEY NOT NULL, natural_key TEXT UNIQUE);
             INSERT INTO users VALUES ('{A}', 'old');
             INSERT INTO roles VALUES ('{A}', 'old');
             INSERT INTO permissions VALUES ('{A}', 'old');"
        )).await.unwrap();
    }

    async fn witnesses(db: &impl ConnectionTrait) -> i64 {
        db.query_one_raw(Statement::from_string(db.get_database_backend(),
            "SELECT COUNT(*) AS count FROM rbac_identity_witnesses"))
            .await.unwrap().unwrap().try_get("", "count").unwrap()
    }

    #[tokio::test]
    async fn sqlite_identity_births_survive_delete_and_reject_replace_without_recursive_triggers() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA recursive_triggers = OFF").await.unwrap();
        setup(&db).await;
        let tx = db.begin().await.unwrap();
        Migration.up(&SchemaManager::new(&tx)).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(witnesses(&db).await, 3);
        for (index, table) in OWNERS.iter().enumerate() {
            // A natural-key conflict that actually does nothing consumes nothing.
            db.execute_unprepared(&format!(
                "INSERT INTO {table} VALUES ('{B}', 'old') ON CONFLICT(natural_key) DO NOTHING"
            )).await.unwrap();
            assert_eq!(witnesses(&db).await, 3 + index as i64);
            for sql in [
                format!("INSERT OR REPLACE INTO {table} VALUES ('{A}', 'new')"),
                format!("UPDATE {table} SET id = '{B}' WHERE id = '{A}'"),
            ] {
                assert!(db.execute_unprepared(&sql).await.is_err(), "{sql}");
            }
            db.execute_unprepared(&format!("DELETE FROM {table} WHERE id = '{A}'")).await.unwrap();
            for id in [A.to_owned(), A.replace('-', ""), A.to_uppercase()] {
                assert!(db.execute_unprepared(&format!("INSERT OR IGNORE INTO {table} VALUES ('{id}', 'old')")).await.is_err());
            }
            let simple = A.replace('-', "");
            assert!(db.execute_unprepared(&format!("INSERT INTO {table} VALUES (X'{simple}', 'old')")).await.is_err());
            let tx = db.begin().await.unwrap();
            tx.execute_unprepared(&format!("INSERT INTO {table} VALUES ('{B}', 'fresh')")).await.unwrap();
            tx.rollback().await.unwrap();
            db.execute_unprepared(&format!("INSERT INTO {table} VALUES ('{B}', 'fresh')")).await.unwrap();
        }
        assert_eq!(witnesses(&db).await, 6);
        for sql in [
            "DELETE FROM rbac_identity_witnesses".to_owned(),
            "UPDATE rbac_identity_witnesses SET id = id".to_owned(),
            format!("INSERT OR REPLACE INTO rbac_identity_witnesses VALUES ('users', '{}')", A.replace('-', "")),
            format!("INSERT OR IGNORE INTO users VALUES ('not-a-uuid', 'invalid')"),
        ] {
            assert!(db.execute_unprepared(&sql).await.is_err(), "{sql}");
        }
        assert_eq!(witnesses(&db).await, 6);
        db.execute_unprepared(&format!("INSERT INTO users VALUES ('{C}', 'fresh-2')")).await.unwrap();
        assert_eq!(witnesses(&db).await, 7);
    }

    #[tokio::test]
    async fn sqlite_cascades_cannot_discard_identity_or_revive_old_revisions() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA foreign_keys = ON").await.unwrap();
        db.execute_unprepared("PRAGMA recursive_triggers = OFF").await.unwrap();
        let tenant = "20000000-0000-0000-0000-000000000001";
        let new_tenant = "20000000-0000-0000-0000-000000000002";
        db.execute_unprepared(&format!(
            "CREATE TABLE tenants (id UUID PRIMARY KEY NOT NULL);
             CREATE TABLE users (id UUID PRIMARY KEY NOT NULL, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE);
             CREATE TABLE roles (id UUID PRIMARY KEY NOT NULL, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE, slug TEXT NOT NULL, is_system BOOLEAN NOT NULL);
             CREATE TABLE permissions (id UUID PRIMARY KEY NOT NULL, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE, resource TEXT NOT NULL, action TEXT NOT NULL);
             CREATE TABLE user_roles (id UUID PRIMARY KEY NOT NULL, user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE, role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE);
             CREATE TABLE role_permissions (id UUID PRIMARY KEY NOT NULL, role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE, permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE);
             INSERT INTO tenants VALUES ('{tenant}'), ('{new_tenant}');
             INSERT INTO users VALUES ('{A}', '{tenant}');
             INSERT INTO roles VALUES ('{B}', '{tenant}', 'manager', TRUE);
             INSERT INTO permissions VALUES ('{C}', '{tenant}', 'users', 'read');"
        )).await.unwrap();
        let tx = db.begin().await.unwrap();
        let manager = SchemaManager::new(&tx);
        crate::m20261127_000001_create_rbac_membership_revision::Migration.up(&manager).await.unwrap();
        crate::m20261127_000003_create_rbac_role_grant_revision::Migration.up(&manager).await.unwrap();
        Migration.up(&manager).await.unwrap();
        tx.execute_unprepared(&format!(
            "INSERT INTO user_roles VALUES ('{A}', '{A}', '{B}');
             INSERT INTO role_permissions VALUES ('{B}', '{B}', '{C}');"
        )).await.unwrap();
        tx.commit().await.unwrap();

        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("DELETE FROM tenants WHERE id = '{tenant}'")).await.unwrap();
        assert_eq!(witnesses(&tx).await, 3);
        tx.rollback().await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("DELETE FROM tenants WHERE id = '{tenant}'")).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(witnesses(&db).await, 3);
        for table in ["users", "roles", "permissions", "user_roles", "role_permissions"] {
            let row = db.query_one_raw(Statement::from_string(DatabaseBackend::Sqlite,
                format!("SELECT COUNT(*) AS count FROM {table}"))).await.unwrap().unwrap();
            assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
        }
        // Recreating the same authority UUID in another tenant is also denied.
        for sql in [
            format!("INSERT INTO users (id, tenant_id, rbac_membership_revision) VALUES ('{A}', '{new_tenant}', 1)"),
            format!("INSERT INTO roles (id, tenant_id, slug, is_system, grant_revision) VALUES ('{B}', '{new_tenant}', 'manager', TRUE, 1)"),
            format!("INSERT INTO permissions VALUES ('{C}', '{new_tenant}', 'users', 'read')"),
        ] {
            assert!(db.execute_unprepared(&sql).await.is_err(), "{sql}");
        }
        assert_eq!(witnesses(&db).await, 3);
    }

    #[tokio::test]
    async fn sqlite_backfill_rejects_aliases_and_rolls_back() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        setup(&db).await;
        db.execute_unprepared(&format!("INSERT INTO users VALUES ('{}', 'alias')", A.replace('-', ""))).await.unwrap();
        let tx = db.begin().await.unwrap();
        assert!(Migration.up(&SchemaManager::new(&tx)).await.is_err());
        tx.rollback().await.unwrap();
        assert!(!SchemaManager::new(&db).has_table("rbac_identity_witnesses").await.unwrap());
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_RBAC_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_identity_witness_is_permanent_and_nonwaiting() {
        use std::time::Duration;
        let db = Database::connect(std::env::var("RUSTOK_RBAC_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("rbac_identity_{}", uuid::Uuid::new_v4().simple());
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("CREATE SCHEMA {schema}; SET LOCAL search_path TO {schema}")).await.unwrap();
        setup(&tx).await;
        Migration.up(&SchemaManager::new(&tx)).await.unwrap();
        assert_eq!(witnesses(&tx).await, 3);
        tx.commit().await.unwrap();

        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        for table in OWNERS {
            tx.execute_unprepared(&format!("DELETE FROM {table} WHERE id = '{A}'")).await.unwrap();
        }
        tx.commit().await.unwrap();
        for table in OWNERS {
            let tx = db.begin().await.unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
            assert!(tx.execute_unprepared(&format!("INSERT INTO {table} VALUES ('{A}', 'old')")).await.is_err());
            tx.rollback().await.unwrap();
        }

        let holder = db.begin().await.unwrap();
        holder.execute_unprepared(&format!("SET LOCAL search_path TO {schema}; INSERT INTO rbac_identity_witnesses VALUES ('users', '{B}')")).await.unwrap();
        let contender = db.begin().await.unwrap();
        contender.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        let result = tokio::time::timeout(Duration::from_secs(3),
            contender.execute_unprepared(&format!("INSERT INTO users VALUES ('{B}', 'new')")))
            .await.expect("late witness must not wait");
        let error = result.expect_err("competing witness must reject the birth");
        assert!(error.to_string().contains("RBAC identity witness busy"), "{error}");
        contender.rollback().await.unwrap();
        holder.rollback().await.unwrap();

        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}; INSERT INTO users VALUES ('{B}', 'new')")).await.unwrap();
        tx.execute_unprepared(&format!("INSERT INTO users VALUES ('{C}', 'new') ON CONFLICT(natural_key) DO NOTHING")).await.unwrap();
        assert_eq!(witnesses(&tx).await, 4);
        tx.commit().await.unwrap();
        for sql in ["DELETE FROM rbac_identity_witnesses", "UPDATE rbac_identity_witnesses SET id = id", "TRUNCATE rbac_identity_witnesses"] {
            let tx = db.begin().await.unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
            assert!(tx.execute_unprepared(sql).await.is_err(), "{sql}");
            tx.rollback().await.unwrap();
        }
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
