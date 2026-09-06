//! Enforces tenant integrity for RBAC-owned role and permission relations.

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
                "RBAC tenant-integrity migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "RBAC tenant-integrity migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DELETE FROM user_roles ur
WHERE NOT EXISTS (
    SELECT 1
    FROM users u
    JOIN roles r ON r.id = ur.role_id
    WHERE u.id = ur.user_id
      AND u.tenant_id = r.tenant_id
);

DELETE FROM role_permissions rp
WHERE NOT EXISTS (
    SELECT 1
    FROM roles r
    JOIN permissions p ON p.id = rp.permission_id
    WHERE r.id = rp.role_id
      AND r.tenant_id = p.tenant_id
);

CREATE INDEX IF NOT EXISTS idx_user_roles_role_id
    ON user_roles (role_id);
CREATE INDEX IF NOT EXISTS idx_role_permissions_permission_id
    ON role_permissions (permission_id);

CREATE OR REPLACE FUNCTION rustok_enforce_user_role_tenant()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM users u
        JOIN roles r ON r.id = NEW.role_id
        WHERE u.id = NEW.user_id
          AND u.tenant_id = r.tenant_id
    ) THEN
        RAISE EXCEPTION 'RBAC user role tenant mismatch'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_rbac_user_roles_tenant ON user_roles;
CREATE TRIGGER trg_rbac_user_roles_tenant
BEFORE INSERT OR UPDATE OF user_id, role_id ON user_roles
FOR EACH ROW EXECUTE FUNCTION rustok_enforce_user_role_tenant();

CREATE OR REPLACE FUNCTION rustok_enforce_role_permission_tenant()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM roles r
        JOIN permissions p ON p.id = NEW.permission_id
        WHERE r.id = NEW.role_id
          AND r.tenant_id = p.tenant_id
    ) THEN
        RAISE EXCEPTION 'RBAC role permission tenant mismatch'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_rbac_role_permissions_tenant ON role_permissions;
CREATE TRIGGER trg_rbac_role_permissions_tenant
BEFORE INSERT OR UPDATE OF role_id, permission_id ON role_permissions
FOR EACH ROW EXECUTE FUNCTION rustok_enforce_role_permission_tenant();

CREATE OR REPLACE FUNCTION rustok_guard_user_tenant_update()
RETURNS trigger AS $$
BEGIN
    IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id AND EXISTS (
        SELECT 1
        FROM user_roles ur
        JOIN roles r ON r.id = ur.role_id
        WHERE ur.user_id = NEW.id
          AND r.tenant_id <> NEW.tenant_id
    ) THEN
        RAISE EXCEPTION 'RBAC user tenant update would invalidate role assignments'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_rbac_users_tenant_update ON users;
CREATE TRIGGER trg_rbac_users_tenant_update
BEFORE UPDATE OF tenant_id ON users
FOR EACH ROW EXECUTE FUNCTION rustok_guard_user_tenant_update();

CREATE OR REPLACE FUNCTION rustok_guard_role_tenant_update()
RETURNS trigger AS $$
BEGIN
    IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id AND (
        EXISTS (
            SELECT 1
            FROM user_roles ur
            JOIN users u ON u.id = ur.user_id
            WHERE ur.role_id = NEW.id
              AND u.tenant_id <> NEW.tenant_id
        ) OR EXISTS (
            SELECT 1
            FROM role_permissions rp
            JOIN permissions p ON p.id = rp.permission_id
            WHERE rp.role_id = NEW.id
              AND p.tenant_id <> NEW.tenant_id
        )
    ) THEN
        RAISE EXCEPTION 'RBAC role tenant update would invalidate relations'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_rbac_roles_tenant_update ON roles;
CREATE TRIGGER trg_rbac_roles_tenant_update
BEFORE UPDATE OF tenant_id ON roles
FOR EACH ROW EXECUTE FUNCTION rustok_guard_role_tenant_update();

CREATE OR REPLACE FUNCTION rustok_guard_permission_tenant_update()
RETURNS trigger AS $$
BEGIN
    IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id AND EXISTS (
        SELECT 1
        FROM role_permissions rp
        JOIN roles r ON r.id = rp.role_id
        WHERE rp.permission_id = NEW.id
          AND r.tenant_id <> NEW.tenant_id
    ) THEN
        RAISE EXCEPTION 'RBAC permission tenant update would invalidate role relations'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_rbac_permissions_tenant_update ON permissions;
CREATE TRIGGER trg_rbac_permissions_tenant_update
BEFORE UPDATE OF tenant_id ON permissions
FOR EACH ROW EXECUTE FUNCTION rustok_guard_permission_tenant_update();
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
DROP TRIGGER IF EXISTS trg_rbac_user_roles_tenant ON user_roles;
DROP TRIGGER IF EXISTS trg_rbac_role_permissions_tenant ON role_permissions;
DROP TRIGGER IF EXISTS trg_rbac_users_tenant_update ON users;
DROP TRIGGER IF EXISTS trg_rbac_roles_tenant_update ON roles;
DROP TRIGGER IF EXISTS trg_rbac_permissions_tenant_update ON permissions;

