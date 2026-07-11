//! Owner-owned RBAC persistence consistency diagnostics.

use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TryGetable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RbacConsistencyStats {
    pub users_without_roles_total: i64,
    pub orphan_user_roles_total: i64,
    pub orphan_role_permissions_total: i64,
}

pub async fn load_consistency_stats(
    db: &DatabaseConnection,
) -> Result<RbacConsistencyStats, sea_orm::DbErr> {
    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            "SELECT (SELECT COUNT(*) FROM users u LEFT JOIN user_roles ur ON ur.user_id = u.id WHERE ur.id IS NULL) AS users_without_roles_total, (SELECT COUNT(*) FROM user_roles ur LEFT JOIN roles r ON r.id = ur.role_id WHERE r.id IS NULL) AS orphan_user_roles_total, (SELECT COUNT(*) FROM role_permissions rp LEFT JOIN permissions p ON p.id = rp.permission_id WHERE p.id IS NULL) AS orphan_role_permissions_total".to_string(),
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("RBAC consistency stats query returned no rows".to_string()))?;
    Ok(RbacConsistencyStats {
        users_without_roles_total: row.try_get("", "users_without_roles_total")?,
        orphan_user_roles_total: row.try_get("", "orphan_user_roles_total")?,
        orphan_role_permissions_total: row.try_get("", "orphan_role_permissions_total")?,
    })
}
