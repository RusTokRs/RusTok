//! Owner-local localized presentation storage for canonical RBAC roles and permissions.
//!
//! This migration deliberately leaves authorization identity in `roles`, `permissions`,
//! `role_permissions`, and `user_roles` untouched. Legacy inline presentation is copied
//! into the storage-only `und` locale because its original language cannot be proven.

use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        ensure_supported_backend(backend)?;
        let connection = manager.get_connection();

        for statement in create_statements(backend)? {
            connection.execute_unprepared(statement).await?;
        }
        for statement in backfill_statements() {
            connection.execute_unprepared(statement).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        ensure_supported_backend(backend)?;
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

fn ensure_supported_backend(backend: DbBackend) -> Result<(), DbErr> {
    match backend {
        DbBackend::Postgres | DbBackend::Sqlite => Ok(()),
        _ => Err(DbErr::Migration(format!(
            "RBAC localized presentation migration does not support {backend:?}"
        ))),
    }
}

fn create_statements(backend: DbBackend) -> Result<&'static [&'static str], DbErr> {
    match backend {
        DbBackend::Postgres => Ok(&[
            r#"CREATE TABLE rbac_role_presentations (
                role_id UUID NOT NULL REFERENCES roles(id) ON UPDATE RESTRICT ON DELETE CASCADE,
                locale VARCHAR(32) NOT NULL,
                source_locale VARCHAR(32) NOT NULL,
                name TEXT NOT NULL,
                description TEXT NULL,
                copy_revision BIGINT NOT NULL DEFAULT 1 CHECK (copy_revision > 0),
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (role_id, locale),
                CHECK (length(trim(name)) > 0)
            )"#,
            r#"CREATE TABLE rbac_permission_presentations (
                permission_id UUID NOT NULL REFERENCES permissions(id) ON UPDATE RESTRICT ON DELETE CASCADE,
                locale VARCHAR(32) NOT NULL,
                source_locale VARCHAR(32) NOT NULL,
                label TEXT NULL,
                description TEXT NULL,
                copy_revision BIGINT NOT NULL DEFAULT 1 CHECK (copy_revision > 0),
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (permission_id, locale),
                CHECK (label IS NOT NULL OR description IS NOT NULL)
            )"#,
        ]),
        DbBackend::Sqlite => Ok(&[
            r#"CREATE TABLE rbac_role_presentations (
                role_id TEXT NOT NULL REFERENCES roles(id) ON UPDATE RESTRICT ON DELETE CASCADE,
                locale VARCHAR(32) NOT NULL,
                source_locale VARCHAR(32) NOT NULL,
                name TEXT NOT NULL,
                description TEXT NULL,
                copy_revision INTEGER NOT NULL DEFAULT 1 CHECK (copy_revision > 0),
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (role_id, locale),
                CHECK (length(trim(name)) > 0)
            )"#,
            r#"CREATE TABLE rbac_permission_presentations (
                permission_id TEXT NOT NULL REFERENCES permissions(id) ON UPDATE RESTRICT ON DELETE CASCADE,
                locale VARCHAR(32) NOT NULL,
                source_locale VARCHAR(32) NOT NULL,
                label TEXT NULL,
                description TEXT NULL,
                copy_revision INTEGER NOT NULL DEFAULT 1 CHECK (copy_revision > 0),
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (permission_id, locale),
                CHECK (label IS NOT NULL OR description IS NOT NULL)
            )"#,
        ]),
        _ => Err(DbErr::Migration(format!(
            "RBAC localized presentation migration does not support {backend:?}"
        ))),
    }
}

fn backfill_statements() -> [&'static str; 2] {
    [
        r#"INSERT INTO rbac_role_presentations (
                role_id, locale, source_locale, name, description,
                copy_revision, created_at, updated_at
            )
            SELECT id, 'und', 'und', name, description, 1, created_at, updated_at
            FROM roles
            ON CONFLICT (role_id, locale) DO NOTHING"#,
        r#"INSERT INTO rbac_permission_presentations (
                permission_id, locale, source_locale, label, description,
                copy_revision, created_at, updated_at
            )
            SELECT id, 'und', 'und', NULL, description, 1, created_at, created_at
            FROM permissions
            WHERE description IS NOT NULL
            ON CONFLICT (permission_id, locale) DO NOTHING"#,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_backfill_is_truthfully_storage_only_und() {
        for statement in backfill_statements() {
            assert!(statement.contains("'und', 'und'"));
            assert!(!statement.contains("'en'"));
        }
    }

    #[test]
    fn presentation_schema_has_copy_only_revisions() {
        let statements = create_statements(DbBackend::Postgres).expect("postgres schema");
        assert!(
            statements
                .iter()
                .all(|statement| statement.contains("copy_revision"))
        );
        assert!(
            statements
                .iter()
                .all(|statement| !statement.contains("invalidation_generation"))
        );
    }
}
