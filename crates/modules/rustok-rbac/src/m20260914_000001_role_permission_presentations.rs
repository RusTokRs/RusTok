use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        ensure_supported_backend(backend)?;
        let connection = manager.get_connection();

        for sql in up_statements(backend) {
            connection
                .execute_raw(Statement::from_string(backend, sql.to_string()))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        ensure_supported_backend(backend)?;
        let connection = manager.get_connection();
        for sql in [
            "DROP TABLE IF EXISTS rbac_permission_presentations",
            "DROP TABLE IF EXISTS rbac_role_presentations",
            "DROP INDEX IF EXISTS uq_rbac_permissions_tenant_id_id",
        ] {
            connection
                .execute_raw(Statement::from_string(backend, sql.to_string()))
                .await?;
        }
        Ok(())
    }
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), DbErr> {
    match backend {
        DbBackend::Postgres | DbBackend::Sqlite => Ok(()),
        backend => Err(DbErr::Migration(format!(
            "RBAC presentation storage does not support {backend:?}"
        ))),
    }
}

fn up_statements(backend: DbBackend) -> &'static [&'static str] {
    match backend {
        DbBackend::Postgres => &[
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_rbac_permissions_tenant_id_id ON permissions (tenant_id, id)",
            "CREATE TABLE IF NOT EXISTS rbac_role_presentations (tenant_id UUID NOT NULL, role_id UUID NOT NULL, locale VARCHAR(32) NOT NULL, display_name TEXT NOT NULL, description TEXT, copy_revision BIGINT NOT NULL DEFAULT 1, created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (tenant_id, role_id, locale), CONSTRAINT fk_rbac_role_presentation_role FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON UPDATE RESTRICT ON DELETE CASCADE, CONSTRAINT ck_rbac_role_presentation_locale CHECK (locale <> ''), CONSTRAINT ck_rbac_role_presentation_revision CHECK (copy_revision > 0))",
            "CREATE INDEX IF NOT EXISTS idx_rbac_role_presentations_role ON rbac_role_presentations (tenant_id, role_id)",
            "INSERT INTO rbac_role_presentations (tenant_id, role_id, locale, display_name, description, copy_revision) SELECT tenant_id, id, 'und', name, description, 1 FROM roles WHERE name IS NOT NULL ON CONFLICT (tenant_id, role_id, locale) DO NOTHING",
            "CREATE TABLE IF NOT EXISTS rbac_permission_presentations (tenant_id UUID NOT NULL, permission_id UUID NOT NULL, locale VARCHAR(32) NOT NULL, display_name TEXT, description TEXT, copy_revision BIGINT NOT NULL DEFAULT 1, created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (tenant_id, permission_id, locale), CONSTRAINT fk_rbac_permission_presentation_permission FOREIGN KEY (tenant_id, permission_id) REFERENCES permissions (tenant_id, id) ON UPDATE RESTRICT ON DELETE CASCADE, CONSTRAINT ck_rbac_permission_presentation_locale CHECK (locale <> ''), CONSTRAINT ck_rbac_permission_presentation_revision CHECK (copy_revision > 0), CONSTRAINT ck_rbac_permission_presentation_copy CHECK (display_name IS NOT NULL OR description IS NOT NULL))",
            "CREATE INDEX IF NOT EXISTS idx_rbac_permission_presentations_permission ON rbac_permission_presentations (tenant_id, permission_id)",
            "INSERT INTO rbac_permission_presentations (tenant_id, permission_id, locale, display_name, description, copy_revision) SELECT tenant_id, id, 'und', NULL, description, 1 FROM permissions WHERE description IS NOT NULL ON CONFLICT (tenant_id, permission_id, locale) DO NOTHING",
        ],
        DbBackend::Sqlite => &[
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_rbac_permissions_tenant_id_id ON permissions (tenant_id, id)",
            "CREATE TABLE IF NOT EXISTS rbac_role_presentations (tenant_id BLOB NOT NULL, role_id BLOB NOT NULL, locale VARCHAR(32) NOT NULL, display_name TEXT NOT NULL, description TEXT, copy_revision INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (tenant_id, role_id, locale), FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON UPDATE RESTRICT ON DELETE CASCADE, CHECK (locale <> ''), CHECK (copy_revision > 0))",
            "CREATE INDEX IF NOT EXISTS idx_rbac_role_presentations_role ON rbac_role_presentations (tenant_id, role_id)",
            "INSERT INTO rbac_role_presentations (tenant_id, role_id, locale, display_name, description, copy_revision) SELECT tenant_id, id, 'und', name, description, 1 FROM roles WHERE name IS NOT NULL ON CONFLICT (tenant_id, role_id, locale) DO NOTHING",
            "CREATE TABLE IF NOT EXISTS rbac_permission_presentations (tenant_id BLOB NOT NULL, permission_id BLOB NOT NULL, locale VARCHAR(32) NOT NULL, display_name TEXT, description TEXT, copy_revision INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (tenant_id, permission_id, locale), FOREIGN KEY (tenant_id, permission_id) REFERENCES permissions (tenant_id, id) ON UPDATE RESTRICT ON DELETE CASCADE, CHECK (locale <> ''), CHECK (copy_revision > 0), CHECK (display_name IS NOT NULL OR description IS NOT NULL))",
            "CREATE INDEX IF NOT EXISTS idx_rbac_permission_presentations_permission ON rbac_permission_presentations (tenant_id, permission_id)",
            "INSERT INTO rbac_permission_presentations (tenant_id, permission_id, locale, display_name, description, copy_revision) SELECT tenant_id, id, 'und', NULL, description, 1 FROM permissions WHERE description IS NOT NULL ON CONFLICT (tenant_id, permission_id, locale) DO NOTHING",
        ],
        _ => &[],
    }
}
