use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Creates the control-plane-owned, digest-pinned installation record.
///
/// Artifact bytes remain in the platform CAS; this table stores the immutable
/// registry provenance, admitted descriptor, and exact dependency lock needed
/// for reproducible execution. On PostgreSQL, the host sets
/// `rustok.tenant_id` for tenant-scoped connections.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_installations (\
                    installation_id UUID PRIMARY KEY,\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    tenant_id UUID NULL,\
                    registry TEXT NOT NULL,\
                    repository TEXT NOT NULL,\
                    manifest_digest TEXT NOT NULL,\
                    slug TEXT NOT NULL,\
                    version TEXT NOT NULL,\
                    payload_kind TEXT NOT NULL,\
                    runtime_abi TEXT NOT NULL,\
                    payload_digest TEXT NOT NULL,\
                    entrypoint TEXT NOT NULL,\
                    descriptor JSONB NOT NULL,\
                    dependency_graph_revision BIGINT NOT NULL,\
                    dependency_graph_digest TEXT NOT NULL,\
                    dependency_lock JSONB NOT NULL,\
                    installed_at TIMESTAMPTZ NOT NULL,\
                    CHECK ((scope_kind = 'platform' AND tenant_id IS NULL) OR (scope_kind = 'tenant' AND tenant_id IS NOT NULL))\
                )",
                "CREATE INDEX module_artifact_installations_tenant_idx ON module_artifact_installations (tenant_id, slug, version)",
                "CREATE UNIQUE INDEX module_artifact_installations_platform_identity ON module_artifact_installations (slug, version, manifest_digest) WHERE scope_kind = 'platform'",
                "CREATE UNIQUE INDEX module_artifact_installations_tenant_identity ON module_artifact_installations (tenant_id, slug, version, manifest_digest) WHERE scope_kind = 'tenant'",
                "ALTER TABLE module_artifact_installations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_installations_tenant_scope ON module_artifact_installations \
                    USING (scope_kind = 'platform' OR tenant_id::text = current_setting('rustok.tenant_id', true)) \
                    WITH CHECK (scope_kind = 'platform' OR tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_installations (\
                    installation_id TEXT PRIMARY KEY NOT NULL,\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    tenant_id TEXT NULL,\
                    registry TEXT NOT NULL,\
                    repository TEXT NOT NULL,\
                    manifest_digest TEXT NOT NULL,\
                    slug TEXT NOT NULL,\
                    version TEXT NOT NULL,\
                    payload_kind TEXT NOT NULL,\
                    runtime_abi TEXT NOT NULL,\
                    payload_digest TEXT NOT NULL,\
                    entrypoint TEXT NOT NULL,\
                    descriptor JSON NOT NULL,\
                    dependency_graph_revision INTEGER NOT NULL,\
                    dependency_graph_digest TEXT NOT NULL,\
                    dependency_lock JSON NOT NULL,\
                    installed_at TEXT NOT NULL,\
                    CHECK ((scope_kind = 'platform' AND tenant_id IS NULL) OR (scope_kind = 'tenant' AND tenant_id IS NOT NULL))\
                )",
                "CREATE INDEX module_artifact_installations_tenant_idx ON module_artifact_installations (tenant_id, slug, version)",
                "CREATE UNIQUE INDEX module_artifact_installations_platform_identity ON module_artifact_installations (slug, version, manifest_digest) WHERE scope_kind = 'platform'",
                "CREATE UNIQUE INDEX module_artifact_installations_tenant_identity ON module_artifact_installations (tenant_id, slug, version, manifest_digest) WHERE scope_kind = 'tenant'",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "module artifact installation migration does not support database backend {backend:?}"
                )));
            }
        };

        for statement in statements {
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE module_artifact_installations")
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TryGetable};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn sqlite_schema_keeps_scope_and_identity_invariants() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("migration");

        database
            .execute_unprepared(
                "INSERT INTO module_artifact_installations (\
                    installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, \
                    slug, version, payload_kind, runtime_abi, payload_digest, entrypoint, descriptor, \
                    dependency_graph_revision, dependency_graph_digest, dependency_lock, installed_at\
                 ) VALUES (\
                    'a', 'platform', NULL, 'registry.example', 'modules/example', 'sha256:manifest', \
                    'example', '1.0.0', 'rhai', 'rustok:module/runtime@1', 'sha256:payload', 'main', '{}', \
                    1, 'sha256:graph', '{}', '2026-07-11T00:00:00Z'\
                 )",
            )
            .await
            .expect("platform installation");

        let duplicate = database
            .execute_unprepared(
                "INSERT INTO module_artifact_installations (\
                    installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, \
                    slug, version, payload_kind, runtime_abi, payload_digest, entrypoint, descriptor, \
                    dependency_graph_revision, dependency_graph_digest, dependency_lock, installed_at\
                 ) VALUES (\
                    'b', 'platform', NULL, 'registry.example', 'modules/example', 'sha256:manifest', \
                    'example', '1.0.0', 'rhai', 'rustok:module/runtime@1', 'sha256:payload', 'main', '{}', \
                    1, 'sha256:graph', '{}', '2026-07-11T00:00:00Z'\
                 )",
            )
            .await;
        assert!(duplicate.is_err());

        let row = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT scope_kind FROM module_artifact_installations".to_string(),
            ))
            .await
            .expect("query")
            .expect("row");
        assert_eq!(
            String::try_get(&row, "", "scope_kind").expect("scope"),
            "platform"
        );
    }
}
