//! Preserve the tenant namespace across hard deletion and cleanup.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
LOCK TABLE tenants IN SHARE ROW EXCLUSIVE MODE;
DO $$ BEGIN
    IF row_security_active('tenants'::regclass) THEN
        RAISE EXCEPTION 'Tenant UUID backfill requires complete visibility' USING ERRCODE = '42501';
    END IF;
END; $$;
CREATE TABLE tenant_uuid_witnesses (id UUID PRIMARY KEY);
INSERT INTO tenant_uuid_witnesses SELECT id FROM tenants;

CREATE FUNCTION tenant_guard_uuid_witness() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'INSERT' THEN
        RAISE EXCEPTION 'Tenant UUID witness is permanent' USING ERRCODE = '23514';
    END IF;
    IF NOT pg_try_advisory_xact_lock(hashtextextended('rustok:tenant-uuid:' || NEW.id::text, 0)) THEN
        RAISE EXCEPTION 'Tenant UUID witness busy' USING ERRCODE = '55P03';
    END IF;
    RETURN NEW;
END; $$;
CREATE TRIGGER tenant_uuid_witness_guard BEFORE INSERT OR UPDATE OR DELETE ON tenant_uuid_witnesses
FOR EACH ROW EXECUTE FUNCTION tenant_guard_uuid_witness();
CREATE TRIGGER tenant_uuid_witness_no_truncate BEFORE TRUNCATE ON tenant_uuid_witnesses
FOR EACH STATEMENT EXECUTE FUNCTION tenant_guard_uuid_witness();

CREATE FUNCTION tenant_claim_uuid_witness() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    LOCK TABLE tenant_uuid_witnesses IN ROW EXCLUSIVE MODE NOWAIT;
    IF row_security_active('tenant_uuid_witnesses'::regclass) THEN
        RAISE EXCEPTION 'Tenant UUID witness visibility incomplete' USING ERRCODE = '42501';
    END IF;
    INSERT INTO tenant_uuid_witnesses VALUES (NEW.id);
    RETURN NULL;
END; $$;
CREATE TRIGGER tenant_uuid_claim AFTER INSERT ON tenants
FOR EACH ROW EXECUTE FUNCTION tenant_claim_uuid_witness();

CREATE FUNCTION tenant_guard_uuid() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.id IS DISTINCT FROM OLD.id THEN
        RAISE EXCEPTION 'Tenant UUID is immutable' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END; $$;
CREATE TRIGGER tenant_uuid_immutable BEFORE UPDATE ON tenants
FOR EACH ROW EXECUTE FUNCTION tenant_guard_uuid();
"#;

