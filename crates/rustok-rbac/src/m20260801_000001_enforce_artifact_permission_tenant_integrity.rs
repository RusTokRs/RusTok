//! Enforces database-level tenant and parent integrity for dynamic artifact grants.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "RBAC artifact permission tenant-integrity migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "RBAC artifact permission tenant-integrity migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DELETE FROM rbac_artifact_role_permissions grant_row
WHERE NOT EXISTS (
    SELECT 1 FROM roles role_row
    WHERE role_row.id = grant_row.role_id
      AND role_row.tenant_id = grant_row.tenant_id
) OR NOT EXISTS (
    SELECT 1 FROM users actor_row
    WHERE actor_row.id = grant_row.granted_by_actor_id
      AND actor_row.tenant_id = grant_row.tenant_id
) OR NOT EXISTS (
    SELECT 1 FROM rbac_artifact_permission_catalog catalog_row
    WHERE catalog_row.installation_id = grant_row.installation_id
      AND catalog_row.permission_key = grant_row.permission_key
      AND (
          catalog_row.scope_key = 'platform'
          OR catalog_row.scope_key = 'tenant:' || grant_row.tenant_id::text
      )
);

DELETE FROM rbac_artifact_role_permission_operations operation_row
WHERE NOT EXISTS (
    SELECT 1 FROM roles role_row
    WHERE role_row.id = operation_row.role_id
      AND role_row.tenant_id = operation_row.tenant_id
) OR NOT EXISTS (
    SELECT 1 FROM users actor_row
    WHERE actor_row.id = operation_row.actor_id
      AND actor_row.tenant_id = operation_row.tenant_id
) OR NOT EXISTS (
    SELECT 1 FROM rbac_artifact_permission_catalog catalog_row
    WHERE catalog_row.installation_id = operation_row.installation_id
      AND catalog_row.permission_key = operation_row.permission_key
      AND (
          catalog_row.scope_key = 'platform'
          OR catalog_row.scope_key = 'tenant:' || operation_row.tenant_id::text
      )
);

CREATE INDEX IF NOT EXISTS idx_rbac_artifact_role_permissions_role_id
    ON rbac_artifact_role_permissions (role_id);
CREATE INDEX IF NOT EXISTS idx_rbac_artifact_role_permissions_actor_id
    ON rbac_artifact_role_permissions (granted_by_actor_id);
CREATE INDEX IF NOT EXISTS idx_rbac_artifact_role_permission_operations_role_id
    ON rbac_artifact_role_permission_operations (role_id);
CREATE INDEX IF NOT EXISTS idx_rbac_artifact_role_permission_operations_actor_id
    ON rbac_artifact_role_permission_operations (actor_id);

CREATE OR REPLACE FUNCTION rustok_enforce_artifact_role_permission_integrity()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM roles
        WHERE id = NEW.role_id AND tenant_id = NEW.tenant_id
    ) THEN
        RAISE EXCEPTION 'RBAC artifact grant role tenant mismatch'
            USING ERRCODE = '23514';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM users
        WHERE id = NEW.granted_by_actor_id AND tenant_id = NEW.tenant_id
    ) THEN
        RAISE EXCEPTION 'RBAC artifact grant actor tenant mismatch'
            USING ERRCODE = '23514';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM rbac_artifact_permission_catalog
        WHERE installation_id = NEW.installation_id
          AND permission_key = NEW.permission_key
          AND (scope_key = 'platform' OR scope_key = 'tenant:' || NEW.tenant_id::text)
    ) THEN
        RAISE EXCEPTION 'RBAC artifact grant permission is not admitted for tenant scope'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_rbac_artifact_role_permissions_integrity
    ON rbac_artifact_role_permissions;
CREATE TRIGGER trg_rbac_artifact_role_permissions_integrity
BEFORE INSERT OR UPDATE OF tenant_id, role_id, installation_id, permission_key, granted_by_actor_id
ON rbac_artifact_role_permissions
FOR EACH ROW EXECUTE FUNCTION rustok_enforce_artifact_role_permission_integrity();

