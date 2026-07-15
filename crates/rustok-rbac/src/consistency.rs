//! Owner-owned RBAC persistence consistency diagnostics.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rustok_api::Permission;
use rustok_core::{Rbac, UserRole};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RbacConsistencyStats {
    pub users_without_roles_total: i64,
    pub orphan_user_roles_total: i64,
    pub orphan_role_permissions_total: i64,
    pub cross_tenant_user_roles_total: i64,
    pub cross_tenant_role_permissions_total: i64,
    pub reserved_role_slug_collisions_total: i64,
    pub system_roles_with_permission_drift_total: i64,
    pub missing_system_role_permissions_total: i64,
    pub extra_system_role_permissions_total: i64,
}

pub async fn load_consistency_stats(
    db: &DatabaseConnection,
) -> Result<RbacConsistencyStats, sea_orm::DbErr> {
    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            "SELECT \
                (SELECT COUNT(*) FROM users u LEFT JOIN user_roles ur ON ur.user_id = u.id WHERE ur.id IS NULL) AS users_without_roles_total, \
                (SELECT COUNT(*) FROM user_roles ur LEFT JOIN users u ON u.id = ur.user_id LEFT JOIN roles r ON r.id = ur.role_id WHERE u.id IS NULL OR r.id IS NULL) AS orphan_user_roles_total, \
                (SELECT COUNT(*) FROM role_permissions rp LEFT JOIN roles r ON r.id = rp.role_id LEFT JOIN permissions p ON p.id = rp.permission_id WHERE r.id IS NULL OR p.id IS NULL) AS orphan_role_permissions_total, \
                (SELECT COUNT(*) FROM user_roles ur JOIN users u ON u.id = ur.user_id JOIN roles r ON r.id = ur.role_id WHERE u.tenant_id <> r.tenant_id) AS cross_tenant_user_roles_total, \
                (SELECT COUNT(*) FROM role_permissions rp JOIN roles r ON r.id = rp.role_id JOIN permissions p ON p.id = rp.permission_id WHERE r.tenant_id <> p.tenant_id) AS cross_tenant_role_permissions_total, \
                (SELECT COUNT(*) FROM roles r WHERE r.slug IN ('super_admin', 'admin', 'manager', 'customer') AND r.is_system = FALSE) AS reserved_role_slug_collisions_total"
                .to_string(),
        ))
        .await?
        .ok_or_else(|| {
            sea_orm::DbErr::Custom("RBAC consistency stats query returned no rows".to_string())
        })?;
    let drift = load_system_role_permission_drift(db).await?;

    Ok(RbacConsistencyStats {
        users_without_roles_total: row.try_get("", "users_without_roles_total")?,
        orphan_user_roles_total: row.try_get("", "orphan_user_roles_total")?,
        orphan_role_permissions_total: row.try_get("", "orphan_role_permissions_total")?,
        cross_tenant_user_roles_total: row.try_get("", "cross_tenant_user_roles_total")?,
        cross_tenant_role_permissions_total: row
            .try_get("", "cross_tenant_role_permissions_total")?,
        reserved_role_slug_collisions_total: row
            .try_get("", "reserved_role_slug_collisions_total")?,
        system_roles_with_permission_drift_total: drift.roles_with_drift,
        missing_system_role_permissions_total: drift.missing_permissions,
        extra_system_role_permissions_total: drift.extra_permissions,
    })
}

#[derive(Default)]
struct SystemRolePermissionDrift {
    roles_with_drift: i64,
    missing_permissions: i64,
    extra_permissions: i64,
}

struct SystemRoleSnapshot {
    role: UserRole,
    permissions: HashSet<Permission>,
    invalid_permission_rows: i64,
}

async fn load_system_role_permission_drift(
    db: &DatabaseConnection,
) -> Result<SystemRolePermissionDrift, sea_orm::DbErr> {
    let rows = db
        .query_all(Statement::from_string(
            db.get_database_backend(),
            "SELECT r.id AS role_id, r.slug AS role_slug, p.resource AS permission_resource, p.action AS permission_action \
             FROM roles r \
             LEFT JOIN role_permissions rp ON rp.role_id = r.id \
             LEFT JOIN permissions p ON p.id = rp.permission_id AND p.tenant_id = r.tenant_id \
             WHERE r.is_system = TRUE AND r.slug IN ('super_admin', 'admin', 'manager', 'customer')"
                .to_string(),
        ))
        .await?;

    let mut snapshots: HashMap<Uuid, SystemRoleSnapshot> = HashMap::new();
    for row in rows {
        let role_id: Uuid = row.try_get("", "role_id")?;
        let role_slug: String = row.try_get("", "role_slug")?;
        let role = UserRole::from_str(&role_slug).map_err(|error| {
            sea_orm::DbErr::Custom(format!(
                "invalid system role slug `{role_slug}` in RBAC consistency query: {error}"
            ))
        })?;
        let snapshot = snapshots
            .entry(role_id)
            .or_insert_with(|| SystemRoleSnapshot {
                role,
                permissions: HashSet::new(),
                invalid_permission_rows: 0,
            });

        let resource: Option<String> = row.try_get("", "permission_resource")?;
        let action: Option<String> = row.try_get("", "permission_action")?;
        let (Some(resource), Some(action)) = (resource, action) else {
            continue;
        };
        match Permission::from_str(&format!("{resource}:{action}")) {
            Ok(permission) => {
                snapshot.permissions.insert(permission);
            }
            Err(_) => snapshot.invalid_permission_rows += 1,
        }
    }

    let mut drift = SystemRolePermissionDrift::default();
    for snapshot in snapshots.into_values() {
        let expected = Rbac::permissions_for_role(&snapshot.role);
        let missing = expected
            .iter()
            .filter(|permission| !snapshot.permissions.contains(permission))
            .count() as i64;
        let extra = snapshot
            .permissions
            .iter()
            .filter(|permission| !expected.contains(permission))
            .count() as i64
            + snapshot.invalid_permission_rows;
        if missing > 0 || extra > 0 {
            drift.roles_with_drift += 1;
            drift.missing_permissions += missing;
            drift.extra_permissions += extra;
        }
    }

    Ok(drift)
}