fn sqlite_uuid(column: &str) -> String {
    format!(
        "CASE typeof({column}) WHEN 'blob' THEN lower(hex({column})) \
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
            DatabaseBackend::Postgres => { db.execute_unprepared(POSTGRES).await?; }
            DatabaseBackend::Sqlite => {
                db.execute_unprepared("UPDATE tenants SET id = id WHERE 0").await?;
                db.execute_unprepared(&format!(
                    "CREATE TABLE tenant_uuid_witnesses (
                        id TEXT PRIMARY KEY NOT NULL CHECK (length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*')
                     );
                     INSERT INTO tenant_uuid_witnesses SELECT {} FROM tenants;
                     CREATE TRIGGER tenant_uuid_witness_insert_guard BEFORE INSERT ON tenant_uuid_witnesses
                     WHEN NEW.id IS NULL OR typeof(NEW.id) <> 'text' OR length(NEW.id) <> 32
                       OR NEW.id GLOB '*[^0-9a-f]*'
                       OR EXISTS (SELECT 1 FROM tenant_uuid_witnesses WHERE id = NEW.id)
                     BEGIN SELECT RAISE(ABORT, 'Tenant UUID invalid or already consumed'); END;
                     CREATE TRIGGER tenant_uuid_witness_no_update BEFORE UPDATE ON tenant_uuid_witnesses
                     BEGIN SELECT RAISE(ABORT, 'Tenant UUID witness is permanent'); END;
                     CREATE TRIGGER tenant_uuid_witness_no_delete BEFORE DELETE ON tenant_uuid_witnesses
                     BEGIN SELECT RAISE(ABORT, 'Tenant UUID witness is permanent'); END;
                     CREATE TRIGGER tenant_uuid_claim AFTER INSERT ON tenants
                     BEGIN INSERT INTO tenant_uuid_witnesses VALUES ({}); END;
                     CREATE TRIGGER tenant_uuid_immutable BEFORE UPDATE ON tenants
                     WHEN NEW.id IS NOT OLD.id
                     BEGIN SELECT RAISE(ABORT, 'Tenant UUID is immutable'); END;
                     -- RAISE(IGNORE) preserves idempotent natural-key conflicts
                     -- while stopping REPLACE before its implicit parent DELETE.
                     CREATE TRIGGER tenant_natural_identity_insert_guard BEFORE INSERT ON tenants
                     WHEN EXISTS (SELECT 1 FROM tenants WHERE slug = NEW.slug)
                       OR (NEW.domain IS NOT NULL AND EXISTS (
                           SELECT 1 FROM tenants WHERE domain = NEW.domain))
                     BEGIN SELECT RAISE(IGNORE); END;",
                    sqlite_uuid("id"), sqlite_uuid("NEW.id")
                )).await?;
            }
            _ => return Err(DbErr::Migration("unsupported tenant UUID witness backend".into())),
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("Tenant UUID witnesses cannot be discarded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, Statement, TransactionTrait};

    const A: &str = "10000000-0000-0000-0000-000000000001";
    const B: &str = "10000000-0000-0000-0000-000000000002";

    async fn fixture(db: &impl ConnectionTrait) {
        db.execute_unprepared(&format!(
            "CREATE TABLE tenants (id UUID PRIMARY KEY NOT NULL, slug TEXT NOT NULL UNIQUE, domain TEXT UNIQUE);
             CREATE TABLE tenant_children (id TEXT PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE);
             INSERT INTO tenants VALUES ('{A}', 'old', 'old.example.test');
             INSERT INTO tenant_children VALUES ('child', '{A}');"
        )).await.unwrap();
    }

    async fn count(db: &impl ConnectionTrait, table: &str) -> i64 {
        db.query_one_raw(Statement::from_string(db.get_database_backend(),
            format!("SELECT COUNT(*) AS count FROM {table}")))
            .await.unwrap().unwrap().try_get("", "count").unwrap()
    }

    #[tokio::test]
    async fn sqlite_tenant_uuid_survives_cascade_and_conflict_noop() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA foreign_keys = ON; PRAGMA recursive_triggers = OFF").await.unwrap();
        fixture(&db).await;
        let tx = db.begin().await.unwrap();
        Migration.up(&SchemaManager::new(&tx)).await.unwrap();
        tx.commit().await.unwrap();
        db.execute_unprepared(&format!(
            "INSERT INTO tenants VALUES ('{B}', 'old', 'new.example.test') ON CONFLICT(slug) DO NOTHING"
        )).await.unwrap();
        assert_eq!(count(&db, "tenant_uuid_witnesses").await, 1);
        for sql in [
            format!("UPDATE tenants SET id = '{B}'"),
            format!("INSERT OR REPLACE INTO tenants VALUES ('{A}', 'replacement', 'replacement.example.test')"),
        ] {
            assert!(db.execute_unprepared(&sql).await.is_err(), "{sql}");
        }
        // A fresh UUID cannot replace a tenant through either natural key.
        for sql in [
            format!("INSERT OR REPLACE INTO tenants VALUES ('{B}', 'old', 'new.example.test')"),
            format!("INSERT OR REPLACE INTO tenants VALUES ('{B}', 'replacement', 'old.example.test')"),
        ] {
            db.execute_unprepared(&sql).await.unwrap();
        }
        let row = db.query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT id, slug, domain FROM tenants",
        )).await.unwrap().unwrap();
        assert_eq!(row.try_get::<String>("", "id").unwrap(), A);
        assert_eq!(row.try_get::<String>("", "slug").unwrap(), "old");
        assert_eq!(row.try_get::<String>("", "domain").unwrap(), "old.example.test");
        assert_eq!(count(&db, "tenant_children").await, 1);
        assert_eq!(count(&db, "tenant_uuid_witnesses").await, 1);
        db.execute_unprepared("DELETE FROM tenants").await.unwrap();
        assert_eq!(count(&db, "tenant_children").await, 0);
        assert_eq!(count(&db, "tenant_uuid_witnesses").await, 1);
        for id in [A.to_owned(), A.replace('-', "")] {
            assert!(db.execute_unprepared(&format!("INSERT OR IGNORE INTO tenants VALUES ('{id}', 'old', 'old.example.test')")).await.is_err());
        }
        assert!(db.execute_unprepared(&format!("INSERT INTO tenants VALUES (X'{}', 'old', NULL)", A.replace('-', ""))).await.is_err());
        for sql in ["DELETE FROM tenant_uuid_witnesses", "UPDATE tenant_uuid_witnesses SET id = id"] {
            assert!(db.execute_unprepared(sql).await.is_err());
        }
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("INSERT INTO tenants VALUES ('{B}', 'old', NULL)")).await.unwrap();
        tx.rollback().await.unwrap();
        db.execute_unprepared(&format!("INSERT INTO tenants VALUES ('{B}', 'old', NULL)")).await.unwrap();
        assert_eq!(count(&db, "tenant_uuid_witnesses").await, 2);
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_TENANT_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_tenant_uuid_witness_rejects_rebirth_and_truncate() {
        let db = Database::connect(std::env::var("RUSTOK_TENANT_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("tenant_uuid_{}", uuid::Uuid::new_v4().simple());
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("CREATE SCHEMA {schema}; SET LOCAL search_path TO {schema}")).await.unwrap();
        fixture(&tx).await;
        Migration.up(&SchemaManager::new(&tx)).await.unwrap();
        tx.execute_unprepared("DELETE FROM tenants").await.unwrap();
        assert_eq!(count(&tx, "tenant_children").await, 0);
        assert_eq!(count(&tx, "tenant_uuid_witnesses").await, 1);
        tx.commit().await.unwrap();
        for sql in [
            format!("INSERT INTO tenants VALUES ('{A}', 'old', 'old.example.test')"),
            "TRUNCATE tenant_uuid_witnesses".to_owned(),
        ] {
            let tx = db.begin().await.unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
            assert!(tx.execute_unprepared(&sql).await.is_err(), "{sql}");
            tx.rollback().await.unwrap();
        }
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
