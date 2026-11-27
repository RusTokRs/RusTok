//! Protect permanent registry continuity witnesses against direct SQL reset and late waits.
use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
CREATE FUNCTION registry_release_guard_authority_witness() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' OR TG_OP = 'TRUNCATE' THEN
        RAISE EXCEPTION 'Registry authority witness cannot be removed' USING ERRCODE = '23514';
    END IF;
    IF TG_OP = 'UPDATE' THEN
        IF NEW.slug IS DISTINCT FROM OLD.slug OR NEW.version IS DISTINCT FROM OLD.version
           OR OLD.revision = 9223372036854775807 THEN
            RAISE EXCEPTION 'Registry authority witness identity or revision is immutable at exhaustion' USING ERRCODE = '23514';
        END IF;
        IF NEW.revision IS DISTINCT FROM OLD.revision + 1 THEN
            RAISE EXCEPTION 'Registry authority witness must advance by one' USING ERRCODE = '23514';
        END IF;
    ELSIF NEW.revision IS DISTINCT FROM 1 THEN
        RAISE EXCEPTION 'Registry authority witness must begin at one' USING ERRCODE = '23514';
    END IF;
    -- Raw witness DML may already own its row. Never wait for the slug gate.
    -- Release-trigger advancement already owns this gate, so the try is reentrant.
    IF NOT pg_try_advisory_xact_lock(hashtextextended('rustok:module-artifact-platform:' || NEW.slug, 0)) THEN
        RAISE EXCEPTION 'Registry authority witness platform fence busy' USING ERRCODE = '55P03';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER registry_release_authority_witness_guard
BEFORE INSERT OR UPDATE OR DELETE ON registry_release_authority_revisions
FOR EACH ROW EXECUTE FUNCTION registry_release_guard_authority_witness();
CREATE TRIGGER registry_release_authority_witness_no_truncate
BEFORE TRUNCATE ON registry_release_authority_revisions
FOR EACH STATEMENT EXECUTE FUNCTION registry_release_guard_authority_witness();

CREATE OR REPLACE FUNCTION registry_release_guard_platform_fence() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE target_slug TEXT;
BEGIN
    IF TG_OP = 'TRUNCATE' THEN
        RAISE EXCEPTION 'Registry releases cannot be truncated' USING ERRCODE = '0A000';
    END IF;
    IF TG_OP = 'UPDATE' THEN
        IF NEW.slug IS DISTINCT FROM OLD.slug OR NEW.version IS DISTINCT FROM OLD.version
           OR NEW.id IS DISTINCT FROM OLD.id THEN
            RAISE EXCEPTION 'Registry release identity is immutable' USING ERRCODE = '23514';
        END IF;
    END IF;
    IF TG_OP = 'DELETE' THEN target_slug := OLD.slug;
    ELSE target_slug := NEW.slug; END IF;
    IF NOT pg_try_advisory_xact_lock(hashtextextended('rustok:module-artifact-platform:' || target_slug, 0)) THEN
        RAISE EXCEPTION 'Registry release platform fence busy' USING ERRCODE = '55P03';
    END IF;
    -- Row-level NOWAIT alone does not cover the implicit table locks.
    -- ROW EXCLUSIVE covers both the lookup and a possible first insertion.
    LOCK TABLE module_artifact_activation_locks IN ROW EXCLUSIVE MODE NOWAIT;
    PERFORM 1 FROM module_artifact_activation_locks
      WHERE scope_kind = 'platform' AND scope_tenant_key = 'platform' AND slug = target_slug
      FOR UPDATE NOWAIT;
    IF NOT FOUND THEN
        INSERT INTO module_artifact_activation_locks (scope_kind, scope_tenant_key, slug)
          VALUES ('platform', 'platform', target_slug);
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'Registry release platform fence busy' USING ERRCODE = '55P03';
END $$;

CREATE OR REPLACE FUNCTION registry_release_advance_authority() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE target_slug TEXT; target_version TEXT; current_revision BIGINT;
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF NEW.status IS NOT DISTINCT FROM OLD.status AND
           NEW.checksum_sha256 IS NOT DISTINCT FROM OLD.checksum_sha256 THEN
            RETURN NEW;
        END IF;
    END IF;
    IF TG_OP = 'DELETE' THEN
        target_slug := OLD.slug; target_version := OLD.version;
    ELSE
        target_slug := NEW.slug; target_version := NEW.version;
    END IF;
    -- The earlier slug gate is held by the release BEFORE trigger.
    -- A maintenance reader may independently hold a witness row or table.
    -- Reject rather than wait while holding a release row.
    LOCK TABLE registry_release_authority_revisions IN ROW EXCLUSIVE MODE NOWAIT;
    SELECT revision INTO current_revision FROM registry_release_authority_revisions
      WHERE slug = target_slug AND version = target_version FOR UPDATE NOWAIT;
    IF FOUND THEN
        IF current_revision = 9223372036854775807 THEN
            RAISE EXCEPTION 'Registry release authority exhausted' USING ERRCODE = '23514';
        END IF;
        UPDATE registry_release_authority_revisions SET revision = current_revision + 1
          WHERE slug = target_slug AND version = target_version;
    ELSE
        INSERT INTO registry_release_authority_revisions (slug, version, revision)
          VALUES (target_slug, target_version, 1);
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'Registry release authority witness busy' USING ERRCODE = '55P03';
END $$;
"#;

