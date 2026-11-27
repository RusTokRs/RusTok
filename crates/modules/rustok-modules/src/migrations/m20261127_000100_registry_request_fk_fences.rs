//! Nonblocking PostgreSQL FK parent admission for registry request children.
use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

// These tables have exactly one FK to a publish request. The two-parent
// children and owner transfer receipt are installed separately below.
const REQUEST_CHILDREN: &[&str] = &[
    "registry_publish_request_review_operations",
    "registry_publish_request_create_operations",
    "registry_publish_artifact_operations",
    "registry_validation_job_enqueue_operations",
    "registry_validation_stage_report_operations",
    "registry_publication_evidence",
    "registry_validation_jobs",
    "registry_validation_stages",
    "registry_publish_build_staging",
    "registry_publish_external_staging",
    "registry_publish_alloy_staging",
    "registry_publish_platform_admissions",
];

const POSTGRES_GUARDS: &str = r#"
CREATE FUNCTION registry_fence_request_child_parents() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    -- Child DML may already own a child row. Never wait for a parent here.
    -- A request precedes its evidence or release in canonical publication.
    LOCK TABLE registry_publish_requests IN ROW SHARE MODE NOWAIT;
    PERFORM 1 FROM registry_publish_requests WHERE id = NEW.request_id FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Registry request child parent missing or invisible' USING ERRCODE = '23503';
    END IF;

    IF TG_TABLE_NAME = 'registry_author_signature_evidence_operations' THEN
        LOCK TABLE registry_publication_evidence IN ROW SHARE MODE NOWAIT;
        PERFORM 1 FROM registry_publication_evidence WHERE id = NEW.evidence_id FOR KEY SHARE NOWAIT;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'Registry evidence receipt parent missing or invisible' USING ERRCODE = '23503';
        END IF;
    ELSIF TG_TABLE_NAME = 'registry_module_release_artifacts' THEN
        LOCK TABLE registry_module_releases IN ROW SHARE MODE NOWAIT;
        PERFORM 1 FROM registry_module_releases WHERE id = NEW.release_id FOR KEY SHARE NOWAIT;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'Registry release artifact parent missing or invisible' USING ERRCODE = '23503';
        END IF;
    END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'Registry request child parent fence busy' USING ERRCODE = '55P03';
END; $$;

CREATE FUNCTION registry_fence_owner_receipt_parent() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    LOCK TABLE registry_module_owners IN ROW SHARE MODE NOWAIT;
    PERFORM 1 FROM registry_module_owners WHERE slug = NEW.slug FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Registry owner receipt parent missing or invisible' USING ERRCODE = '23503';
    END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'Registry owner receipt parent fence busy' USING ERRCODE = '55P03';
END; $$;

