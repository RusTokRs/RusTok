//! Prevent late registry release and receipt FK checks from waiting on owner rows.
use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES_GUARDS: &str = r#"
CREATE FUNCTION registry_release_fence_request_parent() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    -- The platform guard runs first (trigger names sort lexically).
    -- A SET NULL cascade and an unchanged reference acquire no new FK parent.
    IF NEW.request_id IS NULL THEN RETURN NEW; END IF;
    IF TG_OP = 'UPDATE' THEN
        IF NEW.request_id IS NOT DISTINCT FROM OLD.request_id THEN RETURN NEW; END IF;
    END IF;
    LOCK TABLE registry_publish_requests IN ROW SHARE MODE NOWAIT;
    PERFORM 1 FROM registry_publish_requests WHERE id = NEW.request_id FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Registry release request parent missing or invisible' USING ERRCODE = '23503';
    END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'Registry release request parent fence busy' USING ERRCODE = '55P03';
END; $$;
CREATE TRIGGER registry_release_request_parent_fence
BEFORE INSERT OR UPDATE ON registry_module_releases
FOR EACH ROW EXECUTE FUNCTION registry_release_fence_request_parent();

CREATE FUNCTION registry_release_fence_receipt_parents() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    -- Publication owns request before release. Yank/unyank have only the
    -- release FK. Direct receipt DML may already hold a receipt row; do
    -- not wait for either earlier parent from this entry point.
    IF TG_TABLE_NAME = 'registry_publication_operations' THEN
        LOCK TABLE registry_publish_requests IN ROW SHARE MODE NOWAIT;
        PERFORM 1 FROM registry_publish_requests WHERE id = NEW.request_id FOR KEY SHARE NOWAIT;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'Registry publication receipt request parent missing or invisible' USING ERRCODE = '23503';
        END IF;
    END IF;
    LOCK TABLE registry_module_releases IN ROW SHARE MODE NOWAIT;
    PERFORM 1 FROM registry_module_releases WHERE id = NEW.release_id FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Registry release receipt parent missing or invisible' USING ERRCODE = '23503';
    END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'Registry release receipt parent fence busy' USING ERRCODE = '55P03';
