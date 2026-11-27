//! Permanent registry release continuity witness, independent of yank semantics.
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
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        if backend == SupportedBackend::Postgres {
            db.execute_unprepared("LOCK TABLE registry_module_releases IN SHARE ROW EXCLUSIVE MODE").await?;
        }
        // No FK: absence and removal are observable authority states. Never
        // delete or reset a witness when its release disappears.
        db.execute_unprepared(
            "CREATE TABLE registry_release_authority_revisions (
                slug TEXT NOT NULL, version TEXT NOT NULL,
                revision BIGINT NOT NULL CHECK (revision > 0),
                PRIMARY KEY (slug, version)
             )"
        ).await?;
        db.execute_unprepared(
            "INSERT INTO registry_release_authority_revisions (slug, version, revision)
             SELECT slug, version, 1 FROM registry_module_releases"
        ).await?;

        match backend {
            SupportedBackend::Postgres => {
                db.execute_unprepared(
                    "CREATE FUNCTION registry_release_advance_authority() RETURNS TRIGGER LANGUAGE plpgsql AS $$
                     DECLARE target_slug TEXT; target_version TEXT;
                     BEGIN
                       IF TG_OP = 'UPDATE' THEN
                         IF NEW.slug IS DISTINCT FROM OLD.slug OR NEW.version IS DISTINCT FROM OLD.version
                            OR NEW.id IS DISTINCT FROM OLD.id THEN
                           RAISE EXCEPTION 'Registry release identity is immutable' USING ERRCODE = '23514';
                         END IF;
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
                       INSERT INTO module_artifact_activation_locks (scope_kind, scope_tenant_key, slug)
                         VALUES ('platform', 'platform', target_slug)
                         ON CONFLICT (scope_kind, scope_tenant_key, slug) DO UPDATE SET slug = EXCLUDED.slug;
                       INSERT INTO registry_release_authority_revisions (slug, version, revision)
                         VALUES (target_slug, target_version, 1)
                         ON CONFLICT (slug, version) DO UPDATE SET revision =
                           CASE WHEN registry_release_authority_revisions.revision < 9223372036854775807
                                THEN registry_release_authority_revisions.revision + 1 ELSE 0 END;
                       IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
                       RETURN NEW;
                     END $$"
                ).await?;
                db.execute_unprepared(
                    "CREATE TRIGGER registry_release_authority_change
                     AFTER INSERT OR UPDATE OR DELETE ON registry_module_releases
                     FOR EACH ROW EXECUTE FUNCTION registry_release_advance_authority()"
                ).await?;
            }
            SupportedBackend::Sqlite => {
                // AFTER triggers only advance for successful row mutations,
                // including an insertion following a previous deletion.
                db.execute_unprepared(
                    "CREATE TRIGGER registry_release_authority_identity
                     BEFORE UPDATE ON registry_module_releases
                     WHEN NEW.slug IS NOT OLD.slug OR NEW.version IS NOT OLD.version OR NEW.id IS NOT OLD.id
                     BEGIN SELECT RAISE(ABORT, 'Registry release identity is immutable'); END"
                ).await?;
                for (event, row, predicate) in [
                    ("INSERT", "NEW", ""),
                    ("UPDATE", "NEW", "WHEN NEW.status IS NOT OLD.status OR NEW.checksum_sha256 IS NOT OLD.checksum_sha256"),
                    ("DELETE", "OLD", ""),
                ] {
                    db.execute_unprepared(&format!(
                        "CREATE TRIGGER registry_release_authority_{event}
                         AFTER {event} ON registry_module_releases {predicate}
                         BEGIN
                           INSERT INTO module_artifact_activation_locks (scope_kind, scope_tenant_key, slug)
                             VALUES ('platform', 'platform', {row}.slug)
                             ON CONFLICT (scope_kind, scope_tenant_key, slug) DO UPDATE SET slug = EXCLUDED.slug;
                           SELECT RAISE(ABORT, 'Registry release authority exhausted')
                             WHERE EXISTS (SELECT 1 FROM registry_release_authority_revisions
                               WHERE slug = {row}.slug AND version = {row}.version AND revision = 9223372036854775807);
                           INSERT INTO registry_release_authority_revisions (slug, version, revision)
                             VALUES ({row}.slug, {row}.version, 1)
                             ON CONFLICT (slug, version) DO UPDATE SET revision =
                               CASE WHEN registry_release_authority_revisions.revision < 9223372036854775807
                                    THEN registry_release_authority_revisions.revision + 1 ELSE 0 END;
                         END"
                    )).await?;
                }
            }
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("registry release authority continuity cannot be downgraded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DatabaseTransaction, DbBackend, TransactionTrait};
    use crate::{ArtifactRegistryExecutionAuthority, ArtifactReleaseRef};

    const SCHEMA: &str =
        "CREATE TABLE module_artifact_activation_locks (
            scope_kind TEXT NOT NULL, scope_tenant_key TEXT NOT NULL, slug TEXT NOT NULL,
            PRIMARY KEY (scope_kind, scope_tenant_key, slug)
         );
         CREATE TABLE registry_module_releases (
            id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL,
            status TEXT NOT NULL, checksum_sha256 TEXT, metadata TEXT,
            UNIQUE (slug, version)
         )";

    fn release() -> ArtifactReleaseRef {
        ArtifactReleaseRef {
            slug: "sample_module".into(), version: "1.0.0".into(),
            digest: format!("sha256:{}", "a".repeat(64)),
        }
    }

    fn insert_release() -> String {
        format!("INSERT INTO registry_module_releases (id, slug, version, status, checksum_sha256)
            VALUES ('release-1', 'sample_module', '1.0.0', 'active', '{}')", "a".repeat(64))
    }

    #[tokio::test]
    async fn registry_authority_rejects_status_and_availability_aba() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(SCHEMA).await.unwrap();
        Migration.up(&SchemaManager::new(&db)).await.unwrap();

        let release = release();
        let unlisted = ArtifactRegistryExecutionAuthority::capture(&db, &release).await.unwrap();
        db.execute_unprepared(&insert_release()).await.unwrap();
        let active = ArtifactRegistryExecutionAuthority::capture(&db, &release).await.unwrap();
        assert!(!unlisted.is_current(&db).await.unwrap());

        // Metadata and rolled-back status changes cannot renew a lease.
        db.execute_unprepared("UPDATE registry_module_releases SET metadata = 'changed'").await.unwrap();
        let tx = crate::backend::begin_fenced_transaction(&db).await.unwrap();
        tx.execute_unprepared("UPDATE registry_module_releases SET status = 'yanked'").await.unwrap();
        assert!(active.require_current_on(&tx).await.is_err());
        tx.rollback().await.unwrap();
        assert!(active.is_current(&db).await.unwrap());

        db.execute_unprepared("UPDATE registry_module_releases SET status = 'yanked'").await.unwrap();
        let yanked = ArtifactRegistryExecutionAuthority::capture(&db, &release).await.unwrap();
        assert!(!active.is_current(&db).await.unwrap());
        db.execute_unprepared("UPDATE registry_module_releases SET status = 'active'").await.unwrap();
        assert!(!active.is_current(&db).await.unwrap());
        assert!(!yanked.is_current(&db).await.unwrap());

        let restored = ArtifactRegistryExecutionAuthority::capture(&db, &release).await.unwrap();
        db.execute_unprepared("UPDATE registry_module_releases SET status = 'unavailable'").await.unwrap();
        assert!(ArtifactRegistryExecutionAuthority::capture(&db, &release).await.is_err());
        db.execute_unprepared("UPDATE registry_module_releases SET status = 'active'").await.unwrap();
        assert!(!restored.is_current(&db).await.unwrap());

        db.execute_unprepared("DELETE FROM registry_module_releases").await.unwrap();
        assert!(!unlisted.is_current(&db).await.unwrap());
        let absent_again = ArtifactRegistryExecutionAuthority::capture(&db, &release).await.unwrap();
        db.execute_unprepared(&insert_release()).await.unwrap();
        assert!(!absent_again.is_current(&db).await.unwrap());

        let current = ArtifactRegistryExecutionAuthority::capture(&db, &release).await.unwrap();
        let mut other_version = release.clone();
        other_version.version = "2.0.0".into();
        let other = ArtifactRegistryExecutionAuthority::capture(&db, &other_version).await.unwrap();
        db.execute_unprepared("UPDATE registry_module_releases SET checksum_sha256 = 'different'").await.unwrap();
        assert!(!current.is_current(&db).await.unwrap());
        assert!(other.is_current(&db).await.unwrap());
    }

    async fn postgres_transaction(db: &sea_orm::DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = crate::backend::begin_fenced_transaction(db).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_registry_writer_and_effect_share_platform_fence() {
        use std::time::Duration;
        use uuid::Uuid;

        let db = Database::connect(std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let schema = format!("registry_authority_test_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = postgres_transaction(&db, &schema).await;
        setup.execute_unprepared(SCHEMA).await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.execute_unprepared(&insert_release()).await.unwrap();
        let proof = ArtifactRegistryExecutionAuthority::load_on(&setup, &release()).await.unwrap();
        setup.commit().await.unwrap();

        let writer = postgres_transaction(&db, &schema).await;
        writer.execute_unprepared("UPDATE registry_module_releases SET status = 'yanked'").await.unwrap();
        let worker_db = db.clone();
        let worker_schema = schema.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let mut worker = tokio::spawn(async move {
            let effect = postgres_transaction(&worker_db, &worker_schema).await;
            ready_tx.send(()).unwrap();
            let result = proof.require_current_on(&effect).await;
            effect.rollback().await.unwrap();
            result
        });
        ready_rx.await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(150), &mut worker).await.is_err());
        writer.commit().await.unwrap();
        assert!(worker.await.unwrap().is_err());

        let effect = postgres_transaction(&db, &schema).await;
        let proof = ArtifactRegistryExecutionAuthority::load_on(&effect, &release()).await.unwrap();
        proof.require_current_on(&effect).await.unwrap();
        let worker_db = db.clone();
        let worker_schema = schema.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let mut writer = tokio::spawn(async move {
            let tx = postgres_transaction(&worker_db, &worker_schema).await;
            ready_tx.send(()).unwrap();
            tx.execute_unprepared("UPDATE registry_module_releases SET status = 'active'").await.unwrap();
            tx.commit().await.unwrap();
        });
        ready_rx.await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(150), &mut writer).await.is_err());
        effect.commit().await.unwrap();
        writer.await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