DROP FUNCTION IF EXISTS rustok_enforce_user_role_tenant();
DROP FUNCTION IF EXISTS rustok_enforce_role_permission_tenant();
DROP FUNCTION IF EXISTS rustok_guard_user_tenant_update();
DROP FUNCTION IF EXISTS rustok_guard_role_tenant_update();
DROP FUNCTION IF EXISTS rustok_guard_permission_tenant_update();

DROP INDEX IF EXISTS idx_user_roles_role_id;
DROP INDEX IF EXISTS idx_role_permissions_permission_id;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for statement in [
        "DELETE FROM user_roles WHERE NOT EXISTS (SELECT 1 FROM users u JOIN roles r ON r.id = user_roles.role_id WHERE u.id = user_roles.user_id AND u.tenant_id = r.tenant_id)",
        "DELETE FROM role_permissions WHERE NOT EXISTS (SELECT 1 FROM roles r JOIN permissions p ON p.id = role_permissions.permission_id WHERE r.id = role_permissions.role_id AND r.tenant_id = p.tenant_id)",
        "CREATE INDEX IF NOT EXISTS idx_user_roles_role_id ON user_roles (role_id)",
        "CREATE INDEX IF NOT EXISTS idx_role_permissions_permission_id ON role_permissions (permission_id)",
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
    for name in [
        "trg_rbac_user_roles_tenant_insert",
        "trg_rbac_user_roles_tenant_update",
        "trg_rbac_role_permissions_tenant_insert",
        "trg_rbac_role_permissions_tenant_update",
        "trg_rbac_users_tenant_update",
        "trg_rbac_roles_tenant_update",
        "trg_rbac_permissions_tenant_update",
    ] {
        manager
            .get_connection()
            .execute_unprepared(&format!("DROP TRIGGER IF EXISTS {name}"))
            .await?;
    }
    for statement in [
        "DROP INDEX IF EXISTS idx_user_roles_role_id",
        "DROP INDEX IF EXISTS idx_role_permissions_permission_id",
    ] {
        manager
            .get_connection()
            .execute_unprepared(statement)
            .await?;
    }
    Ok(())
}

fn sqlite_triggers() -> [&'static str; 7] {
    [
        r#"CREATE TRIGGER trg_rbac_user_roles_tenant_insert
           BEFORE INSERT ON user_roles FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1 FROM users u JOIN roles r ON r.id = NEW.role_id
               WHERE u.id = NEW.user_id AND u.tenant_id = r.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'RBAC user role tenant mismatch'); END"#,
        r#"CREATE TRIGGER trg_rbac_user_roles_tenant_update
           BEFORE UPDATE OF user_id, role_id ON user_roles FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1 FROM users u JOIN roles r ON r.id = NEW.role_id
               WHERE u.id = NEW.user_id AND u.tenant_id = r.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'RBAC user role tenant mismatch'); END"#,
        r#"CREATE TRIGGER trg_rbac_role_permissions_tenant_insert
           BEFORE INSERT ON role_permissions FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1 FROM roles r JOIN permissions p ON p.id = NEW.permission_id
               WHERE r.id = NEW.role_id AND r.tenant_id = p.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'RBAC role permission tenant mismatch'); END"#,
        r#"CREATE TRIGGER trg_rbac_role_permissions_tenant_update
           BEFORE UPDATE OF role_id, permission_id ON role_permissions FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1 FROM roles r JOIN permissions p ON p.id = NEW.permission_id
               WHERE r.id = NEW.role_id AND r.tenant_id = p.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'RBAC role permission tenant mismatch'); END"#,
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
        r#"CREATE TRIGGER trg_rbac_permissions_tenant_update
           BEFORE UPDATE OF tenant_id ON permissions FOR EACH ROW
           WHEN NEW.tenant_id <> OLD.tenant_id AND EXISTS (
               SELECT 1 FROM role_permissions rp JOIN roles r ON r.id = rp.role_id
               WHERE rp.permission_id = NEW.id AND r.tenant_id <> NEW.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'RBAC permission tenant update would invalidate role relations'); END"#,
    ]
}

#[cfg(test)]
mod tests {
    use super::sqlite_triggers;

    #[test]
    fn sqlite_trigger_inventory_covers_relations_and_parent_tenant_updates() {
        let triggers = sqlite_triggers();
        assert_eq!(triggers.len(), 7);
        for required in [
            "user_roles_tenant_insert",
            "user_roles_tenant_update",
            "role_permissions_tenant_insert",
            "role_permissions_tenant_update",
            "users_tenant_update",
            "roles_tenant_update",
            "permissions_tenant_update",
        ] {
            assert!(triggers.iter().any(|trigger| trigger.contains(required)));
        }
    }
}