CREATE OR REPLACE FUNCTION rustok_enforce_artifact_role_permission_operation_integrity()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM roles
        WHERE id = NEW.role_id AND tenant_id = NEW.tenant_id
    ) THEN
        RAISE EXCEPTION 'RBAC artifact operation role tenant mismatch'
            USING ERRCODE = '23514';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM users
        WHERE id = NEW.actor_id AND tenant_id = NEW.tenant_id
    ) THEN
        RAISE EXCEPTION 'RBAC artifact operation actor tenant mismatch'
            USING ERRCODE = '23514';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM rbac_artifact_permission_catalog
        WHERE installation_id = NEW.installation_id
          AND permission_key = NEW.permission_key
          AND (scope_key = 'platform' OR scope_key = 'tenant:' || NEW.tenant_id::text)
    ) THEN
        RAISE EXCEPTION 'RBAC artifact operation permission is not admitted for tenant scope'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_rbac_artifact_role_permission_operations_integrity
    ON rbac_artifact_role_permission_operations;
CREATE TRIGGER trg_rbac_artifact_role_permission_operations_integrity
BEFORE INSERT OR UPDATE OF tenant_id, role_id, installation_id, permission_key, actor_id
ON rbac_artifact_role_permission_operations
FOR EACH ROW EXECUTE FUNCTION rustok_enforce_artifact_role_permission_operation_integrity();

CREATE OR REPLACE FUNCTION rustok_guard_user_tenant_update()
RETURNS trigger AS $$
BEGIN
    IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id AND (
        EXISTS (
            SELECT 1 FROM user_roles ur
            JOIN roles r ON r.id = ur.role_id
            WHERE ur.user_id = NEW.id AND r.tenant_id <> NEW.tenant_id
        ) OR EXISTS (
            SELECT 1 FROM rbac_artifact_role_permissions
            WHERE granted_by_actor_id = NEW.id AND tenant_id <> NEW.tenant_id
        ) OR EXISTS (
            SELECT 1 FROM rbac_artifact_role_permission_operations
            WHERE actor_id = NEW.id AND tenant_id <> NEW.tenant_id
        )
    ) THEN
        RAISE EXCEPTION 'RBAC user tenant update would invalidate authorization relations'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION rustok_guard_role_tenant_update()
RETURNS trigger AS $$
BEGIN
    IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id AND (
        EXISTS (
            SELECT 1 FROM user_roles ur
            JOIN users u ON u.id = ur.user_id
            WHERE ur.role_id = NEW.id AND u.tenant_id <> NEW.tenant_id
        ) OR EXISTS (
            SELECT 1 FROM role_permissions rp
            JOIN permissions p ON p.id = rp.permission_id
            WHERE rp.role_id = NEW.id AND p.tenant_id <> NEW.tenant_id
        ) OR EXISTS (
            SELECT 1 FROM rbac_artifact_role_permissions
            WHERE role_id = NEW.id AND tenant_id <> NEW.tenant_id
        ) OR EXISTS (
            SELECT 1 FROM rbac_artifact_role_permission_operations
            WHERE role_id = NEW.id AND tenant_id <> NEW.tenant_id
        )
    ) THEN
        RAISE EXCEPTION 'RBAC role tenant update would invalidate authorization relations'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION rustok_guard_artifact_role_delete()
RETURNS trigger AS $$
BEGIN
    IF EXISTS (SELECT 1 FROM rbac_artifact_role_permissions WHERE role_id = OLD.id)
       OR EXISTS (SELECT 1 FROM rbac_artifact_role_permission_operations WHERE role_id = OLD.id)
    THEN
        RAISE EXCEPTION 'RBAC role deletion would orphan artifact authorization state'
            USING ERRCODE = '23514';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_rbac_artifact_role_delete ON roles;
CREATE TRIGGER trg_rbac_artifact_role_delete
BEFORE DELETE ON roles
FOR EACH ROW EXECUTE FUNCTION rustok_guard_artifact_role_delete();