CREATE TRIGGER registry_request_parent_fence
BEFORE INSERT OR UPDATE OF request_id, evidence_id ON registry_author_signature_evidence_operations
FOR EACH ROW EXECUTE FUNCTION registry_fence_request_child_parents();
CREATE TRIGGER registry_request_parent_fence
BEFORE INSERT OR UPDATE OF request_id, release_id ON registry_module_release_artifacts
FOR EACH ROW EXECUTE FUNCTION registry_fence_request_child_parents();
CREATE TRIGGER registry_owner_receipt_parent_fence
BEFORE INSERT OR UPDATE OF slug ON registry_owner_transfer_operations
FOR EACH ROW EXECUTE FUNCTION registry_fence_owner_receipt_parent();
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if SupportedBackend::new(manager.get_database_backend())? != SupportedBackend::Postgres {
            // SQLite serializes writers; retain its existing FK enforcement.
            return Ok(());
        }
        let db = manager.get_connection();
        db.execute_unprepared(POSTGRES_GUARDS).await?;
        for table in REQUEST_CHILDREN {
            // These are compile-time owner table names, never external input.
            // Metadata-only UPDATE (notably a runner heartbeat) does not
            // acquire a new request lock. Deletes retain existing FK actions.
            db.execute_unprepared(&format!(
                "CREATE TRIGGER registry_request_parent_fence
                 BEFORE INSERT OR UPDATE OF request_id ON {table}
                 FOR EACH ROW EXECUTE FUNCTION registry_fence_request_child_parents()"
            )).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if SupportedBackend::new(manager.get_database_backend())? == SupportedBackend::Postgres {
            return Err(DbErr::Migration("registry request FK lock order cannot be downgraded safely".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DatabaseConnection, DatabaseTransaction, DbBackend, TransactionTrait};
    use std::time::Duration;

    async fn tx(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = crate::backend::begin_fenced_transaction(db).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx
    }

    async fn reject_busy(db: &DatabaseConnection, schema: &str, sql: &str, message: &str) {
        let writer = tx(db, schema).await;
        let error = tokio::time::timeout(Duration::from_secs(3), writer.execute_unprepared(sql))
            .await.expect("late FK admission must not wait")
            .expect_err("contended FK parent must reject the write");
        assert!(error.to_string().contains(message), "{error}");
        writer.rollback().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_registry_request_child_parent_guards_cover_receipts_and_staging() {
        let db = Database::connect(std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let schema = format!("registry_request_fk_{}", uuid::Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = tx(&db, &schema).await;
        setup.execute_unprepared(
            "CREATE TABLE registry_publish_requests (id TEXT PRIMARY KEY);
             CREATE TABLE registry_module_releases (id TEXT PRIMARY KEY);
             CREATE TABLE registry_module_owners (slug TEXT PRIMARY KEY);
             INSERT INTO registry_publish_requests VALUES ('a'), ('b');
             INSERT INTO registry_module_releases VALUES ('release');
             INSERT INTO registry_module_owners VALUES ('owner');"
        ).await.unwrap();
        for table in REQUEST_CHILDREN {
            setup.execute_unprepared(&format!(
                "CREATE TABLE {table} (id TEXT PRIMARY KEY, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id), detail TEXT);
                 INSERT INTO {table} VALUES ('existing', 'a', NULL)"
            )).await.unwrap();
        }
        setup.execute_unprepared(
            "CREATE TABLE registry_author_signature_evidence_operations (
                 id TEXT PRIMARY KEY, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),
                 evidence_id TEXT NOT NULL REFERENCES registry_publication_evidence(id));
             CREATE TABLE registry_module_release_artifacts (
                 id TEXT PRIMARY KEY, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),
                 release_id TEXT NOT NULL REFERENCES registry_module_releases(id));
             CREATE TABLE registry_owner_transfer_operations (
                 id TEXT PRIMARY KEY, slug TEXT NOT NULL REFERENCES registry_module_owners(slug));"
        ).await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.commit().await.unwrap();

        let owner = tx(&db, &schema).await;
        owner.execute_unprepared("SELECT id FROM registry_publish_requests WHERE id = 'b' FOR UPDATE").await.unwrap();
        for table in REQUEST_CHILDREN {
            reject_busy(&db, &schema, &format!("INSERT INTO {table} VALUES ('new', 'b', NULL)"),
                "Registry request child parent fence busy").await;
            reject_busy(&db, &schema, &format!("UPDATE {table} SET request_id = 'b' WHERE id = 'existing'"),
                "Registry request child parent fence busy").await;
        }
        reject_busy(&db, &schema, "INSERT INTO registry_author_signature_evidence_operations VALUES ('op', 'b', 'existing')", "Registry request child parent fence busy").await;
        reject_busy(&db, &schema, "INSERT INTO registry_module_release_artifacts VALUES ('artifact', 'b', 'release')", "Registry request child parent fence busy").await;
        owner.execute_unprepared("INSERT INTO registry_validation_jobs VALUES ('canonical', 'b', NULL)").await.unwrap();
        owner.commit().await.unwrap();

        let owner = tx(&db, &schema).await;
        owner.execute_unprepared(
            "SELECT id FROM registry_publication_evidence WHERE id = 'existing' FOR UPDATE;
             SELECT id FROM registry_module_releases WHERE id = 'release' FOR UPDATE;
             SELECT slug FROM registry_module_owners WHERE slug = 'owner' FOR UPDATE"
        ).await.unwrap();
        reject_busy(&db, &schema, "INSERT INTO registry_author_signature_evidence_operations VALUES ('op', 'a', 'existing')", "Registry request child parent fence busy").await;
        reject_busy(&db, &schema, "INSERT INTO registry_module_release_artifacts VALUES ('artifact', 'a', 'release')", "Registry request child parent fence busy").await;
        reject_busy(&db, &schema, "INSERT INTO registry_owner_transfer_operations VALUES ('transfer', 'owner')", "Registry owner receipt parent fence busy").await;
        owner.rollback().await.unwrap();

        let owner = tx(&db, &schema).await;
        owner.execute_unprepared("SELECT id FROM registry_publish_requests WHERE id = 'a' FOR UPDATE").await.unwrap();
        // A heartbeat-like metadata write must not introduce a parent edge.
        let writer = tx(&db, &schema).await;
        tokio::time::timeout(Duration::from_secs(3), writer.execute_unprepared(
            "UPDATE registry_validation_stages SET detail = 'heartbeat' WHERE id = 'existing'"
        )).await.unwrap().unwrap();
        writer.commit().await.unwrap();
        owner.rollback().await.unwrap();

        let writer = tx(&db, &schema).await;
        writer.execute_unprepared(
            "INSERT INTO registry_author_signature_evidence_operations VALUES ('op', 'a', 'existing');
             INSERT INTO registry_module_release_artifacts VALUES ('artifact', 'a', 'release');
             INSERT INTO registry_owner_transfer_operations VALUES ('transfer', 'owner');"
        ).await.unwrap();
        writer.commit().await.unwrap();

        let missing = tx(&db, &schema).await;
        let error = missing.execute_unprepared("INSERT INTO registry_validation_jobs VALUES ('missing', 'absent', NULL)").await.unwrap_err();
        assert!(error.to_string().contains("Registry request child parent missing or invisible"), "{error}");
        missing.rollback().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
