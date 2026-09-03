use std::collections::HashMap;

use rustok_api::normalize_locale_tag;
use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

/// Append-only cutover from locale-bearing artifact permission rows to immutable
/// definitions, owner translations, and exact tenant-safe authorization identity.
///
/// Existing grants and operation receipts are migrated only when their legacy
/// `(tenant, installation_id, permission_key)` selector resolves to exactly one admitted
/// platform-or-tenant definition. Ambiguous or orphan state fails closed instead of
/// inventing an authorization identity.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Clone, Debug, PartialEq, Eq)]
struct TranslationValue {
    label: String,
    description: String,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        ensure_supported_backend(backend)?;
        let connection = manager.get_connection();
        if backend != DbBackend::Sqlite {
            return apply_up(connection, backend).await;
        }

        let transaction = connection.begin().await?;
        match apply_up(&transaction, backend).await {
            Ok(()) => transaction.commit().await,
            Err(error) => match transaction.rollback().await {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(DbErr::Migration(format!(
                    "artifact permission migration failed: {error}; SQLite rollback failed: {rollback_error}"
                ))),
            },
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        ensure_supported_backend(backend)?;
        let connection = manager.get_connection();
        if backend != DbBackend::Sqlite {
            return apply_down(connection, backend).await;
        }

        let transaction = connection.begin().await?;
        match apply_down(&transaction, backend).await {
            Ok(()) => transaction.commit().await,
            Err(error) => match transaction.rollback().await {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(DbErr::Migration(format!(
                    "artifact permission rollback failed: {error}; SQLite rollback failed: {rollback_error}"
                ))),
            },
        }
    }
}