CREATE OR REPLACE FUNCTION rustok_guard_artifact_actor_delete()
RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM rbac_artifact_role_permissions WHERE granted_by_actor_id = OLD.id
    ) OR EXISTS (
        SELECT 1 FROM rbac_artifact_role_permission_operations WHERE actor_id = OLD.id
    ) THEN
        RAISE EXCEPTION 'RBAC user deletion would orphan artifact authorization audit state'
            USING ERRCODE = '23514';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_rbac_artifact_actor_delete ON users;
CREATE TRIGGER trg_rbac_artifact_actor_delete
BEFORE DELETE ON users
FOR EACH ROW EXECUTE FUNCTION rustok_guard_artifact_actor_delete();

CREATE OR REPLACE FUNCTION rustok_guard_artifact_permission_catalog_identity()
RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM (
            SELECT tenant_id, installation_id, permission_key
            FROM rbac_artifact_role_permissions
            UNION
            SELECT tenant_id, installation_id, permission_key
            FROM rbac_artifact_role_permission_operations
        ) reference_row
        WHERE reference_row.installation_id = OLD.installation_id
          AND reference_row.permission_key = OLD.permission_key
          AND (OLD.scope_key = 'platform' OR OLD.scope_key = 'tenant:' || reference_row.tenant_id::text)
    ) THEN
        RAISE EXCEPTION 'RBAC referenced artifact permission identity is immutable'
            USING ERRCODE = '23514';
    END IF;
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_rbac_artifact_permission_catalog_identity_update
    ON rbac_artifact_permission_catalog;
CREATE TRIGGER trg_rbac_artifact_permission_catalog_identity_update
BEFORE UPDATE OF scope_key, installation_id, permission_key
ON rbac_artifact_permission_catalog
FOR EACH ROW
WHEN (
    OLD.scope_key IS DISTINCT FROM NEW.scope_key
    OR OLD.installation_id IS DISTINCT FROM NEW.installation_id
    OR OLD.permission_key IS DISTINCT FROM NEW.permission_key
)
EXECUTE FUNCTION rustok_guard_artifact_permission_catalog_identity();

DROP TRIGGER IF EXISTS trg_rbac_artifact_permission_catalog_delete
    ON rbac_artifact_permission_catalog;
CREATE TRIGGER trg_rbac_artifact_permission_catalog_delete
BEFORE DELETE ON rbac_artifact_permission_catalog
FOR EACH ROW EXECUTE FUNCTION rustok_guard_artifact_permission_catalog_identity();
"#,
        )
        .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TRIGGER IF EXISTS trg_rbac_artifact_role_permissions_integrity
    ON rbac_artifact_role_permissions;
DROP TRIGGER IF EXISTS trg_rbac_artifact_role_permission_operations_integrity
    ON rbac_artifact_role_permission_operations;
DROP TRIGGER IF EXISTS trg_rbac_artifact_role_delete ON roles;
DROP TRIGGER IF EXISTS trg_rbac_artifact_actor_delete ON users;
DROP TRIGGER IF EXISTS trg_rbac_artifact_permission_catalog_identity_update
    ON rbac_artifact_permission_catalog;
DROP TRIGGER IF EXISTS trg_rbac_artifact_permission_catalog_delete
    ON rbac_artifact_permission_catalog;

DROP FUNCTION IF EXISTS rustok_enforce_artifact_role_permission_integrity();
DROP FUNCTION IF EXISTS rustok_enforce_artifact_role_permission_operation_integrity();
DROP FUNCTION IF EXISTS rustok_guard_artifact_role_delete();
DROP FUNCTION IF EXISTS rustok_guard_artifact_actor_delete();
DROP FUNCTION IF EXISTS rustok_guard_artifact_permission_catalog_identity();

DROP INDEX IF EXISTS idx_rbac_artifact_role_permissions_role_id;
DROP INDEX IF EXISTS idx_rbac_artifact_role_permissions_actor_id;
DROP INDEX IF EXISTS idx_rbac_artifact_role_permission_operations_role_id;
DROP INDEX IF EXISTS idx_rbac_artifact_role_permission_operations_actor_id;