const SQLITE_GUARDS: &str = r#"
-- SQLite REPLACE can delete conflicting rows without DELETE triggers even
-- when the outer statement is an INSERT or UPDATE. Publication uses a plain
-- INSERT; changes to an existing release use UPDATE, never replacement.
CREATE TRIGGER registry_release_no_replace
BEFORE INSERT ON registry_module_releases
WHEN EXISTS (SELECT 1 FROM registry_module_releases
             WHERE id = NEW.id OR (slug = NEW.slug AND version = NEW.version)
                OR rowid = NEW.rowid)
BEGIN SELECT RAISE(ABORT, 'Registry release cannot be replaced'); END;

CREATE TRIGGER registry_release_rowid_guard
BEFORE UPDATE ON registry_module_releases
WHEN NEW.rowid IS NOT OLD.rowid
BEGIN SELECT RAISE(ABORT, 'Registry release row identity is immutable'); END;

CREATE TRIGGER registry_release_authority_witness_insert
BEFORE INSERT ON registry_release_authority_revisions
WHEN typeof(NEW.revision) <> 'integer' OR NEW.revision <> 1
 OR EXISTS (SELECT 1 FROM registry_release_authority_revisions
             WHERE (slug = NEW.slug AND version = NEW.version) OR rowid = NEW.rowid)
BEGIN SELECT RAISE(ABORT, 'Registry authority witness must begin at one and cannot be replaced'); END;

CREATE TRIGGER registry_release_authority_witness_update
BEFORE UPDATE ON registry_release_authority_revisions
WHEN NEW.slug IS NOT OLD.slug OR NEW.version IS NOT OLD.version
 OR NEW.rowid IS NOT OLD.rowid
 OR typeof(OLD.revision) <> 'integer' OR typeof(NEW.revision) <> 'integer'
 OR OLD.revision = 9223372036854775807 OR NEW.revision <> OLD.revision + 1
BEGIN SELECT RAISE(ABORT, 'Registry authority witness must advance by one without changing identity'); END;

