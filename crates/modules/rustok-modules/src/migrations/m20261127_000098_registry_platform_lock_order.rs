//! PostgreSQL registry writers must not wait on a platform fence while holding a release row.
use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        if SupportedBackend::new(manager.get_database_backend())? != SupportedBackend::Postgres {
            // SQLite already serializes writers, and has no TRUNCATE.
            return Ok(());
        }

        // BEFORE INSERT also guards the first release/fence insertion and
        // ON CONFLICT paths. UPDATE/DELETE may already own the release row
        // when their BEFORE ROW trigger runs, so the gate MUST be nonblocking.
        // Canonical writers hold the same advisory gate before any row lock.
        db.execute_unprepared(
            "CREATE FUNCTION registry_release_guard_platform_fence() RETURNS TRIGGER LANGUAGE plpgsql AS $$
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
               PERFORM 1 FROM module_artifact_activation_locks
                 WHERE scope_kind = 'platform' AND scope_tenant_key = 'platform' AND slug = target_slug
                 FOR UPDATE NOWAIT;
               IF NOT FOUND THEN
                 INSERT INTO module_artifact_activation_locks (scope_kind, scope_tenant_key, slug)
                   VALUES ('platform', 'platform', target_slug);
               END IF;
               IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
               RETURN NEW;
             END $$",
        ).await?;
        db.execute_unprepared(
            "CREATE TRIGGER registry_release_platform_fence
             BEFORE INSERT OR UPDATE OR DELETE ON registry_module_releases
             FOR EACH ROW EXECUTE FUNCTION registry_release_guard_platform_fence()",
        ).await?;
        db.execute_unprepared(
            "CREATE TRIGGER registry_release_no_truncate
             BEFORE TRUNCATE ON registry_module_releases
             FOR EACH STATEMENT EXECUTE FUNCTION registry_release_guard_platform_fence()",
        ).await?;

        // The AFTER trigger now only advances the exact permanent witness.
        // It cannot acquire the earlier platform fence; no witness is reset.
        db.execute_unprepared(
            "CREATE OR REPLACE FUNCTION registry_release_advance_authority() RETURNS TRIGGER LANGUAGE plpgsql AS $$
             DECLARE target_slug TEXT; target_version TEXT;
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
               INSERT INTO registry_release_authority_revisions (slug, version, revision)
                 VALUES (target_slug, target_version, 1)
                 ON CONFLICT (slug, version) DO UPDATE SET revision =
                   CASE WHEN registry_release_authority_revisions.revision < 9223372036854775807
                        THEN registry_release_authority_revisions.revision + 1 ELSE 0 END;
               IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
               RETURN NEW;
             END $$",
        ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if SupportedBackend::new(manager.get_database_backend())? == SupportedBackend::Postgres {
            return Err(DbErr::Migration("registry platform lock order cannot be downgraded safely".into()));
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
    use crate::{ArtifactRegistryExecutionAuthority, ArtifactReleaseRef, ModuleInstallationScope};

    async fn transaction(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = crate::backend::begin_fenced_transaction(db).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx
    }

    async fn require_busy(tx: DatabaseTransaction, sql: &str) {
        let result = tokio::time::timeout(Duration::from_secs(3), tx.execute_unprepared(sql))
            .await.expect("a late registry fence must fail instead of waiting")
            .expect_err("unfenced registry mutation must fail while the gate is owned");
        assert!(result.to_string().contains("Registry release platform fence busy"), "{result}");
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_registry_platform_gate_rejects_late_writer_without_waiting() {
        let db = Database::connect(std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let schema = format!("registry_lock_order_{}", Uuid::new_v4().simple());
        let slug = format!("sample_{}", Uuid::new_v4().simple());
        let fresh_slug = format!("fresh_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = transaction(&db, &schema).await;
        setup.execute_unprepared(
            "CREATE TABLE module_artifact_activation_locks (
                scope_kind TEXT NOT NULL, scope_tenant_key TEXT NOT NULL, slug TEXT NOT NULL,
                PRIMARY KEY (scope_kind, scope_tenant_key, slug));
             CREATE TABLE registry_module_releases (
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL,
                status TEXT NOT NULL, checksum_sha256 TEXT, metadata TEXT, UNIQUE (slug, version));"
        ).await.unwrap();
        super::super::m20261127_000096_registry_release_authority::Migration
            .up(&SchemaManager::new(&setup)).await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.execute_unprepared(&format!(
            "INSERT INTO registry_module_releases VALUES ('release-1', '{slug}', '1.0.0', 'active', '{}', NULL)",
            "a".repeat(64)
        )).await.unwrap();
        let release = ArtifactReleaseRef {
            slug: slug.clone(), version: "1.0.0".into(), digest: format!("sha256:{}", "a".repeat(64)),
        };
        let proof = ArtifactRegistryExecutionAuthority::load_on(&setup, &release).await.unwrap();
        setup.commit().await.unwrap();

        let effect = transaction(&db, &schema).await;
        proof.require_current_on(&effect).await.unwrap();
        // Direct SQL cannot block under a release-row lock and leave the
        // canonical writer waiting for that row while it holds the gate.
        require_busy(transaction(&db, &schema).await, "UPDATE registry_module_releases SET status = 'yanked'").await;
        require_busy(transaction(&db, &schema).await, "DELETE FROM registry_module_releases").await;
        // Metadata still follows the fence but must not advance the witness.
        require_busy(transaction(&db, &schema).await, "UPDATE registry_module_releases SET metadata = 'changed'").await;
        effect.execute_unprepared("UPDATE registry_module_releases SET metadata = 'canonical'").await.unwrap();
        proof.require_current_on(&effect).await.unwrap();
        effect.commit().await.unwrap();

        // No durable fence row exists for this slug until the holder commits.
        let first = transaction(&db, &schema).await;
        crate::installation::acquire_artifact_activation_lock(&first, &ModuleInstallationScope::Platform, &fresh_slug).await.unwrap();
        let insert = format!("INSERT INTO registry_module_releases VALUES ('release-2', '{fresh_slug}', '1.0.0', 'active', '{}', NULL)", "b".repeat(64));
        require_busy(transaction(&db, &schema).await, &insert).await;
        first.rollback().await.unwrap();
        let writer = transaction(&db, &schema).await;
        writer.execute_unprepared(&insert).await.unwrap();
        writer.execute_unprepared("UPDATE registry_module_releases SET status = 'yanked' WHERE id = 'release-1'").await.unwrap();
        // Canonical admission waits for an already-admitted direct writer,
        // then observes its committed witness rather than an old snapshot.
        let worker_db = db.clone();
        let worker_schema = schema.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let mut worker = tokio::spawn(async move {
            let tx = transaction(&worker_db, &worker_schema).await;
            ready_tx.send(()).unwrap();
            let result = proof.require_current_on(&tx).await;
            tx.rollback().await.unwrap();
            result
        });
        ready_rx.await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(150), &mut worker).await.is_err());
        writer.commit().await.unwrap();
        assert!(tokio::time::timeout(Duration::from_secs(5), worker).await.unwrap().unwrap().is_err());

        let truncate = transaction(&db, &schema).await;
        let error = truncate.execute_unprepared("TRUNCATE registry_module_releases").await.unwrap_err();
        assert!(error.to_string().contains("Registry releases cannot be truncated"), "{error}");
        truncate.rollback().await.unwrap();
        let check = transaction(&db, &schema).await;
        let current = ArtifactRegistryExecutionAuthority::load_on(&check, &release).await.unwrap();
        current.require_current_on(&check).await.unwrap();
        check.rollback().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