CREATE OR REPLACE FUNCTION rustok_guard_user_tenant_update()
RETURNS trigger AS $$
BEGIN
    IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id AND EXISTS (
        SELECT 1 FROM user_roles ur
        JOIN roles r ON r.id = ur.role_id
        WHERE ur.user_id = NEW.id AND r.tenant_id <> NEW.tenant_id
    ) THEN
        RAISE EXCEPTION 'RBAC user tenant update would invalidate role assignments'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION rustok_guard_role_tenant_update()
RETURNS trigger AS $$
BEGIN
    IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id AND (
        EXISTS (
            SELECT 1 FROM user_roles ur
            JOIN users u ON u.id = ur.user_id
            WHERE ur.role_id = NEW.id AND u.tenant_id <> NEW.tenant_id
        ) OR EXISTS (
            SELECT 1 FROM role_permissions rp
            JOIN permissions p ON p.id = rp.permission_id
            WHERE rp.role_id = NEW.id AND p.tenant_id <> NEW.tenant_id
        )
    ) THEN
        RAISE EXCEPTION 'RBAC role tenant update would invalidate relations'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for statement in [
        "DELETE FROM rbac_artifact_role_permissions WHERE NOT EXISTS (SELECT 1 FROM roles WHERE id = rbac_artifact_role_permissions.role_id AND tenant_id = rbac_artifact_role_permissions.tenant_id) OR NOT EXISTS (SELECT 1 FROM users WHERE id = rbac_artifact_role_permissions.granted_by_actor_id AND tenant_id = rbac_artifact_role_permissions.tenant_id) OR NOT EXISTS (SELECT 1 FROM rbac_artifact_permission_catalog WHERE installation_id = rbac_artifact_role_permissions.installation_id AND permission_key = rbac_artifact_role_permissions.permission_key AND (scope_key = 'platform' OR scope_key = 'tenant:' || rbac_artifact_role_permissions.tenant_id))",
        "DELETE FROM rbac_artifact_role_permission_operations WHERE NOT EXISTS (SELECT 1 FROM roles WHERE id = rbac_artifact_role_permission_operations.role_id AND tenant_id = rbac_artifact_role_permission_operations.tenant_id) OR NOT EXISTS (SELECT 1 FROM users WHERE id = rbac_artifact_role_permission_operations.actor_id AND tenant_id = rbac_artifact_role_permission_operations.tenant_id) OR NOT EXISTS (SELECT 1 FROM rbac_artifact_permission_catalog WHERE installation_id = rbac_artifact_role_permission_operations.installation_id AND permission_key = rbac_artifact_role_permission_operations.permission_key AND (scope_key = 'platform' OR scope_key = 'tenant:' || rbac_artifact_role_permission_operations.tenant_id))",
        "CREATE INDEX IF NOT EXISTS idx_rbac_artifact_role_permissions_role_id ON rbac_artifact_role_permissions (role_id)",
        "CREATE INDEX IF NOT EXISTS idx_rbac_artifact_role_permissions_actor_id ON rbac_artifact_role_permissions (granted_by_actor_id)",
        "CREATE INDEX IF NOT EXISTS idx_rbac_artifact_role_permission_operations_role_id ON rbac_artifact_role_permission_operations (role_id)",
        "CREATE INDEX IF NOT EXISTS idx_rbac_artifact_role_permission_operations_actor_id ON rbac_artifact_role_permission_operations (actor_id)",
        "DROP TRIGGER IF EXISTS trg_rbac_users_tenant_update",
        "DROP TRIGGER IF EXISTS trg_rbac_roles_tenant_update",
    ] {
        manager
            .get_connection()
            .execute_unprepared(statement)
            .await?;
    }
    for statement in sqlite_triggers() {
        manager
            .get_connection()
            .execute_unprepared(statement)
            .await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for name in sqlite_trigger_names() {
        manager
            .get_connection()
            .execute_unprepared(&format!("DROP TRIGGER IF EXISTS {name}"))
            .await?;
    }
    for statement in [
        "DROP INDEX IF EXISTS idx_rbac_artifact_role_permissions_role_id",
        "DROP INDEX IF EXISTS idx_rbac_artifact_role_permissions_actor_id",
        "DROP INDEX IF EXISTS idx_rbac_artifact_role_permission_operations_role_id",
        "DROP INDEX IF EXISTS idx_rbac_artifact_role_permission_operations_actor_id",
        r#"CREATE TRIGGER trg_rbac_users_tenant_update
           BEFORE UPDATE OF tenant_id ON users FOR EACH ROW
           WHEN NEW.tenant_id <> OLD.tenant_id AND EXISTS (
               SELECT 1 FROM user_roles ur JOIN roles r ON r.id = ur.role_id
               WHERE ur.user_id = NEW.id AND r.tenant_id <> NEW.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'RBAC user tenant update would invalidate role assignments'); END"#,
        r#"CREATE TRIGGER trg_rbac_roles_tenant_update
           BEFORE UPDATE OF tenant_id ON roles FOR EACH ROW
           WHEN NEW.tenant_id <> OLD.tenant_id AND (
               EXISTS (
                   SELECT 1 FROM user_roles ur JOIN users u ON u.id = ur.user_id
                   WHERE ur.role_id = NEW.id AND u.tenant_id <> NEW.tenant_id
               ) OR EXISTS (
                   SELECT 1 FROM role_permissions rp JOIN permissions p ON p.id = rp.permission_id
                   WHERE rp.role_id = NEW.id AND p.tenant_id <> NEW.tenant_id
               )
           )
           BEGIN SELECT RAISE(ABORT, 'RBAC role tenant update would invalidate relations'); END"#,
    ] {
        manager
            .get_connection()
            .execute_unprepared(statement)
            .await?;
    }
    Ok(())
}