CREATE TRIGGER registry_release_authority_witness_delete
BEFORE DELETE ON registry_release_authority_revisions
BEGIN SELECT RAISE(ABORT, 'Registry authority witness cannot be removed'); END;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match SupportedBackend::new(manager.get_database_backend())? {
            SupportedBackend::Postgres => {
                db.execute_unprepared(POSTGRES).await?;
            }
            SupportedBackend::Sqlite => {
                // Existing SQLite INTEGER affinity alone permits REAL storage.
                // Never silently repair a potentially rolled-back authority witness.
                let invalid = db.query_one_raw(Statement::from_string(
                    sea_orm::DbBackend::Sqlite,
                    "SELECT 1 AS invalid FROM registry_release_authority_revisions
                     WHERE typeof(revision) <> 'integer' OR revision < 1 LIMIT 1",
                )).await?;
                if invalid.is_some() {
                    return Err(DbErr::Migration("invalid registry authority witness storage".into()));
                }
                db.execute_unprepared(SQLITE_GUARDS).await?;
                for (event, row, predicate) in [
                    ("INSERT", "NEW", ""),
                    ("UPDATE", "NEW", "WHEN NEW.status IS NOT OLD.status OR NEW.checksum_sha256 IS NOT OLD.checksum_sha256"),
                    ("DELETE", "OLD", ""),
                ] {
                    db.execute_unprepared(&format!(
                        "DROP TRIGGER registry_release_authority_{event};
                         CREATE TRIGGER registry_release_authority_{event}
                         AFTER {event} ON registry_module_releases {predicate}
                         BEGIN
                           INSERT INTO module_artifact_activation_locks (scope_kind, scope_tenant_key, slug)
                             VALUES ('platform', 'platform', {row}.slug)
                             ON CONFLICT (scope_kind, scope_tenant_key, slug) DO UPDATE SET slug = EXCLUDED.slug;
                           SELECT RAISE(ABORT, 'Registry release authority exhausted')
                             WHERE EXISTS (SELECT 1 FROM registry_release_authority_revisions
                               WHERE slug = {row}.slug AND version = {row}.version AND revision = 9223372036854775807);
                           UPDATE registry_release_authority_revisions SET revision = revision + 1
                             WHERE slug = {row}.slug AND version = {row}.version;
                           INSERT INTO registry_release_authority_revisions (slug, version, revision)
                             SELECT {row}.slug, {row}.version, 1
                             WHERE NOT EXISTS (SELECT 1 FROM registry_release_authority_revisions
                               WHERE slug = {row}.slug AND version = {row}.version);
                         END"
                    )).await?;
                }
            }
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("registry authority witness integrity cannot be downgraded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DatabaseConnection, DatabaseTransaction, DbBackend, TransactionTrait};
    use std::time::Duration;

    const SCHEMA: &str = "
        CREATE TABLE module_artifact_activation_locks (
            scope_kind TEXT NOT NULL, scope_tenant_key TEXT NOT NULL, slug TEXT NOT NULL,
            PRIMARY KEY (scope_kind, scope_tenant_key, slug));
        CREATE TABLE registry_module_releases (
            id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL,
            status TEXT NOT NULL, checksum_sha256 TEXT, metadata TEXT, UNIQUE (slug, version));";
    const RELEASE: &str = "INSERT INTO registry_module_releases
        VALUES ('release', 'witness_sample', '1.0.0', 'active', 'digest', NULL)";
    const WITNESS: &str = "SELECT revision FROM registry_release_authority_revisions
        WHERE slug = 'witness_sample' AND version = '1.0.0'";

    async fn setup(manager: &SchemaManager<'_>) {
        manager.get_connection().execute_unprepared(SCHEMA).await.unwrap();
        super::super::m20261127_000096_registry_release_authority::Migration
            .up(manager).await.unwrap();
        super::super::m20261127_000098_registry_platform_lock_order::Migration
            .up(manager).await.unwrap();
        manager.get_connection().execute_unprepared(RELEASE).await.unwrap();
        Migration.up(manager).await.unwrap();
    }

    async fn revision(db: &impl ConnectionTrait) -> i64 {
        db.query_one_raw(Statement::from_string(db.get_database_backend(), WITNESS))
            .await.unwrap().unwrap().try_get("", "revision").unwrap()
    }

    #[tokio::test]
    async fn sqlite_witness_cannot_be_reset_removed_replaced_or_changed_to_real() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        setup(&SchemaManager::new(&db)).await;
        // SQLite REPLACE otherwise deletes a conflicting row without firing
        // DELETE triggers when recursive_triggers is disabled.
        db.execute_unprepared("PRAGMA recursive_triggers = OFF").await.unwrap();
        db.execute_unprepared("DELETE FROM registry_module_releases").await.unwrap();
        assert_eq!(revision(&db).await, 2);
        for sql in [
            "DELETE FROM registry_release_authority_revisions",
            "UPDATE registry_release_authority_revisions SET revision = 1",
            "UPDATE registry_release_authority_revisions SET revision = 2.5",
            "UPDATE registry_release_authority_revisions SET slug = 'another'",
            "INSERT OR REPLACE INTO registry_release_authority_revisions VALUES ('witness_sample', '1.0.0', 1)",
            "INSERT OR REPLACE INTO registry_release_authority_revisions (rowid, slug, version, revision)
             SELECT rowid, 'different', '2.0.0', 1 FROM registry_release_authority_revisions",
            "UPDATE registry_release_authority_revisions SET rowid = rowid + 100, revision = revision + 1",
        ] {
            assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
            assert_eq!(revision(&db).await, 2);
        }
        db.execute_unprepared(RELEASE).await.unwrap();
        assert_eq!(revision(&db).await, 3);
        db.execute_unprepared("UPDATE registry_module_releases SET metadata = 'changed'").await.unwrap();
        assert_eq!(revision(&db).await, 3);
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("UPDATE registry_module_releases SET status = 'yanked'").await.unwrap();
        assert_eq!(revision(&tx).await, 4);
        tx.rollback().await.unwrap();
        assert_eq!(revision(&db).await, 3);
    }

    #[tokio::test]
    async fn sqlite_release_replace_cannot_displace_another_release_or_its_receipts() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        setup(&SchemaManager::new(&db)).await;
        db.execute_unprepared("PRAGMA recursive_triggers = OFF").await.unwrap();
        db.execute_unprepared("PRAGMA foreign_keys = ON").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE release_receipts (
                id TEXT PRIMARY KEY, release_id TEXT NOT NULL
                REFERENCES registry_module_releases(id) ON DELETE CASCADE);
             INSERT INTO release_receipts VALUES ('receipt', 'release');
             INSERT INTO registry_module_releases
             VALUES ('other', 'other_sample', '2.0.0', 'active', 'other_digest', NULL);"
        ).await.unwrap();

        for sql in [
            "INSERT OR REPLACE INTO registry_module_releases
             VALUES ('release', 'different', '3.0.0', 'active', 'digest', NULL)",
            "INSERT OR REPLACE INTO registry_module_releases
             VALUES ('different', 'witness_sample', '1.0.0', 'active', 'digest', NULL)",
            "INSERT OR REPLACE INTO registry_module_releases
             VALUES ('release', 'witness_sample', '1.0.0', 'active', 'digest', NULL)",
            "INSERT OR REPLACE INTO registry_module_releases (rowid, id, slug, version, status, checksum_sha256)
             SELECT rowid, 'different', 'different', '3.0.0', 'active', 'digest'
             FROM registry_module_releases WHERE id = 'release'",
            "UPDATE OR REPLACE registry_module_releases
             SET rowid = (SELECT rowid FROM registry_module_releases WHERE id = 'other')
             WHERE id = 'release'",
        ] {
            assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
            assert_eq!(revision(&db).await, 1);
            let row = db.query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT (SELECT COUNT(*) FROM registry_module_releases) AS releases,
                        (SELECT COUNT(*) FROM release_receipts) AS receipts,
                        (SELECT COUNT(*) FROM registry_release_authority_revisions) AS witnesses",
            )).await.unwrap().unwrap();
            assert_eq!(row.try_get::<i64>("", "releases").unwrap(), 2);
            assert_eq!(row.try_get::<i64>("", "receipts").unwrap(), 1);
            assert_eq!(row.try_get::<i64>("", "witnesses").unwrap(), 2);
        }

        db.execute_unprepared("UPDATE registry_module_releases SET status = 'yanked' WHERE id = 'release'")
            .await.unwrap();
        assert_eq!(revision(&db).await, 2);
        db.execute_unprepared("DELETE FROM registry_module_releases WHERE id = 'release'")
            .await.unwrap();
        assert_eq!(revision(&db).await, 3);
        db.execute_unprepared(RELEASE).await.unwrap();
        assert_eq!(revision(&db).await, 4);
    }

    async fn transaction(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = crate::backend::begin_fenced_transaction(db).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_witness_is_permanent_and_late_row_or_table_locks_fail_without_waiting() {
        let db = Database::connect(std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let schema = format!("registry_witness_{}", uuid::Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let initial = transaction(&db, &schema).await;
        setup(&SchemaManager::new(&initial)).await;
        initial.commit().await.unwrap();

        for (lock, diagnostic) in [
            ("LOCK TABLE module_artifact_activation_locks IN SHARE MODE", "Registry release platform fence busy"),
            ("LOCK TABLE registry_release_authority_revisions IN SHARE MODE", "Registry release authority witness busy"),
            ("SELECT revision FROM registry_release_authority_revisions FOR UPDATE", "Registry release authority witness busy"),
        ] {
            let holder = transaction(&db, &schema).await;
            holder.execute_unprepared(lock).await.unwrap();
            let writer = transaction(&db, &schema).await;
            let error = tokio::time::timeout(Duration::from_secs(3),
                writer.execute_unprepared("UPDATE registry_module_releases SET status = 'yanked'")
            ).await.expect("late lock must not wait").unwrap_err();
            assert!(error.to_string().contains(diagnostic), "{error}");
            writer.rollback().await.unwrap();
            holder.rollback().await.unwrap();
        }

        for sql in [
            "TRUNCATE registry_release_authority_revisions",
            "DELETE FROM registry_release_authority_revisions",
            "UPDATE registry_release_authority_revisions SET revision = 0",
            "UPDATE registry_release_authority_revisions SET revision = 10",
            "UPDATE registry_release_authority_revisions SET version = '2.0.0'",
        ] {
            let tx = transaction(&db, &schema).await;
            assert!(tx.execute_unprepared(sql).await.is_err(), "{sql}");
            tx.rollback().await.unwrap();
        }
        let writer = transaction(&db, &schema).await;
        assert_eq!(revision(&writer).await, 1);
        writer.execute_unprepared("DELETE FROM registry_module_releases").await.unwrap();
        assert_eq!(revision(&writer).await, 2);
        writer.execute_unprepared(RELEASE).await.unwrap();
        assert_eq!(revision(&writer).await, 3);
        writer.commit().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