async fn apply_up<C>(connection: &C, backend: DbBackend) -> Result<(), DbErr>
where
    C: ConnectionTrait + ?Sized,
{
    ensure_zero(
        connection,
        backend,
        match backend {
            DbBackend::Postgres => {
                "SELECT COUNT(*) AS count FROM (SELECT installation_id FROM rbac_artifact_permission_catalog GROUP BY installation_id HAVING COUNT(DISTINCT scope_key) <> 1 OR COUNT(DISTINCT module_slug) <> 1 OR COUNT(DISTINCT release_digest) <> 1) invalid"
            }
            DbBackend::Sqlite => {
                "SELECT COUNT(*) AS count FROM (SELECT installation_id FROM rbac_artifact_permission_catalog GROUP BY installation_id HAVING COUNT(DISTINCT scope_key) <> 1 OR COUNT(DISTINCT module_slug) <> 1 OR COUNT(DISTINCT release_digest) <> 1)"
            }
            _ => unreachable!(),
        },
        "artifact permission installation identity is bound to conflicting scope or admitted metadata",
    )
    .await?;

    execute_all(
        connection,
        backend,
        match backend {
            DbBackend::Postgres => &[
                "CREATE TABLE rbac_artifact_permission_installations (installation_id UUID PRIMARY KEY, scope_key TEXT NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, registered_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (installation_id, scope_key, module_slug, release_digest))",
                "INSERT INTO rbac_artifact_permission_installations (installation_id, scope_key, module_slug, release_digest, registered_at) SELECT installation_id, MIN(scope_key), MIN(module_slug), MIN(release_digest), MIN(registered_at) FROM rbac_artifact_permission_catalog GROUP BY installation_id",
                "CREATE TABLE rbac_artifact_permission_definitions_new (id UUID PRIMARY KEY, scope_key TEXT NOT NULL, installation_id UUID NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, permission_key TEXT NOT NULL, registered_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, CONSTRAINT fk_rbac_artifact_definition_installation FOREIGN KEY (installation_id, scope_key, module_slug, release_digest) REFERENCES rbac_artifact_permission_installations (installation_id, scope_key, module_slug, release_digest) ON UPDATE RESTRICT ON DELETE RESTRICT, UNIQUE (id, scope_key), UNIQUE (scope_key, installation_id, permission_key))",
                "INSERT INTO rbac_artifact_permission_definitions_new (id, scope_key, installation_id, module_slug, release_digest, permission_key, registered_at) SELECT MIN(id::text)::uuid, scope_key, installation_id, MIN(module_slug), MIN(release_digest), permission_key, MIN(registered_at) FROM rbac_artifact_permission_catalog GROUP BY scope_key, installation_id, permission_key",
                "CREATE TABLE rbac_artifact_permission_translations_new (id UUID PRIMARY KEY, artifact_permission_id UUID NOT NULL REFERENCES rbac_artifact_permission_definitions_new (id) ON UPDATE RESTRICT ON DELETE CASCADE, locale VARCHAR(32) NOT NULL, label TEXT NOT NULL, description TEXT NOT NULL, registered_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (artifact_permission_id, locale))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE rbac_artifact_permission_installations (installation_id TEXT PRIMARY KEY, scope_key TEXT NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, registered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (installation_id, scope_key, module_slug, release_digest))",
                "INSERT INTO rbac_artifact_permission_installations (installation_id, scope_key, module_slug, release_digest, registered_at) SELECT installation_id, MIN(scope_key), MIN(module_slug), MIN(release_digest), MIN(registered_at) FROM rbac_artifact_permission_catalog GROUP BY installation_id",
                "CREATE TABLE rbac_artifact_permission_definitions_new (id TEXT PRIMARY KEY, scope_key TEXT NOT NULL, installation_id TEXT NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, permission_key TEXT NOT NULL, registered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, FOREIGN KEY (installation_id, scope_key, module_slug, release_digest) REFERENCES rbac_artifact_permission_installations (installation_id, scope_key, module_slug, release_digest) ON UPDATE RESTRICT ON DELETE RESTRICT, UNIQUE (id, scope_key), UNIQUE (scope_key, installation_id, permission_key))",
                "INSERT INTO rbac_artifact_permission_definitions_new (id, scope_key, installation_id, module_slug, release_digest, permission_key, registered_at) SELECT MIN(id), scope_key, installation_id, MIN(module_slug), MIN(release_digest), permission_key, MIN(registered_at) FROM rbac_artifact_permission_catalog GROUP BY scope_key, installation_id, permission_key",
                "CREATE TABLE rbac_artifact_permission_translations_new (id TEXT PRIMARY KEY, artifact_permission_id TEXT NOT NULL REFERENCES rbac_artifact_permission_definitions_new (id) ON UPDATE RESTRICT ON DELETE CASCADE, locale VARCHAR(32) NOT NULL, label TEXT NOT NULL, description TEXT NOT NULL, registered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (artifact_permission_id, locale))",
            ],
            _ => unreachable!(),
        },
    )
    .await?;
    backfill_translations(connection, backend).await?;

    execute_all(
        connection,
        backend,
        &[
            "DROP INDEX rbac_artifact_permission_catalog_lookup_idx",
            "DROP TABLE rbac_artifact_permission_catalog",
            "ALTER TABLE rbac_artifact_permission_definitions_new RENAME TO rbac_artifact_permission_definitions",
            "ALTER TABLE rbac_artifact_permission_translations_new RENAME TO rbac_artifact_permission_translations",
            "CREATE INDEX rbac_artifact_permission_definitions_lookup_idx ON rbac_artifact_permission_definitions (scope_key, module_slug, permission_key)",
        ],
    )
    .await?;
    execute_all(
        connection,
        backend,
        match backend {
            DbBackend::Postgres => &[
                "CREATE OR REPLACE FUNCTION rustok_reject_artifact_permission_installation_update() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'artifact permission installation identities are immutable'; END; $$",
                "CREATE TRIGGER rbac_artifact_permission_installations_immutable BEFORE UPDATE ON rbac_artifact_permission_installations FOR EACH ROW EXECUTE FUNCTION rustok_reject_artifact_permission_installation_update()",
                "CREATE OR REPLACE FUNCTION rustok_reject_artifact_permission_definition_update() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'artifact permission definitions are immutable'; END; $$",
                "CREATE TRIGGER rbac_artifact_permission_definitions_immutable BEFORE UPDATE ON rbac_artifact_permission_definitions FOR EACH ROW EXECUTE FUNCTION rustok_reject_artifact_permission_definition_update()",
            ],
            DbBackend::Sqlite => &[
                "CREATE TRIGGER rbac_artifact_permission_installations_immutable BEFORE UPDATE ON rbac_artifact_permission_installations BEGIN SELECT RAISE(ABORT, 'artifact permission installation identities are immutable'); END",
                "CREATE TRIGGER rbac_artifact_permission_definitions_immutable BEFORE UPDATE ON rbac_artifact_permission_definitions BEGIN SELECT RAISE(ABORT, 'artifact permission definitions are immutable'); END",
            ],
            _ => unreachable!(),
        },
    )
    .await?;

    execute_all(
        connection,
        backend,
        &[
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_rbac_roles_tenant_id_id ON roles (tenant_id, id)",
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_rbac_users_tenant_id_id ON users (tenant_id, id)",
        ],
    )
    .await?;
    validate_legacy_authorization_rows(connection, backend).await?;

    execute_all(
        connection,
        backend,
        match backend {
            DbBackend::Postgres => &[
                "CREATE TABLE rbac_artifact_role_permissions_new (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, role_id UUID NOT NULL, artifact_permission_id UUID NOT NULL, permission_scope_key TEXT NOT NULL, granted_by_actor_id UUID NOT NULL, granted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, CONSTRAINT ck_rbac_artifact_grant_permission_scope CHECK (permission_scope_key = 'platform' OR permission_scope_key = 'tenant:' || tenant_id::text), CONSTRAINT fk_rbac_artifact_grant_role FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, CONSTRAINT fk_rbac_artifact_grant_actor FOREIGN KEY (tenant_id, granted_by_actor_id) REFERENCES users (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, CONSTRAINT fk_rbac_artifact_grant_permission FOREIGN KEY (artifact_permission_id, permission_scope_key) REFERENCES rbac_artifact_permission_definitions (id, scope_key) ON UPDATE RESTRICT ON DELETE RESTRICT, UNIQUE (tenant_id, role_id, artifact_permission_id))",
                "INSERT INTO rbac_artifact_role_permissions_new (id, tenant_id, role_id, artifact_permission_id, permission_scope_key, granted_by_actor_id, granted_at) SELECT legacy.id, legacy.tenant_id, legacy.role_id, definition.id, definition.scope_key, legacy.granted_by_actor_id, legacy.granted_at FROM rbac_artifact_role_permissions legacy JOIN rbac_artifact_permission_definitions definition ON definition.installation_id = legacy.installation_id AND definition.permission_key = legacy.permission_key AND (definition.scope_key = 'platform' OR definition.scope_key = 'tenant:' || legacy.tenant_id::text)",
                "CREATE TABLE rbac_artifact_role_permission_operations_new (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, idempotency_key TEXT NOT NULL, role_id UUID NOT NULL, artifact_permission_id UUID NOT NULL, permission_scope_key TEXT NOT NULL, actor_id UUID NOT NULL, granted BOOLEAN NOT NULL, applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, CONSTRAINT ck_rbac_artifact_operation_permission_scope CHECK (permission_scope_key = 'platform' OR permission_scope_key = 'tenant:' || tenant_id::text), CONSTRAINT fk_rbac_artifact_operation_role FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, CONSTRAINT fk_rbac_artifact_operation_actor FOREIGN KEY (tenant_id, actor_id) REFERENCES users (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, CONSTRAINT fk_rbac_artifact_operation_permission FOREIGN KEY (artifact_permission_id, permission_scope_key) REFERENCES rbac_artifact_permission_definitions (id, scope_key) ON UPDATE RESTRICT ON DELETE RESTRICT, UNIQUE (tenant_id, idempotency_key))",
                "INSERT INTO rbac_artifact_role_permission_operations_new (id, tenant_id, idempotency_key, role_id, artifact_permission_id, permission_scope_key, actor_id, granted, applied_at) SELECT legacy.id, legacy.tenant_id, legacy.idempotency_key, legacy.role_id, definition.id, definition.scope_key, legacy.actor_id, legacy.granted, legacy.applied_at FROM rbac_artifact_role_permission_operations legacy JOIN rbac_artifact_permission_definitions definition ON definition.installation_id = legacy.installation_id AND definition.permission_key = legacy.permission_key AND (definition.scope_key = 'platform' OR definition.scope_key = 'tenant:' || legacy.tenant_id::text)",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE rbac_artifact_role_permissions_new (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, role_id TEXT NOT NULL, artifact_permission_id TEXT NOT NULL, permission_scope_key TEXT NOT NULL, granted_by_actor_id TEXT NOT NULL, granted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, CHECK (permission_scope_key = 'platform' OR permission_scope_key = 'tenant:' || tenant_id), FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, FOREIGN KEY (tenant_id, granted_by_actor_id) REFERENCES users (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, FOREIGN KEY (artifact_permission_id, permission_scope_key) REFERENCES rbac_artifact_permission_definitions (id, scope_key) ON UPDATE RESTRICT ON DELETE RESTRICT, UNIQUE (tenant_id, role_id, artifact_permission_id))",
                "INSERT INTO rbac_artifact_role_permissions_new (id, tenant_id, role_id, artifact_permission_id, permission_scope_key, granted_by_actor_id, granted_at) SELECT legacy.id, legacy.tenant_id, legacy.role_id, definition.id, definition.scope_key, legacy.granted_by_actor_id, legacy.granted_at FROM rbac_artifact_role_permissions legacy JOIN rbac_artifact_permission_definitions definition ON definition.installation_id = legacy.installation_id AND definition.permission_key = legacy.permission_key AND (definition.scope_key = 'platform' OR definition.scope_key = 'tenant:' || legacy.tenant_id)",
                "CREATE TABLE rbac_artifact_role_permission_operations_new (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, role_id TEXT NOT NULL, artifact_permission_id TEXT NOT NULL, permission_scope_key TEXT NOT NULL, actor_id TEXT NOT NULL, granted BOOLEAN NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, CHECK (permission_scope_key = 'platform' OR permission_scope_key = 'tenant:' || tenant_id), FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, FOREIGN KEY (tenant_id, actor_id) REFERENCES users (tenant_id, id) ON UPDATE RESTRICT ON DELETE RESTRICT, FOREIGN KEY (artifact_permission_id, permission_scope_key) REFERENCES rbac_artifact_permission_definitions (id, scope_key) ON UPDATE RESTRICT ON DELETE RESTRICT, UNIQUE (tenant_id, idempotency_key))",
                "INSERT INTO rbac_artifact_role_permission_operations_new (id, tenant_id, idempotency_key, role_id, artifact_permission_id, permission_scope_key, actor_id, granted, applied_at) SELECT legacy.id, legacy.tenant_id, legacy.idempotency_key, legacy.role_id, definition.id, definition.scope_key, legacy.actor_id, legacy.granted, legacy.applied_at FROM rbac_artifact_role_permission_operations legacy JOIN rbac_artifact_permission_definitions definition ON definition.installation_id = legacy.installation_id AND definition.permission_key = legacy.permission_key AND (definition.scope_key = 'platform' OR definition.scope_key = 'tenant:' || legacy.tenant_id)",
            ],
            _ => unreachable!(),
        },
    )
    .await?;
    execute_all(
        connection,
        backend,
        &[
            "DROP TABLE rbac_artifact_role_permission_operations",
            "DROP INDEX rbac_artifact_role_permissions_authorize_idx",
            "DROP TABLE rbac_artifact_role_permissions",
            "ALTER TABLE rbac_artifact_role_permissions_new RENAME TO rbac_artifact_role_permissions",
            "ALTER TABLE rbac_artifact_role_permission_operations_new RENAME TO rbac_artifact_role_permission_operations",
            "CREATE INDEX rbac_artifact_role_permissions_authorize_idx ON rbac_artifact_role_permissions (tenant_id, role_id, artifact_permission_id)",
        ],
    )
    .await?;
    Ok(())
}

async fn apply_down<C>(connection: &C, backend: DbBackend) -> Result<(), DbErr>
where
    C: ConnectionTrait + ?Sized,
{
    ensure_zero(
        connection,
        backend,
        "SELECT COUNT(*) AS count FROM rbac_artifact_permission_definitions definition WHERE NOT EXISTS (SELECT 1 FROM rbac_artifact_permission_translations translation WHERE translation.artifact_permission_id = definition.id)",
        "cannot roll back an artifact permission definition without localized catalog copy",
    )
    .await?;
    ensure_zero(
        connection,
        backend,
        match backend {
            DbBackend::Postgres => {
                "SELECT COUNT(*) AS count FROM (SELECT grant_row.tenant_id, grant_row.role_id, definition.installation_id, definition.permission_key FROM rbac_artifact_role_permissions grant_row JOIN rbac_artifact_permission_definitions definition ON definition.id = grant_row.artifact_permission_id AND definition.scope_key = grant_row.permission_scope_key GROUP BY grant_row.tenant_id, grant_row.role_id, definition.installation_id, definition.permission_key HAVING COUNT(*) > 1) invalid"
            }
            DbBackend::Sqlite => {
                "SELECT COUNT(*) AS count FROM (SELECT grant_row.tenant_id, grant_row.role_id, definition.installation_id, definition.permission_key FROM rbac_artifact_role_permissions grant_row JOIN rbac_artifact_permission_definitions definition ON definition.id = grant_row.artifact_permission_id AND definition.scope_key = grant_row.permission_scope_key GROUP BY grant_row.tenant_id, grant_row.role_id, definition.installation_id, definition.permission_key HAVING COUNT(*) > 1)"
            }
            _ => unreachable!(),
        },
        "cannot roll back distinct scoped grants that collapse to one legacy key",
    )
    .await?;
    validate_rollback_legacy_selectors(connection, backend).await?;

    execute_all(
        connection,
        backend,
        match backend {
            DbBackend::Postgres => &[
                "CREATE TABLE rbac_artifact_permission_catalog_restore (id UUID PRIMARY KEY, scope_key TEXT NOT NULL, installation_id UUID NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, permission_key TEXT NOT NULL, locale TEXT NOT NULL, label TEXT NOT NULL, description TEXT NOT NULL, registered_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (scope_key, installation_id, permission_key, locale))",
                "INSERT INTO rbac_artifact_permission_catalog_restore (id, scope_key, installation_id, module_slug, release_digest, permission_key, locale, label, description, registered_at) SELECT translation.id, definition.scope_key, definition.installation_id, definition.module_slug, definition.release_digest, definition.permission_key, translation.locale, translation.label, translation.description, translation.registered_at FROM rbac_artifact_permission_definitions definition JOIN rbac_artifact_permission_translations translation ON translation.artifact_permission_id = definition.id",
                "CREATE TABLE rbac_artifact_role_permissions_restore (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, role_id UUID NOT NULL, installation_id UUID NOT NULL, permission_key TEXT NOT NULL, granted_by_actor_id UUID NOT NULL, granted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, role_id, installation_id, permission_key))",
                "INSERT INTO rbac_artifact_role_permissions_restore (id, tenant_id, role_id, installation_id, permission_key, granted_by_actor_id, granted_at) SELECT grant_row.id, grant_row.tenant_id, grant_row.role_id, definition.installation_id, definition.permission_key, grant_row.granted_by_actor_id, grant_row.granted_at FROM rbac_artifact_role_permissions grant_row JOIN rbac_artifact_permission_definitions definition ON definition.id = grant_row.artifact_permission_id AND definition.scope_key = grant_row.permission_scope_key",
                "CREATE TABLE rbac_artifact_role_permission_operations_restore (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, idempotency_key TEXT NOT NULL, role_id UUID NOT NULL, installation_id UUID NOT NULL, permission_key TEXT NOT NULL, actor_id UUID NOT NULL, granted BOOLEAN NOT NULL, applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, idempotency_key))",
                "INSERT INTO rbac_artifact_role_permission_operations_restore (id, tenant_id, idempotency_key, role_id, installation_id, permission_key, actor_id, granted, applied_at) SELECT operation.id, operation.tenant_id, operation.idempotency_key, operation.role_id, definition.installation_id, definition.permission_key, operation.actor_id, operation.granted, operation.applied_at FROM rbac_artifact_role_permission_operations operation JOIN rbac_artifact_permission_definitions definition ON definition.id = operation.artifact_permission_id AND definition.scope_key = operation.permission_scope_key",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE rbac_artifact_permission_catalog_restore (id TEXT PRIMARY KEY, scope_key TEXT NOT NULL, installation_id TEXT NOT NULL, module_slug TEXT NOT NULL, release_digest TEXT NOT NULL, permission_key TEXT NOT NULL, locale TEXT NOT NULL, label TEXT NOT NULL, description TEXT NOT NULL, registered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (scope_key, installation_id, permission_key, locale))",
                "INSERT INTO rbac_artifact_permission_catalog_restore (id, scope_key, installation_id, module_slug, release_digest, permission_key, locale, label, description, registered_at) SELECT translation.id, definition.scope_key, definition.installation_id, definition.module_slug, definition.release_digest, definition.permission_key, translation.locale, translation.label, translation.description, translation.registered_at FROM rbac_artifact_permission_definitions definition JOIN rbac_artifact_permission_translations translation ON translation.artifact_permission_id = definition.id",
                "CREATE TABLE rbac_artifact_role_permissions_restore (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, role_id TEXT NOT NULL, installation_id TEXT NOT NULL, permission_key TEXT NOT NULL, granted_by_actor_id TEXT NOT NULL, granted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, role_id, installation_id, permission_key))",
                "INSERT INTO rbac_artifact_role_permissions_restore (id, tenant_id, role_id, installation_id, permission_key, granted_by_actor_id, granted_at) SELECT grant_row.id, grant_row.tenant_id, grant_row.role_id, definition.installation_id, definition.permission_key, grant_row.granted_by_actor_id, grant_row.granted_at FROM rbac_artifact_role_permissions grant_row JOIN rbac_artifact_permission_definitions definition ON definition.id = grant_row.artifact_permission_id AND definition.scope_key = grant_row.permission_scope_key",
                "CREATE TABLE rbac_artifact_role_permission_operations_restore (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, role_id TEXT NOT NULL, installation_id TEXT NOT NULL, permission_key TEXT NOT NULL, actor_id TEXT NOT NULL, granted BOOLEAN NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, idempotency_key))",
                "INSERT INTO rbac_artifact_role_permission_operations_restore (id, tenant_id, idempotency_key, role_id, installation_id, permission_key, actor_id, granted, applied_at) SELECT operation.id, operation.tenant_id, operation.idempotency_key, operation.role_id, definition.installation_id, definition.permission_key, operation.actor_id, operation.granted, operation.applied_at FROM rbac_artifact_role_permission_operations operation JOIN rbac_artifact_permission_definitions definition ON definition.id = operation.artifact_permission_id AND definition.scope_key = operation.permission_scope_key",
            ],
            _ => unreachable!(),
        },
    )
    .await?;

    execute_all(
        connection,
        backend,
        &[
            "DROP TABLE rbac_artifact_role_permission_operations",
            "DROP INDEX rbac_artifact_role_permissions_authorize_idx",
            "DROP TABLE rbac_artifact_role_permissions",
            "ALTER TABLE rbac_artifact_role_permissions_restore RENAME TO rbac_artifact_role_permissions",
            "ALTER TABLE rbac_artifact_role_permission_operations_restore RENAME TO rbac_artifact_role_permission_operations",
            "CREATE INDEX rbac_artifact_role_permissions_authorize_idx ON rbac_artifact_role_permissions (tenant_id, role_id, installation_id, permission_key)",
        ],
    )
    .await?;

    execute_all(
        connection,
        backend,
        match backend {
            DbBackend::Postgres => &[
                "DROP TRIGGER rbac_artifact_permission_definitions_immutable ON rbac_artifact_permission_definitions",
                "DROP TRIGGER rbac_artifact_permission_installations_immutable ON rbac_artifact_permission_installations",
                "DROP TABLE rbac_artifact_permission_translations",
                "DROP INDEX rbac_artifact_permission_definitions_lookup_idx",
                "DROP TABLE rbac_artifact_permission_definitions",
                "DROP TABLE rbac_artifact_permission_installations",
                "DROP FUNCTION rustok_reject_artifact_permission_definition_update()",
                "DROP FUNCTION rustok_reject_artifact_permission_installation_update()",
            ],
            DbBackend::Sqlite => &[
                "DROP TRIGGER rbac_artifact_permission_definitions_immutable",
                "DROP TRIGGER rbac_artifact_permission_installations_immutable",
                "DROP TABLE rbac_artifact_permission_translations",
                "DROP INDEX rbac_artifact_permission_definitions_lookup_idx",
                "DROP TABLE rbac_artifact_permission_definitions",
                "DROP TABLE rbac_artifact_permission_installations",
            ],
            _ => unreachable!(),
        },
    )
    .await?;
    execute_all(
        connection,
        backend,
        &[
            "ALTER TABLE rbac_artifact_permission_catalog_restore RENAME TO rbac_artifact_permission_catalog",
            "CREATE INDEX rbac_artifact_permission_catalog_lookup_idx ON rbac_artifact_permission_catalog (scope_key, module_slug, permission_key)",
            "DROP INDEX IF EXISTS uq_rbac_roles_tenant_id_id",
            "DROP INDEX IF EXISTS uq_rbac_users_tenant_id_id",
        ],
    )
    .await?;
    Ok(())
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), DbErr> {
    match backend {
        DbBackend::Postgres | DbBackend::Sqlite => Ok(()),
        backend => Err(DbErr::Migration(format!(
            "artifact permission canonicalization does not support {backend:?}"
        ))),
    }
}

async fn execute_all<C>(
    connection: &C,
    backend: DbBackend,
    statements: &[&str],
) -> Result<(), DbErr>
where
    C: ConnectionTrait + ?Sized,
{
    for statement in statements {
        connection
            .execute_raw(Statement::from_string(backend, (*statement).to_string()))
            .await?;
    }
    Ok(())
}

async fn ensure_zero<C>(
    connection: &C,
    backend: DbBackend,
    sql: &str,
    message: &str,
) -> Result<(), DbErr>
where
    C: ConnectionTrait + ?Sized,
{
    let row = connection
        .query_one_raw(Statement::from_string(backend, sql.to_string()))
        .await?
        .ok_or_else(|| DbErr::Migration("artifact permission validation returned no row".into()))?;
    let count: i64 = row.try_get("", "count")?;
    if count == 0 {
        Ok(())
    } else {
        Err(DbErr::Migration(format!(
            "{message}: {count} invalid row set(s)"
        )))
    }
}

async fn backfill_translations<C>(connection: &C, backend: DbBackend) -> Result<(), DbErr>
where
    C: ConnectionTrait + ?Sized,
{
    let select = match backend {
        DbBackend::Postgres => {
            "SELECT catalog.id::text AS translation_id, definition.id::text AS definition_id, catalog.locale, catalog.label, catalog.description, catalog.registered_at::text AS registered_at_text FROM rbac_artifact_permission_catalog catalog JOIN rbac_artifact_permission_definitions_new definition ON definition.scope_key = catalog.scope_key AND definition.installation_id = catalog.installation_id AND definition.permission_key = catalog.permission_key ORDER BY catalog.registered_at, catalog.id::text"
        }
        DbBackend::Sqlite => {
            "SELECT catalog.id AS translation_id, definition.id AS definition_id, catalog.locale, catalog.label, catalog.description, catalog.registered_at AS registered_at_text FROM rbac_artifact_permission_catalog catalog JOIN rbac_artifact_permission_definitions_new definition ON definition.scope_key = catalog.scope_key AND definition.installation_id = catalog.installation_id AND definition.permission_key = catalog.permission_key ORDER BY catalog.registered_at, catalog.id"
        }
        _ => unreachable!(),
    };
    let rows = connection
        .query_all_raw(Statement::from_string(backend, select.to_string()))
        .await?;
    let mut normalized: HashMap<(Uuid, String), TranslationValue> = HashMap::new();
    for row in rows {
        let translation_id_text: String = row.try_get("", "translation_id")?;
        let translation_id = Uuid::parse_str(&translation_id_text).map_err(|error| {
            DbErr::Migration(format!(
                "artifact permission translation id `{translation_id_text}` is invalid: {error}"
            ))
        })?;
        let definition_id_text: String = row.try_get("", "definition_id")?;
        let definition_id = Uuid::parse_str(&definition_id_text).map_err(|error| {
            DbErr::Migration(format!(
                "artifact permission definition id `{definition_id_text}` is invalid: {error}"
            ))
        })?;
        let raw_locale: String = row.try_get("", "locale")?;
        let locale = normalize_locale_tag(&raw_locale).ok_or_else(|| {
            DbErr::Migration(format!(
                "artifact permission locale `{raw_locale}` cannot be normalized"
            ))
        })?;
        let value = TranslationValue {
            label: row.try_get("", "label")?,
            description: row.try_get("", "description")?,
        };
        let key = (definition_id, locale.clone());
        if let Some(existing) = normalized.get(&key) {
            if existing != &value {
                return Err(DbErr::Migration(format!(
                    "artifact permission definition {definition_id} has conflicting copy for canonical locale {locale}"
                )));
            }
            continue;
        }
        let registered_at: String = row.try_get("", "registered_at_text")?;
        let sql = match backend {
            DbBackend::Postgres => {
                "INSERT INTO rbac_artifact_permission_translations_new (id, artifact_permission_id, locale, label, description, registered_at) VALUES ($1, $2, $3, $4, $5, $6::timestamptz)"
            }
            DbBackend::Sqlite => {
                "INSERT INTO rbac_artifact_permission_translations_new (id, artifact_permission_id, locale, label, description, registered_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
            }
            _ => unreachable!(),
        };
        let (translation_id_value, definition_id_value): (sea_orm::Value, sea_orm::Value) =
            match backend {
                DbBackend::Postgres => (translation_id.into(), definition_id.into()),
                DbBackend::Sqlite => (
                    translation_id.to_string().into(),
                    definition_id.to_string().into(),
                ),
                _ => unreachable!(),
            };
        connection
            .execute_raw(Statement::from_sql_and_values(
                backend,
                sql,
                vec![
                    translation_id_value,
                    definition_id_value,
                    locale.into(),
                    value.label.clone().into(),
                    value.description.clone().into(),
                    registered_at.into(),
                ],
            ))
            .await?;
        normalized.insert(key, value);
    }
    Ok(())
}

async fn validate_legacy_authorization_rows<C>(
    connection: &C,
    backend: DbBackend,
) -> Result<(), DbErr>
where
    C: ConnectionTrait + ?Sized,
{
    let tenant_expression = match backend {
        DbBackend::Postgres => "'tenant:' || legacy.tenant_id::text",
        DbBackend::Sqlite => "'tenant:' || legacy.tenant_id",
        _ => unreachable!(),
    };
    for (table, actor_column, label) in [
        (
            "rbac_artifact_role_permissions",
            "granted_by_actor_id",
            "grant",
        ),
        (
            "rbac_artifact_role_permission_operations",
            "actor_id",
            "operation receipt",
        ),
    ] {
        let sql = format!(
            "SELECT COUNT(*) AS count FROM {table} legacy WHERE (SELECT COUNT(*) FROM rbac_artifact_permission_definitions definition WHERE definition.installation_id = legacy.installation_id AND definition.permission_key = legacy.permission_key AND (definition.scope_key = 'platform' OR definition.scope_key = {tenant_expression})) <> 1 OR NOT EXISTS (SELECT 1 FROM roles role_row WHERE role_row.tenant_id = legacy.tenant_id AND role_row.id = legacy.role_id) OR NOT EXISTS (SELECT 1 FROM users actor_row WHERE actor_row.tenant_id = legacy.tenant_id AND actor_row.id = legacy.{actor_column})"
        );
        ensure_zero(
            connection,
            backend,
            &sql,
            &format!("legacy artifact permission {label} has ambiguous or orphan identity"),
        )
        .await?;
    }
    Ok(())
}

async fn validate_rollback_legacy_selectors<C>(
    connection: &C,
    backend: DbBackend,
) -> Result<(), DbErr>
where
    C: ConnectionTrait + ?Sized,
{
    let tenant_expression = match backend {
        DbBackend::Postgres => "'tenant:' || authorization_row.tenant_id::text",
        DbBackend::Sqlite => "'tenant:' || authorization_row.tenant_id",
        _ => unreachable!(),
    };
    for (table, label) in [
        ("rbac_artifact_role_permissions", "grant"),
        (
            "rbac_artifact_role_permission_operations",
            "operation receipt",
        ),
    ] {
        let sql = format!(
            "SELECT COUNT(*) AS count FROM {table} authorization_row WHERE (SELECT COUNT(*) FROM rbac_artifact_permission_definitions selected_definition JOIN rbac_artifact_permission_definitions candidate_definition ON candidate_definition.installation_id = selected_definition.installation_id AND candidate_definition.permission_key = selected_definition.permission_key AND (candidate_definition.scope_key = 'platform' OR candidate_definition.scope_key = {tenant_expression}) WHERE selected_definition.id = authorization_row.artifact_permission_id AND selected_definition.scope_key = authorization_row.permission_scope_key) <> 1"
        );
        ensure_zero(
            connection,
            backend,
            &sql,
            &format!("cannot roll back artifact permission {label} with ambiguous legacy selector"),
        )
        .await?;
    }
    Ok(())
}
