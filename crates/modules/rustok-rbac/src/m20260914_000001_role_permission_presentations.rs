use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        if !matches!(backend, DbBackend::Postgres | DbBackend::Sqlite) {
            return Err(DbErr::Migration(
                "RBAC presentation storage requires PostgreSQL or SQLite".to_string(),
            ));
        }

        let connection = manager.get_connection();
        for sql in up_statements(backend) {
            connection.execute_unprepared(sql).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        if !matches!(backend, DbBackend::Postgres | DbBackend::Sqlite) {
            return Err(DbErr::Migration(
                "RBAC presentation storage requires PostgreSQL or SQLite".to_string(),
            ));
        }

        let connection = manager.get_connection();
        connection
            .execute_unprepared("DROP TABLE IF EXISTS rbac_permission_presentations")
            .await?;
        connection
            .execute_unprepared("DROP TABLE IF EXISTS rbac_role_presentations")
            .await?;
        Ok(())
    }
}

fn up_statements(backend: DbBackend) -> &'static [&'static str] {
    match backend {
        DbBackend::Postgres => &[
            "CREATE TABLE rbac_role_presentations (tenant_id UUID NOT NULL, role_slug VARCHAR(120) NOT NULL, locale VARCHAR(32) NOT NULL, display_name TEXT NULL, description TEXT NULL, copy_revision BIGINT NOT NULL DEFAULT 1 CHECK (copy_revision > 0), created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (tenant_id, role_slug, locale))",
            "CREATE INDEX rbac_role_presentations_lookup_idx ON rbac_role_presentations (tenant_id, role_slug)",
            "INSERT INTO rbac_role_presentations (tenant_id, role_slug, locale, display_name, description, copy_revision, created_at, updated_at) SELECT tenant_id, slug, 'und', name, description, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP FROM roles ON CONFLICT (tenant_id, role_slug, locale) DO NOTHING",
            "CREATE TABLE rbac_permission_presentations (tenant_id UUID NOT NULL, permission_key VARCHAR(255) NOT NULL, locale VARCHAR(32) NOT NULL, display_name TEXT NULL, description TEXT NULL, copy_revision BIGINT NOT NULL DEFAULT 1 CHECK (copy_revision > 0), created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (tenant_id, permission_key, locale))",
            "CREATE INDEX rbac_permission_presentations_lookup_idx ON rbac_permission_presentations (tenant_id, permission_key)",
            "INSERT INTO rbac_permission_presentations (tenant_id, permission_key, locale, display_name, description, copy_revision, created_at, updated_at) SELECT tenant_id, resource || ':' || action, 'und', NULL, description, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP FROM permissions ON CONFLICT (tenant_id, permission_key, locale) DO NOTHING",
        ],
        DbBackend::Sqlite => &[
            "CREATE TABLE rbac_role_presentations (tenant_id BLOB NOT NULL, role_slug VARCHAR(120) NOT NULL, locale VARCHAR(32) NOT NULL, display_name TEXT NULL, description TEXT NULL, copy_revision BIGINT NOT NULL DEFAULT 1 CHECK (copy_revision > 0), created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (tenant_id, role_slug, locale))",
            "CREATE INDEX rbac_role_presentations_lookup_idx ON rbac_role_presentations (tenant_id, role_slug)",
            "INSERT OR IGNORE INTO rbac_role_presentations (tenant_id, role_slug, locale, display_name, description, copy_revision, created_at, updated_at) SELECT tenant_id, slug, 'und', name, description, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP FROM roles",
            "CREATE TABLE rbac_permission_presentations (tenant_id BLOB NOT NULL, permission_key VARCHAR(255) NOT NULL, locale VARCHAR(32) NOT NULL, display_name TEXT NULL, description TEXT NULL, copy_revision BIGINT NOT NULL DEFAULT 1 CHECK (copy_revision > 0), created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (tenant_id, permission_key, locale))",
            "CREATE INDEX rbac_permission_presentations_lookup_idx ON rbac_permission_presentations (tenant_id, permission_key)",
            "INSERT OR IGNORE INTO rbac_permission_presentations (tenant_id, permission_key, locale, display_name, description, copy_revision, created_at, updated_at) SELECT tenant_id, resource || ':' || action, 'und', NULL, description, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP FROM permissions",
        ],
        _ => &[],
    }
}