END; $$;
CREATE TRIGGER registry_publication_receipt_parent_fence
BEFORE INSERT OR UPDATE ON registry_publication_operations
FOR EACH ROW EXECUTE FUNCTION registry_release_fence_receipt_parents();
CREATE TRIGGER registry_yank_receipt_parent_fence
BEFORE INSERT OR UPDATE ON registry_release_yank_operations
FOR EACH ROW EXECUTE FUNCTION registry_release_fence_receipt_parents();
CREATE TRIGGER registry_unyank_receipt_parent_fence
BEFORE INSERT OR UPDATE ON registry_release_unyank_operations
FOR EACH ROW EXECUTE FUNCTION registry_release_fence_receipt_parents();
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if SupportedBackend::new(manager.get_database_backend())? == SupportedBackend::Postgres {
            manager.get_connection().execute_unprepared(POSTGRES_GUARDS).await?;
        }
        // SQLite serializes writers; existing foreign keys remain authoritative.
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if SupportedBackend::new(manager.get_database_backend())? == SupportedBackend::Postgres {
            return Err(DbErr::Migration(
                "registry release FK lock order cannot be downgraded safely".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DatabaseConnection, DatabaseTransaction, DbBackend, TransactionTrait};
    use std::time::Duration;
    use uuid::Uuid;

    async fn transaction(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = crate::backend::begin_fenced_transaction(db).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx
    }

    async fn reject_busy(db: &DatabaseConnection, schema: &str, sql: &str, message: &str) {
        let tx = transaction(db, schema).await;
        let error = tokio::time::timeout(Duration::from_secs(3), tx.execute_unprepared(sql))
            .await.expect("late FK acquisition must not wait")
            .expect_err("contended parent must reject the statement");
        assert!(error.to_string().contains(message), "{error}");
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_registry_release_and_receipt_fk_fences_never_wait_for_busy_parents() {
        let db = Database::connect(std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let schema = format!("registry_fk_fence_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = transaction(&db, &schema).await;
        setup.execute_unprepared(
            "CREATE TABLE module_artifact_activation_locks (
                scope_kind TEXT NOT NULL, scope_tenant_key TEXT NOT NULL, slug TEXT NOT NULL,
                PRIMARY KEY (scope_kind, scope_tenant_key, slug));
             CREATE TABLE registry_publish_requests (id TEXT PRIMARY KEY);
             CREATE TABLE registry_module_releases (
                id TEXT PRIMARY KEY, request_id TEXT REFERENCES registry_publish_requests(id) ON DELETE SET NULL,
                slug TEXT NOT NULL, version TEXT NOT NULL, status TEXT NOT NULL, checksum_sha256 TEXT,
                UNIQUE (slug, version));
             CREATE TABLE registry_publication_operations (
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),
                release_id TEXT NOT NULL REFERENCES registry_module_releases(id));
             CREATE TABLE registry_release_yank_operations (
                id TEXT PRIMARY KEY, release_id TEXT NOT NULL REFERENCES registry_module_releases(id));
             CREATE TABLE registry_release_unyank_operations (
                id TEXT PRIMARY KEY, release_id TEXT NOT NULL REFERENCES registry_module_releases(id));
             INSERT INTO registry_publish_requests VALUES ('request-a'), ('request-b');",
        ).await.unwrap();
        super::super::m20261127_000096_registry_release_authority::Migration
            .up(&SchemaManager::new(&setup)).await.unwrap();
        super::super::m20261127_000098_registry_platform_lock_order::Migration
            .up(&SchemaManager::new(&setup)).await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.execute_unprepared(&format!(
            "INSERT INTO registry_module_releases VALUES ('release-a', 'request-a', '{schema}', '1.0.0', 'active', NULL);
             INSERT INTO registry_release_yank_operations VALUES ('yank-a', 'release-a')",
        )).await.unwrap();
        setup.commit().await.unwrap();

        let request_owner = transaction(&db, &schema).await;
        request_owner.execute_unprepared("SELECT 1 FROM registry_publish_requests WHERE id = 'request-b' FOR UPDATE").await.unwrap();
        let insert = format!("INSERT INTO registry_module_releases VALUES ('release-b', 'request-b', '{schema}', '2.0.0', 'active', NULL)");
        reject_busy(&db, &schema, &insert, "Registry release request parent fence busy").await;
        reject_busy(&db, &schema, "UPDATE registry_module_releases SET request_id = 'request-b' WHERE id = 'release-a'", "Registry release request parent fence busy").await;
        reject_busy(&db, &schema, "INSERT INTO registry_publication_operations VALUES ('publish-a', 'request-b', 'release-a')", "Registry release receipt parent fence busy").await;
        request_owner.rollback().await.unwrap();

        let release_owner = transaction(&db, &schema).await;
        release_owner.execute_unprepared("SELECT 1 FROM registry_module_releases WHERE id = 'release-a' FOR UPDATE").await.unwrap();
        for sql in [
            "INSERT INTO registry_publication_operations VALUES ('publish-a', 'request-a', 'release-a')",
            "INSERT INTO registry_release_yank_operations VALUES ('yank-b', 'release-a')",
            "INSERT INTO registry_release_unyank_operations VALUES ('unyank-a', 'release-a')",
            "UPDATE registry_release_yank_operations SET release_id = release_id WHERE id = 'yank-a'",
        ] {
            reject_busy(&db, &schema, sql, "Registry release receipt parent fence busy").await;
        }
        // The same parent lock is reusable by the canonical writer.
        release_owner.execute_unprepared("INSERT INTO registry_release_unyank_operations VALUES ('unyank-a', 'release-a')").await.unwrap();
        release_owner.commit().await.unwrap();

        let writer = transaction(&db, &schema).await;
        writer.execute_unprepared(&insert).await.unwrap();
        writer.execute_unprepared("INSERT INTO registry_publication_operations VALUES ('publish-a', 'request-a', 'release-a')").await.unwrap();
        writer.execute_unprepared("DELETE FROM registry_publish_requests WHERE id = 'request-b'").await.unwrap();
        let row = writer.query_one_raw(sea_orm::Statement::from_string(
            DbBackend::Postgres,
            "SELECT request_id, revision FROM registry_module_releases JOIN registry_release_authority_revisions USING (slug, version) WHERE id = 'release-b'",
        )).await.unwrap().unwrap();
        assert_eq!(row.try_get::<Option<String>>("", "request_id").unwrap(), None);
        // The SET NULL cascade is metadata-only, not a new release authority.
        assert_eq!(row.try_get::<i64>("", "revision").unwrap(), 1);
        writer.commit().await.unwrap();

        let missing = transaction(&db, &schema).await;
        let error = missing.execute_unprepared("UPDATE registry_module_releases SET request_id = 'missing' WHERE id = 'release-b'").await.unwrap_err();
        assert!(error.to_string().contains("Registry release request parent missing or invisible"), "{error}");
        missing.rollback().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
