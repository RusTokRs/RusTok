use crate::error::{Error, Result};
use crate::services::server_runtime_context::ServerRuntimeContext;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RbacConsistencyStats {
    pub users_without_roles_total: i64,
    pub orphan_user_roles_total: i64,
    pub orphan_role_permissions_total: i64,
}

pub async fn load_rbac_consistency_stats(
    ctx: &ServerRuntimeContext,
) -> Result<RbacConsistencyStats> {
    let row = ctx
        .db()
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT
                 (SELECT COUNT(*)
                  FROM users u
                  LEFT JOIN user_roles ur ON ur.user_id = u.id
                  WHERE ur.id IS NULL) AS users_without_roles_total,
                 (SELECT COUNT(*)
                  FROM user_roles ur
                  LEFT JOIN roles r ON r.id = ur.role_id
                  WHERE r.id IS NULL) AS orphan_user_roles_total,
                 (SELECT COUNT(*)
                  FROM role_permissions rp
                  LEFT JOIN permissions p ON p.id = rp.permission_id
                  WHERE p.id IS NULL) AS orphan_role_permissions_total"
                .to_string(),
        ))
        .await?
        .ok_or_else(|| Error::string("RBAC consistency stats query returned no rows"))?;

    Ok(RbacConsistencyStats {
        users_without_roles_total: read_metric(&row, "users_without_roles_total")?,
        orphan_user_roles_total: read_metric(&row, "orphan_user_roles_total")?,
        orphan_role_permissions_total: read_metric(&row, "orphan_role_permissions_total")?,
    })
}

fn read_metric(row: &sea_orm::QueryResult, column: &str) -> Result<i64> {
    row.try_get::<i64>("", column).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::RbacConsistencyStats;

    #[test]
    fn stats_default_is_zeroed() {
        let stats = RbacConsistencyStats::default();
        assert_eq!(stats.users_without_roles_total, 0);
        assert_eq!(stats.orphan_user_roles_total, 0);
        assert_eq!(stats.orphan_role_permissions_total, 0);
    }
}
