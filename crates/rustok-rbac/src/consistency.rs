//! Owner-owned RBAC persistence consistency diagnostics.

use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RbacConsistencyStats {
    pub users_without_roles_total: i64,
    pub orphan_user_roles_total: i64,
    pub orphan_role_permissions_total: i64,
    pub cross_tenant_user_roles_total: i64,
    pub cross_tenant_role_permissions_total: i64,
    pub reserved_role_slug_collisions_total: i64,
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
    Ok(RbacConsistencyStats {
        users_without_roles_total: row.try_get("", "users_without_roles_total")?,
        orphan_user_roles_total: row.try_get("", "orphan_user_roles_total")?,
        orphan_role_permissions_total: row.try_get("", "orphan_role_permissions_total")?,
        cross_tenant_user_roles_total: row.try_get("", "cross_tenant_user_roles_total")?,
        cross_tenant_role_permissions_total: row
            .try_get("", "cross_tenant_role_permissions_total")?,
        reserved_role_slug_collisions_total: row
            .try_get("", "reserved_role_slug_collisions_total")?,
    })
}