fn sqlite_trigger_names() -> [&'static str; 10] {
    [
        "trg_rbac_artifact_role_permissions_integrity_insert",
        "trg_rbac_artifact_role_permissions_integrity_update",
        "trg_rbac_artifact_role_permission_operations_integrity_insert",
        "trg_rbac_artifact_role_permission_operations_integrity_update",
        "trg_rbac_users_tenant_update",
        "trg_rbac_roles_tenant_update",
        "trg_rbac_artifact_role_delete",
        "trg_rbac_artifact_actor_delete",
        "trg_rbac_artifact_permission_catalog_identity_update",
        "trg_rbac_artifact_permission_catalog_delete",
    ]
}

fn sqlite_triggers() -> [&'static str; 10] {
    [
        r#"CREATE TRIGGER trg_rbac_artifact_role_permissions_integrity_insert
           BEFORE INSERT ON rbac_artifact_role_permissions FOR EACH ROW
           WHEN NOT EXISTS (SELECT 1 FROM roles WHERE id = NEW.role_id AND tenant_id = NEW.tenant_id)
             OR NOT EXISTS (SELECT 1 FROM users WHERE id = NEW.granted_by_actor_id AND tenant_id = NEW.tenant_id)
             OR NOT EXISTS (SELECT 1 FROM rbac_artifact_permission_catalog WHERE installation_id = NEW.installation_id AND permission_key = NEW.permission_key AND (scope_key = 'platform' OR scope_key = 'tenant:' || NEW.tenant_id))
           BEGIN SELECT RAISE(ABORT, 'RBAC artifact grant tenant integrity violation'); END"#,
        r#"CREATE TRIGGER trg_rbac_artifact_role_permissions_integrity_update
           BEFORE UPDATE OF tenant_id, role_id, installation_id, permission_key, granted_by_actor_id ON rbac_artifact_role_permissions FOR EACH ROW
           WHEN NOT EXISTS (SELECT 1 FROM roles WHERE id = NEW.role_id AND tenant_id = NEW.tenant_id)
             OR NOT EXISTS (SELECT 1 FROM users WHERE id = NEW.granted_by_actor_id AND tenant_id = NEW.tenant_id)
             OR NOT EXISTS (SELECT 1 FROM rbac_artifact_permission_catalog WHERE installation_id = NEW.installation_id AND permission_key = NEW.permission_key AND (scope_key = 'platform' OR scope_key = 'tenant:' || NEW.tenant_id))
           BEGIN SELECT RAISE(ABORT, 'RBAC artifact grant tenant integrity violation'); END"#,
        r#"CREATE TRIGGER trg_rbac_artifact_role_permission_operations_integrity_insert
           BEFORE INSERT ON rbac_artifact_role_permission_operations FOR EACH ROW
           WHEN NOT EXISTS (SELECT 1 FROM roles WHERE id = NEW.role_id AND tenant_id = NEW.tenant_id)
             OR NOT EXISTS (SELECT 1 FROM users WHERE id = NEW.actor_id AND tenant_id = NEW.tenant_id)
             OR NOT EXISTS (SELECT 1 FROM rbac_artifact_permission_catalog WHERE installation_id = NEW.installation_id AND permission_key = NEW.permission_key AND (scope_key = 'platform' OR scope_key = 'tenant:' || NEW.tenant_id))
           BEGIN SELECT RAISE(ABORT, 'RBAC artifact operation tenant integrity violation'); END"#,
        r#"CREATE TRIGGER trg_rbac_artifact_role_permission_operations_integrity_update
           BEFORE UPDATE OF tenant_id, role_id, installation_id, permission_key, actor_id ON rbac_artifact_role_permission_operations FOR EACH ROW
           WHEN NOT EXISTS (SELECT 1 FROM roles WHERE id = NEW.role_id AND tenant_id = NEW.tenant_id)
             OR NOT EXISTS (SELECT 1 FROM users WHERE id = NEW.actor_id AND tenant_id = NEW.tenant_id)
             OR NOT EXISTS (SELECT 1 FROM rbac_artifact_permission_catalog WHERE installation_id = NEW.installation_id AND permission_key = NEW.permission_key AND (scope_key = 'platform' OR scope_key = 'tenant:' || NEW.tenant_id))
           BEGIN SELECT RAISE(ABORT, 'RBAC artifact operation tenant integrity violation'); END"#,
        r#"CREATE TRIGGER trg_rbac_users_tenant_update
           BEFORE UPDATE OF tenant_id ON users FOR EACH ROW
           WHEN NEW.tenant_id <> OLD.tenant_id AND (
               EXISTS (SELECT 1 FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = NEW.id AND r.tenant_id <> NEW.tenant_id)
               OR EXISTS (SELECT 1 FROM rbac_artifact_role_permissions WHERE granted_by_actor_id = NEW.id AND tenant_id <> NEW.tenant_id)
               OR EXISTS (SELECT 1 FROM rbac_artifact_role_permission_operations WHERE actor_id = NEW.id AND tenant_id <> NEW.tenant_id)
           )
           BEGIN SELECT RAISE(ABORT, 'RBAC user tenant update would invalidate authorization relations'); END"#,
        r#"CREATE TRIGGER trg_rbac_roles_tenant_update
           BEFORE UPDATE OF tenant_id ON roles FOR EACH ROW
           WHEN NEW.tenant_id <> OLD.tenant_id AND (
               EXISTS (SELECT 1 FROM user_roles ur JOIN users u ON u.id = ur.user_id WHERE ur.role_id = NEW.id AND u.tenant_id <> NEW.tenant_id)
               OR EXISTS (SELECT 1 FROM role_permissions rp JOIN permissions p ON p.id = rp.permission_id WHERE rp.role_id = NEW.id AND p.tenant_id <> NEW.tenant_id)
               OR EXISTS (SELECT 1 FROM rbac_artifact_role_permissions WHERE role_id = NEW.id AND tenant_id <> NEW.tenant_id)
               OR EXISTS (SELECT 1 FROM rbac_artifact_role_permission_operations WHERE role_id = NEW.id AND tenant_id <> NEW.tenant_id)
           )
           BEGIN SELECT RAISE(ABORT, 'RBAC role tenant update would invalidate authorization relations'); END"#,
        r#"CREATE TRIGGER trg_rbac_artifact_role_delete
           BEFORE DELETE ON roles FOR EACH ROW
           WHEN EXISTS (SELECT 1 FROM rbac_artifact_role_permissions WHERE role_id = OLD.id)
             OR EXISTS (SELECT 1 FROM rbac_artifact_role_permission_operations WHERE role_id = OLD.id)
           BEGIN SELECT RAISE(ABORT, 'RBAC role deletion would orphan artifact authorization state'); END"#,
        r#"CREATE TRIGGER trg_rbac_artifact_actor_delete
           BEFORE DELETE ON users FOR EACH ROW
           WHEN EXISTS (SELECT 1 FROM rbac_artifact_role_permissions WHERE granted_by_actor_id = OLD.id)
             OR EXISTS (SELECT 1 FROM rbac_artifact_role_permission_operations WHERE actor_id = OLD.id)
           BEGIN SELECT RAISE(ABORT, 'RBAC user deletion would orphan artifact authorization audit state'); END"#,
        r#"CREATE TRIGGER trg_rbac_artifact_permission_catalog_identity_update
           BEFORE UPDATE OF scope_key, installation_id, permission_key ON rbac_artifact_permission_catalog FOR EACH ROW
           WHEN (NEW.scope_key <> OLD.scope_key OR NEW.installation_id <> OLD.installation_id OR NEW.permission_key <> OLD.permission_key)
             AND EXISTS (
                 SELECT 1 FROM (
                     SELECT tenant_id, installation_id, permission_key FROM rbac_artifact_role_permissions
                     UNION
                     SELECT tenant_id, installation_id, permission_key FROM rbac_artifact_role_permission_operations
                 ) reference_row
                 WHERE reference_row.installation_id = OLD.installation_id
                   AND reference_row.permission_key = OLD.permission_key
                   AND (OLD.scope_key = 'platform' OR OLD.scope_key = 'tenant:' || reference_row.tenant_id)
             )
           BEGIN SELECT RAISE(ABORT, 'RBAC referenced artifact permission identity is immutable'); END"#,
        r#"CREATE TRIGGER trg_rbac_artifact_permission_catalog_delete
           BEFORE DELETE ON rbac_artifact_permission_catalog FOR EACH ROW
           WHEN EXISTS (
               SELECT 1 FROM (
                   SELECT tenant_id, installation_id, permission_key FROM rbac_artifact_role_permissions
                   UNION
                   SELECT tenant_id, installation_id, permission_key FROM rbac_artifact_role_permission_operations
               ) reference_row
               WHERE reference_row.installation_id = OLD.installation_id
                 AND reference_row.permission_key = OLD.permission_key
                 AND (OLD.scope_key = 'platform' OR OLD.scope_key = 'tenant:' || reference_row.tenant_id)
           )
           BEGIN SELECT RAISE(ABORT, 'RBAC referenced artifact permission identity is immutable'); END"#,
    ]
}

#[cfg(test)]
mod tests {
    use super::{sqlite_trigger_names, sqlite_triggers};

    #[test]
    fn sqlite_trigger_inventory_covers_children_and_parent_changes() {
        assert_eq!(sqlite_triggers().len(), 10);
        assert_eq!(sqlite_trigger_names().len(), 10);
        for required in [
            "artifact_role_permissions_integrity_insert",
            "artifact_role_permissions_integrity_update",
            "artifact_role_permission_operations_integrity_insert",
            "artifact_role_permission_operations_integrity_update",
            "users_tenant_update",
            "roles_tenant_update",
            "artifact_role_delete",
            "artifact_actor_delete",
            "artifact_permission_catalog_identity_update",
            "artifact_permission_catalog_delete",
        ] {
            assert!(
                sqlite_triggers()
                    .iter()
                    .any(|trigger| trigger.contains(required))
            );
            assert!(sqlite_trigger_names().contains(&format!("trg_rbac_{required}").as_str()));
        }
    }
}
